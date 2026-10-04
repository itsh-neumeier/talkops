//! Login, logout, session info, password change and first-run setup.

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use talkops_core::audit;
use talkops_core::crypto;
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
    responses((status = 200, body = Me), (status = 401), (status = 429))
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
    state.limiter.reset(&key);
    start_session(&state, user, &meta).await
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
