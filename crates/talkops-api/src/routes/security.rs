//! SIP protection (admin): failed-login limits, trusted networks, bans.

use std::net::IpAddr;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::sip_guard::{self, Ban, GuardSettings, GuardSettingsInput};
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_sip_guard, update_sip_guard))
        .routes(routes!(unban))
}

#[derive(Serialize, ToSchema)]
pub struct SipGuard {
    pub settings: GuardSettings,
    /// Active bans, newest first.
    pub bans: Vec<Ban>,
}

/// Settings and active bans of the SIP login protection.
#[utoipa::path(get, path = "/api/v1/security/sip", tag = "settings", responses((status = 200, body = SipGuard)))]
pub async fn get_sip_guard(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<SipGuard>> {
    auth.require(Role::Admin)?;
    Ok(Json(SipGuard {
        settings: sip_guard::get(&state.db, auth.tenant).await?,
        bans: sip_guard::list_bans(&state.db, auth.tenant).await?,
    }))
}

/// Changes the SIP login protection.
#[utoipa::path(put, path = "/api/v1/security/sip", tag = "settings", request_body = GuardSettingsInput, responses((status = 200, body = GuardSettings)))]
pub async fn update_sip_guard(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<GuardSettingsInput>,
) -> ApiResult<Json<GuardSettings>> {
    auth.require(Role::Admin)?;
    let s = sip_guard::update(&state.db, auth.tenant, &input).await?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "sip_guard",
        None,
        json!(s),
    )
    .await;
    Ok(Json(s))
}

/// Lifts the ban of an address.
#[utoipa::path(delete, path = "/api/v1/security/sip/bans/{ip}", tag = "settings", params(("ip" = String, Path)), responses((status = 204)))]
pub async fn unban(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(ip): Path<String>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let ip: IpAddr = ip.parse().map_err(|_| ApiError::NotFound)?;
    sip_guard::unban(&state.db, auth.tenant, ip).await?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "unban",
        "sip_ban",
        Some(ip.to_string()),
        json!({}),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
