//! Telephony settings, live status, call log and audit log.

use axum::Json;
use axum::extract::{Query, State};
use serde_json::json;
use talkops_core::audit::{self, AuditEntry};
use talkops_core::cdr::{self, Cdr, CdrQuery};
use talkops_core::extensions;
use talkops_core::settings::{self, TenantSettings};
use talkops_core::users::Role;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::telephony::LiveStatus;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_settings, update_settings))
        .routes(routes!(telephony_status))
        .routes(routes!(list_calls))
        .routes(routes!(list_audit))
}

/// Telephony settings (any logged-in user may read them).
#[utoipa::path(get, path = "/api/v1/settings", tag = "settings", responses((status = 200, body = Object)))]
pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<TenantSettings>> {
    Ok(Json(settings::get(&state.db, auth.tenant).await?))
}

fn validate(s: &TenantSettings) -> Result<(), ApiError> {
    let digits = |v: &str| v.bytes().all(|b| b.is_ascii_digit());
    if !digits(&s.country_code) || s.country_code.is_empty() || !digits(&s.area_code) {
        return Err(ApiError::BadRequest(
            "country and area code must be digits".into(),
        ));
    }
    if s.emergency_numbers.is_empty()
        || s.emergency_numbers
            .iter()
            .any(|n| n.is_empty() || !digits(n))
    {
        return Err(ApiError::BadRequest(
            "at least one emergency number (digits) is required".into(),
        ));
    }
    let ip_ok = s.external_ip.is_empty()
        || s.external_ip.parse::<std::net::IpAddr>().is_ok()
        || s.external_ip.strip_prefix("stun:").is_some_and(|h| {
            !h.is_empty()
                && h.chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-:".contains(c))
        });
    if !ip_ok {
        return Err(ApiError::BadRequest(
            "external IP must be an IP address or stun:host[:port]".into(),
        ));
    }
    Ok(())
}

/// Updates telephony settings (admin). Changing the external IP restarts the
/// trunk profile, which interrupts active external calls.
#[utoipa::path(put, path = "/api/v1/settings", tag = "settings", request_body = Object, responses((status = 200, body = Object)))]
pub async fn update_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TenantSettings>,
) -> ApiResult<Json<TenantSettings>> {
    auth.require(Role::Admin)?;
    validate(&input)?;
    let before = settings::get(&state.db, auth.tenant).await?;
    let after = settings::update(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "settings",
        None,
        serde_json::to_value(&after).unwrap_or(json!({})),
    )
    .await?;
    if before.external_ip != after.external_ip {
        let telephony = state.telephony.clone();
        tokio::spawn(async move { telephony.restart_external_profile().await });
    }
    Ok(Json(after))
}

/// Live registrations and trunk states (operator or admin).
#[utoipa::path(get, path = "/api/v1/telephony/status", tag = "settings", responses((status = 200, body = LiveStatus)))]
pub async fn telephony_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<LiveStatus>> {
    auth.require(Role::Operator)?;
    Ok(Json(state.telephony.snapshot().await))
}

/// Call log. Operators and admins see all calls, users only their own extensions.
#[utoipa::path(get, path = "/api/v1/calls", tag = "settings", params(CdrQuery), responses((status = 200, body = [Cdr])))]
pub async fn list_calls(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<CdrQuery>,
) -> ApiResult<Json<Vec<Cdr>>> {
    if auth.role >= Role::Operator {
        return Ok(Json(cdr::list(&state.db, auth.tenant, &query).await?));
    }
    let own = extensions::list_for_user(&state.db, auth.tenant, auth.id).await?;
    let mut out = Vec::new();
    for ext in own
        .iter()
        .filter(|e| query.extension_id.is_none_or(|id| id == e.id))
    {
        let q = CdrQuery {
            extension_id: Some(ext.id),
            ..query.clone()
        };
        out.extend(cdr::list(&state.db, auth.tenant, &q).await?);
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.started_at));
    out.truncate(query.limit.unwrap_or(100).clamp(1, 500) as usize);
    Ok(Json(out))
}

/// Audit log of administrative changes (admin).
#[utoipa::path(get, path = "/api/v1/audit", tag = "settings", responses((status = 200, body = [AuditEntry])))]
pub async fn list_audit(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<AuditEntry>>> {
    auth.require(Role::Admin)?;
    Ok(Json(audit::list(&state.db, auth.tenant, 200).await?))
}
