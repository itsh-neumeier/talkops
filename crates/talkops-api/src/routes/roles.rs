//! Roles and their permissions. Users with `users.view` or `users.manage`
//! read them (to choose a role); only admins change them.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::roles::{self, Permission, RoleInput, RoleRecord};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_roles, create_role))
        .routes(routes!(update_role, delete_role))
}

/// One entry of the permission catalog.
#[derive(Serialize, ToSchema)]
pub struct PermissionInfo {
    pub key: &'static str,
    /// Group in the editor, e.g. `calls` or `system`.
    pub area: &'static str,
    /// Gives access to secrets, the system or more rights.
    pub sensitive: bool,
}

#[derive(Serialize, ToSchema)]
pub struct RolesOverview {
    pub permissions: Vec<PermissionInfo>,
    pub roles: Vec<RoleRecord>,
}

fn require_admin(auth: &AuthUser) -> ApiResult<()> {
    if auth.is_admin() {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

/// The permission catalog and all roles.
#[utoipa::path(get, path = "/api/v1/roles", tag = "users", responses((status = 200, body = RolesOverview)))]
pub async fn list_roles(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<RolesOverview>> {
    if !auth.can(Permission::UsersView) && !auth.can(Permission::UsersManage) {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(RolesOverview {
        permissions: Permission::ALL
            .iter()
            .map(|p| PermissionInfo {
                key: p.key(),
                area: p.area(),
                sensitive: p.sensitive(),
            })
            .collect(),
        roles: roles::list(&state.db, auth.tenant).await?,
    }))
}

async fn audit_role(state: &AppState, auth: &AuthUser, action: &str, role: &RoleRecord) {
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        action,
        "role",
        Some(role.id.to_string()),
        json!({"name": role.name, "permissions": role.permissions, "directory_group": role.directory_group}),
    )
    .await;
}

/// Creates an own role (admin).
#[utoipa::path(post, path = "/api/v1/roles", tag = "users", request_body = RoleInput, responses((status = 200, body = RoleRecord)))]
pub async fn create_role(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<RoleInput>,
) -> ApiResult<Json<RoleRecord>> {
    require_admin(&auth)?;
    let role = roles::create(&state.db, auth.tenant, &input).await?;
    audit_role(&state, &auth, "create", &role).await;
    Ok(Json(role))
}

/// Changes a role (admin). The administrator role cannot be changed.
#[utoipa::path(put, path = "/api/v1/roles/{id}", tag = "users", params(("id" = Uuid, Path)), request_body = RoleInput, responses((status = 200, body = RoleRecord)))]
pub async fn update_role(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<RoleInput>,
) -> ApiResult<Json<RoleRecord>> {
    require_admin(&auth)?;
    let role = roles::update(&state.db, auth.tenant, id, &input).await?;
    audit_role(&state, &auth, "update", &role).await;
    Ok(Json(role))
}

/// Deletes an own role without users (admin).
#[utoipa::path(delete, path = "/api/v1/roles/{id}", tag = "users", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_role(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    require_admin(&auth)?;
    let role = roles::delete(&state.db, auth.tenant, id).await?;
    audit_role(&state, &auth, "delete", &role).await;
    Ok(StatusCode::NO_CONTENT)
}
