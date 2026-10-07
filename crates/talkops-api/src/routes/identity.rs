//! Settings for single sign-on (OIDC) and directory logins (LDAP/AD).

use axum::Json;
use axum::extract::State;
use serde_json::json;
use talkops_core::audit;
use talkops_core::identity::{self, IdentityInput, IdentitySettings};
use talkops_core::tenant::TenantId;
use talkops_core::users::Role;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_identity, update_identity))
        .routes(routes!(test_ldap))
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

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct LdapTest {
    /// Optional user to look up (no password check).
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct LdapTestResult {
    pub dn: Option<String>,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub groups: Vec<String>,
    /// Role the user would get; `None`: not allowed to log in.
    pub role: Option<Role>,
}

/// Tests the saved LDAP settings: service bind and an optional user lookup (admin).
#[utoipa::path(post, path = "/api/v1/settings/identity/ldap-test", tag = "settings", request_body = LdapTest, responses((status = 200, body = LdapTestResult), (status = 502)))]
pub async fn test_ldap(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<LdapTest>,
) -> ApiResult<Json<LdapTestResult>> {
    auth.require(Role::Admin)?;
    let tenant: TenantId = auth.tenant;
    let s = identity::get(&state.db, tenant).await?;
    let (_, bind_password) = identity::secrets(&state.db, tenant, &state.secrets).await?;
    let user = match crate::ldap::test(&s, bind_password.as_deref(), req.username.as_deref()).await
    {
        Ok(u) => u,
        Err(crate::ldap::LdapError::Invalid) => {
            return Err(ApiError::BadRequest(
                "user not found (or not unique)".into(),
            ));
        }
        Err(err) => return Err(ApiError::Device(err.to_string())),
    };
    Ok(Json(match user {
        Some(u) => LdapTestResult {
            role: identity::map_role(&s, &crate::ldap::group_names(&u.groups)),
            dn: Some(u.dn),
            username: Some(u.username),
            display_name: Some(u.display_name),
            email: u.email,
            groups: u.groups,
        },
        None => LdapTestResult {
            dn: None,
            username: None,
            display_name: None,
            email: None,
            groups: vec![],
            role: None,
        },
    }))
}
