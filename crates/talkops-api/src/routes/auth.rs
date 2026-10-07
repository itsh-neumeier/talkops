//! Login, logout, session info, password change and first-run setup.

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use talkops_core::audit;
use talkops_core::crypto;
use talkops_core::mfa;
use talkops_core::tenant::TenantId;
use talkops_core::users::{self, NewUser, Role, User};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::{self as session, AuthUser};
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(setup_status, setup))
        .routes(routes!(login))
        .routes(routes!(login_totp))
        .routes(routes!(totp_status))
        .routes(routes!(totp_setup))
        .routes(routes!(totp_enable))
        .routes(routes!(totp_disable))
        .routes(routes!(logout))
        .routes(routes!(me))
        .routes(routes!(change_password))
}

/// Request metadata for unauthenticated endpoints (client IP, HTTPS, CSRF guard).
pub struct RequestMeta {
    ip: Option<String>,
    https: bool,
    user_agent: Option<String>,
    token: Option<String>,
}

impl FromRequestParts<AppState> for RequestMeta {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        session::check_requested_with(parts)?;
        Ok(Self {
            ip: session::client_ip(parts),
            https: session::is_https(&parts.headers),
            user_agent: parts
                .headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
            token: session::cookie_value(&parts.headers, session::SESSION_COOKIE)
                .map(str::to_owned),
        })
    }
}

#[derive(Serialize, ToSchema)]
pub struct SetupStatus {
    /// True until the first admin account has been created.
    pub needs_setup: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct SetupRequest {
    pub username: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Answer of a login that needs a second factor (no session yet).
#[derive(Serialize, ToSchema)]
pub struct MfaRequired {
    /// Always `true`.
    pub mfa_required: bool,
    /// Send with the code to `/api/v1/auth/login/totp` (valid 5 minutes).
    pub mfa_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct TotpLogin {
    pub mfa_token: String,
    /// 6-digit code from the authenticator app or a recovery code.
    pub code: String,
}

#[derive(Serialize, ToSchema)]
pub struct Me {
    pub user: User,
    /// Send as `X-CSRF-Token` on state-changing requests.
    pub csrf_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordChange {
    pub current_password: String,
    pub new_password: String,
}

/// Whether the first-run setup is still pending.
#[utoipa::path(get, path = "/api/v1/setup", tag = "auth", responses((status = 200, body = SetupStatus)))]
pub async fn setup_status(State(state): State<AppState>) -> ApiResult<Json<SetupStatus>> {
    let count = users::count(&state.db, TenantId::DEFAULT).await?;
    Ok(Json(SetupStatus {
        needs_setup: count == 0,
    }))
}

/// Creates the first admin account and logs it in. Only works while no user exists.
#[utoipa::path(
    post, path = "/api/v1/setup", tag = "auth", request_body = SetupRequest,
    responses((status = 200, body = Me), (status = 409, description = "setup already completed"))
)]
pub async fn setup(
    State(state): State<AppState>,
    meta: RequestMeta,
    Json(req): Json<SetupRequest>,
) -> ApiResult<Response> {
    let new = NewUser {
        username: req.username,
        display_name: req.display_name,
        email: None,
        role: Role::Admin,
        password: Some(req.password),
    };
    let user = users::create_first_admin(&state.db, TenantId::DEFAULT, &new).await?;
    let actor = audit::Actor {
        tenant: TenantId::DEFAULT,
        user_id: Some(user.id),
        ip: meta.ip.clone(),
    };
    audit::record(
        &state.db,
        &actor,
        "setup",
        "user",
        Some(user.id.to_string()),
        serde_json::json!({"username": user.username}),
    )
    .await?;
    start_session(&state, user, &meta).await
}

async fn start_session(state: &AppState, user: User, meta: &RequestMeta) -> ApiResult<Response> {
    let s = users::create_session(
        &state.db,
        user.id,
        meta.ip.as_deref(),
        meta.user_agent.as_deref(),
    )
    .await?;
    let mut response = Json(Me {
        user,
        csrf_token: s.csrf_token,
    })
    .into_response();
    let cookie = HeaderValue::from_str(&session::session_cookie(&s.token, meta.https))
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    Ok(response)
}

/// Logs in with username and password; sets the session cookie.
#[utoipa::path(
    post, path = "/api/v1/auth/login", tag = "auth", request_body = LoginRequest,
    responses((status = 200, body = Me, description = "logged in; or `MfaRequired` if a code is needed"), (status = 401), (status = 429))
)]
pub async fn login(
    State(state): State<AppState>,
    meta: RequestMeta,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Response> {
    let key = meta.ip.clone().unwrap_or_default();
    if !state.limiter.allow(&key) {
        return Err(ApiError::TooManyRequests);
    }
    let Some(user) =
        users::authenticate(&state.db, TenantId::DEFAULT, &req.username, &req.password).await?
    else {
        tracing::info!(username = %req.username.chars().take(64).collect::<String>(), ip = ?meta.ip, "failed login");
        return Err(ApiError::Unauthorized);
    };
    if user.totp_enabled {
        // Password correct, second factor missing: no session yet.
        let token = mfa::new_challenge(&state.db, user.id).await?;
        return Ok(Json(MfaRequired {
            mfa_required: true,
            mfa_token: token,
        })
        .into_response());
    }
    state.limiter.reset(&key);
    start_session(&state, user, &meta).await
}

/// Second login step: TOTP or recovery code.
#[utoipa::path(
    post, path = "/api/v1/auth/login/totp", tag = "auth", request_body = TotpLogin,
    responses((status = 200, body = Me), (status = 401), (status = 429))
)]
pub async fn login_totp(
    State(state): State<AppState>,
    meta: RequestMeta,
    Json(req): Json<TotpLogin>,
) -> ApiResult<Response> {
    let key = meta.ip.clone().unwrap_or_default();
    if !state.limiter.allow(&key) {
        return Err(ApiError::TooManyRequests);
    }
    let Some(user_id) =
        mfa::complete_challenge(&state.db, &state.secrets, &req.mfa_token, &req.code).await?
    else {
        tracing::info!(ip = ?meta.ip, "failed second factor");
        return Err(ApiError::Unauthorized);
    };
    state.limiter.reset(&key);
    let user = users::get(&state.db, TenantId::DEFAULT, user_id).await?;
    if !user.enabled {
        return Err(ApiError::Unauthorized);
    }
    start_session(&state, user, &meta).await
}

#[derive(Serialize, ToSchema)]
pub struct TotpStatus {
    pub enabled: bool,
    pub recovery_codes_left: i64,
    /// Only local accounts can use TOTP; others log in via their directory.
    pub available: bool,
}

/// Two-factor state of the own account.
#[utoipa::path(get, path = "/api/v1/auth/totp", tag = "auth", responses((status = 200, body = TotpStatus)))]
pub async fn totp_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<TotpStatus>> {
    let user = users::get(&state.db, auth.tenant, auth.id).await?;
    Ok(Json(TotpStatus {
        enabled: user.totp_enabled,
        recovery_codes_left: mfa::recovery_codes_left(&state.db, auth.id).await?,
        available: user.auth_source == "local",
    }))
}

#[derive(Serialize, ToSchema)]
pub struct TotpSetup {
    /// Base32 secret for manual entry.
    pub secret: String,
    pub otpauth_uri: String,
    /// QR code of the URI as SVG.
    pub qr_svg: String,
}

/// Starts setting up two-factor login: a new secret to scan.
#[utoipa::path(post, path = "/api/v1/auth/totp/setup", tag = "auth", responses((status = 200, body = TotpSetup)))]
pub async fn totp_setup(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<TotpSetup>> {
    let user = users::get(&state.db, auth.tenant, auth.id).await?;
    if user.auth_source != "local" {
        return Err(ApiError::BadRequest(
            "two-factor login is managed by your directory".into(),
        ));
    }
    let secret = mfa::begin_enrollment(&state.db, &state.secrets, auth.id).await?;
    let uri = mfa::otpauth_uri(&secret, &user.username);
    let qr_svg = qrcode::QrCode::new(uri.as_bytes())
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(200, 200)
        .quiet_zone(true)
        .build();
    Ok(Json(TotpSetup {
        secret,
        otpauth_uri: uri,
        qr_svg,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct TotpCode {
    pub code: String,
}

#[derive(Serialize, ToSchema)]
pub struct RecoveryCodes {
    /// Shown only once; each works one time instead of a code.
    pub recovery_codes: Vec<String>,
}

/// Activates two-factor login with a code from the app; returns recovery codes.
#[utoipa::path(post, path = "/api/v1/auth/totp/enable", tag = "auth", request_body = TotpCode, responses((status = 200, body = RecoveryCodes), (status = 422)))]
pub async fn totp_enable(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TotpCode>,
) -> ApiResult<Json<RecoveryCodes>> {
    let codes = mfa::confirm_enrollment(&state.db, &state.secrets, auth.id, &req.code).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "enable_totp",
        "user",
        Some(auth.id.to_string()),
        serde_json::json!({}),
    )
    .await?;
    Ok(Json(RecoveryCodes {
        recovery_codes: codes,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct TotpDisable {
    pub password: String,
}

/// Turns two-factor login off (needs the password).
#[utoipa::path(post, path = "/api/v1/auth/totp/disable", tag = "auth", request_body = TotpDisable, responses((status = 204), (status = 422)))]
pub async fn totp_disable(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TotpDisable>,
) -> ApiResult<StatusCode> {
    let user = users::get(&state.db, auth.tenant, auth.id).await?;
    let ok = user
        .password_hash
        .as_deref()
        .is_some_and(|h| crypto::verify_password(&req.password, h));
    if !ok {
        return Err(ApiError::BadRequest("password is wrong".into()));
    }
    mfa::disable(&state.db, auth.id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "disable_totp",
        "user",
        Some(auth.id.to_string()),
        serde_json::json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ends the current session.
#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "auth", responses((status = 204)))]
pub async fn logout(State(state): State<AppState>, meta: RequestMeta) -> ApiResult<Response> {
    if let Some(token) = &meta.token {
        users::delete_session(&state.db, token).await?;
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session::clear_cookie())
            .map_err(|e| ApiError::Internal(e.to_string()))?,
    );
    Ok((StatusCode::NO_CONTENT, headers).into_response())
}

/// The logged-in user and the CSRF token of the session.
#[utoipa::path(get, path = "/api/v1/auth/me", tag = "auth", responses((status = 200, body = Me), (status = 401)))]
pub async fn me(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Me>> {
    let user = users::get(&state.db, auth.tenant, auth.id).await?;
    Ok(Json(Me {
        user,
        csrf_token: auth.csrf_token,
    }))
}

/// Changes the own password and ends all sessions of the user (log in again).
#[utoipa::path(
    post, path = "/api/v1/auth/password", tag = "auth", request_body = PasswordChange,
    responses((status = 204), (status = 401), (status = 422))
)]
pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PasswordChange>,
) -> ApiResult<StatusCode> {
    let user = users::get(&state.db, auth.tenant, auth.id).await?;
    let ok = user
        .password_hash
        .as_deref()
        .is_some_and(|h| crypto::verify_password(&req.current_password, h));
    if !ok {
        return Err(ApiError::BadRequest("current password is wrong".into()));
    }
    users::set_password(&state.db, auth.tenant, auth.id, &req.new_password).await?;
    users::delete_user_sessions(&state.db, auth.id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "change_password",
        "user",
        Some(auth.id.to_string()),
        serde_json::json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
