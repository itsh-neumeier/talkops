//! User management. Changing users needs `users.manage`; only admins give
//! the admin role or change users with more rights than themselves.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::users::{self, NewUser, Role, User, UserUpdate};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use talkops_core::roles::{self, Permission};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_users, create_user))
        .routes(routes!(get_user, update_user, delete_user))
        .routes(routes!(set_user_password))
        .routes(routes!(reset_user_totp))
}

/// Resolves the role choice of a request (`role_id` of any role, else the
/// built-in `role`) into the stored form, and checks that the caller may
/// give it: only admins give the admin role, and nobody gives a role with
/// permissions that they do not have.
async fn assignable(
    state: &AppState,
    auth: &AuthUser,
    role: Role,
    role_id: Option<Uuid>,
) -> ApiResult<(Role, Option<Uuid>)> {
    let (role, own, permissions) = match role_id {
        Some(id) => {
            let (role, own, record) = roles::assignment(&state.db, auth.tenant, id)
                .await
                .map_err(|_| ApiError::BadRequest("unknown role".into()))?;
            (role, own, record.permission_set())
        }
        None => (
            role,
            None,
            roles::of_user(&state.db, auth.tenant, role, None).await?,
        ),
    };
    if !auth.is_admin() && (role == Role::Admin || !auth.permissions.covers(&permissions)) {
        return Err(ApiError::Forbidden);
    }
    Ok((role, own))
}

/// Users with more rights than the caller (admins, or roles with other
/// permissions) can only be changed by admins.
async fn manageable(state: &AppState, auth: &AuthUser, user: &User) -> ApiResult<()> {
    if auth.is_admin() {
        return Ok(());
    }
    let theirs = roles::of_user(&state.db, auth.tenant, user.role, user.role_id).await?;
    if user.role == Role::Admin || !auth.permissions.covers(&theirs) {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

#[derive(Deserialize, ToSchema)]
pub struct SetPassword {
    pub password: String,
}

/// Lists all users (operator or admin).
#[utoipa::path(get, path = "/api/v1/users", tag = "users", responses((status = 200, body = [User])))]
pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<User>>> {
    auth.require_perm(Permission::UsersView)?;
    Ok(Json(users::list(&state.db, auth.tenant).await?))
}

/// Creates a user (admin).
#[utoipa::path(post, path = "/api/v1/users", tag = "users", request_body = NewUser, responses((status = 200, body = User)))]
pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<NewUser>,
) -> ApiResult<Json<User>> {
    auth.require_perm(Permission::UsersManage)?;
    let (role, role_id) = assignable(&state, &auth, input.role, input.role_id).await?;
    let input = NewUser {
        role,
        role_id,
        ..input
    };
    let user = users::create(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "user",
        Some(user.id.to_string()),
        json!({"username": user.username, "role": user.role, "role_id": user.role_id}),
    )
    .await?;
    Ok(Json(user))
}

/// Returns a user (operator, admin, or the user themself).
#[utoipa::path(get, path = "/api/v1/users/{id}", tag = "users", params(("id" = Uuid, Path)), responses((status = 200, body = User)))]
pub async fn get_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    if auth.id != id {
        auth.require_perm(Permission::UsersView)?;
    }
    Ok(Json(users::get(&state.db, auth.tenant, id).await?))
}

/// Updates name, e-mail, role and enabled flag (admin).
#[utoipa::path(put, path = "/api/v1/users/{id}", tag = "users", params(("id" = Uuid, Path)), request_body = UserUpdate, responses((status = 200, body = User)))]
pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UserUpdate>,
) -> ApiResult<Json<User>> {
    auth.require_perm(Permission::UsersManage)?;
    let before = users::get(&state.db, auth.tenant, id).await?;
    manageable(&state, &auth, &before).await?;
    let (role, role_id) = assignable(&state, &auth, input.role, input.role_id).await?;
    let input = UserUpdate {
        role,
        role_id,
        ..input
    };
    let demotes_admin = before.role == Role::Admin
        && before.enabled
        && (input.role != Role::Admin || !input.enabled);
    if demotes_admin && users::enabled_admin_count(&state.db, auth.tenant).await? <= 1 {
        return Err(ApiError::BadRequest(
            "the last enabled admin cannot be demoted or disabled".into(),
        ));
    }
    let user = users::update(&state.db, auth.tenant, id, &input).await?;
    if !user.enabled || user.role != before.role {
        users::delete_user_sessions(&state.db, id).await?;
    }
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "user",
        Some(id.to_string()),
        json!({"role": user.role, "role_id": user.role_id, "enabled": user.enabled}),
    )
    .await?;
    Ok(Json(user))
}

/// Deletes a user (admin). Their extensions stay, unassigned.
#[utoipa::path(delete, path = "/api/v1/users/{id}", tag = "users", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::UsersManage)?;
    if id == auth.id {
        return Err(ApiError::BadRequest(
            "you cannot delete your own account".into(),
        ));
    }
    let user = users::get(&state.db, auth.tenant, id).await?;
    manageable(&state, &auth, &user).await?;
    if user.role == Role::Admin
        && user.enabled
        && users::enabled_admin_count(&state.db, auth.tenant).await? <= 1
    {
        return Err(ApiError::BadRequest(
            "the last enabled admin cannot be deleted".into(),
        ));
    }
    users::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "user",
        Some(id.to_string()),
        json!({"username": user.username}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Sets a user's web password (admin); ends the user's sessions.
#[utoipa::path(post, path = "/api/v1/users/{id}/password", tag = "users", params(("id" = Uuid, Path)), request_body = SetPassword, responses((status = 204)))]
pub async fn set_user_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<SetPassword>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::UsersManage)?;
    let user = users::get(&state.db, auth.tenant, id).await?;
    manageable(&state, &auth, &user).await?;
    if user.auth_source != "local" {
        return Err(ApiError::BadRequest(
            "this account logs in through the directory".into(),
        ));
    }
    users::set_password(&state.db, auth.tenant, id, &input.password).await?;
    users::delete_user_sessions(&state.db, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "set_password",
        "user",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Turns off a user's two-factor login, e.g. after a lost phone (admin).
/// Ends the user's sessions.
#[utoipa::path(post, path = "/api/v1/users/{id}/totp/reset", tag = "users", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn reset_user_totp(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::UsersManage)?;
    let user = users::get(&state.db, auth.tenant, id).await?;
    manageable(&state, &auth, &user).await?;
    talkops_core::mfa::disable(&state.db, id).await?;
    users::delete_user_sessions(&state.db, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "reset_totp",
        "user",
        Some(id.to_string()),
        json!({"username": user.username}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
