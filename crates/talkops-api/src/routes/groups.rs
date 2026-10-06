//! Ring groups.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::json;
use talkops_core::ring_groups::{self, RingGroup, RingGroupInput};
use talkops_core::users::Role;
use talkops_core::{audit, settings};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiResult;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_groups, create_group))
        .routes(routes!(get_group, update_group, delete_group))
}

/// Lists ring groups (operator or admin).
#[utoipa::path(get, path = "/api/v1/ring-groups", tag = "routing", responses((status = 200, body = [RingGroup])))]
pub async fn list_groups(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<RingGroup>>> {
    auth.require(Role::Operator)?;
    Ok(Json(ring_groups::list(&state.db, auth.tenant).await?))
}

/// Creates a ring group (admin).
#[utoipa::path(post, path = "/api/v1/ring-groups", tag = "routing", request_body = RingGroupInput, responses((status = 200, body = RingGroup)))]
pub async fn create_group(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<RingGroupInput>,
) -> ApiResult<Json<RingGroup>> {
    auth.require(Role::Admin)?;
    let emergency = settings::get(&state.db, auth.tenant)
        .await?
        .emergency_numbers;
    let group = ring_groups::create(&state.db, auth.tenant, &input, &emergency).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "ring_group",
        Some(group.id.to_string()),
        json!({"name": group.name, "number": group.number, "members": group.members.len()}),
    )
    .await?;
    Ok(Json(group))
}

/// Returns a ring group (operator or admin).
#[utoipa::path(get, path = "/api/v1/ring-groups/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 200, body = RingGroup)))]
pub async fn get_group(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<RingGroup>> {
    auth.require(Role::Operator)?;
    Ok(Json(ring_groups::get(&state.db, auth.tenant, id).await?))
}

/// Updates a ring group (admin).
#[utoipa::path(put, path = "/api/v1/ring-groups/{id}", tag = "routing", params(("id" = Uuid, Path)), request_body = RingGroupInput, responses((status = 200, body = RingGroup)))]
pub async fn update_group(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<RingGroupInput>,
) -> ApiResult<Json<RingGroup>> {
    auth.require(Role::Admin)?;
    let emergency = settings::get(&state.db, auth.tenant)
        .await?
        .emergency_numbers;
    let group = ring_groups::update(&state.db, auth.tenant, id, &input, &emergency).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "ring_group",
        Some(id.to_string()),
        json!({"name": group.name, "number": group.number, "members": group.members.len()}),
    )
    .await?;
    Ok(Json(group))
}

/// Deletes a ring group (admin).
#[utoipa::path(delete, path = "/api/v1/ring-groups/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_group(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let group = ring_groups::get(&state.db, auth.tenant, id).await?;
    ring_groups::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "ring_group",
        Some(id.to_string()),
        json!({"name": group.name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
