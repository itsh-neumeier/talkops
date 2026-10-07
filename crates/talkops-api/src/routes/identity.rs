//! Settings for single sign-on (OIDC) and directory logins (LDAP/AD).

use axum::Json;
use axum::extract::State;
use serde_json::json;
use talkops_core::audit;
use talkops_core::identity::{self, IdentityInput, IdentitySettings};
use talkops_core::users::Role;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiResult;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_identity, update_identity))
}

/// Identity provider settings (admin).
#[utoipa::path(get, path = "/api/v1/settings/identity", tag = "settings", responses((status = 200, body = IdentitySettings)))]
pub async fn get_identity(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<IdentitySettings>> {
    auth.require(Role::Admin)?;
    Ok(Json(identity::get(&state.db, auth.tenant).await?))
}

/// Updates identity provider settings (admin). Secrets: omit to keep.
#[utoipa::path(put, path = "/api/v1/settings/identity", tag = "settings", request_body = IdentityInput, responses((status = 200, body = IdentitySettings)))]
pub async fn update_identity(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<IdentityInput>,
) -> ApiResult<Json<IdentitySettings>> {
    auth.require(Role::Admin)?;
    let s = identity::update(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "identity_settings",
        None,
        json!({
            "oidc_enabled": s.oidc_enabled, "oidc_issuer": s.oidc_issuer,
            "ldap_enabled": s.ldap_enabled, "ldap_url": s.ldap_url,
            "admin_group": s.admin_group, "operator_group": s.operator_group,
            "user_group": s.user_group,
            "oidc_secret_changed": input.oidc_client_secret.is_some(),
            "ldap_password_changed": input.ldap_bind_password.is_some(),
        }),
    )
    .await?;
    Ok(Json(s))
}
