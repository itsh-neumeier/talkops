//! Call queues.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::json;
use talkops_core::queues::{self, Queue, QueueInput};
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
        .routes(routes!(list_queues, create_queue))
        .routes(routes!(get_queue, update_queue, delete_queue))
}

/// Lists call queues (operator or admin).
#[utoipa::path(get, path = "/api/v1/queues", tag = "routing", responses((status = 200, body = [Queue])))]
pub async fn list_queues(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Queue>>> {
    auth.require(Role::Operator)?;
    Ok(Json(queues::list(&state.db, auth.tenant).await?))
}

async fn save(
    state: &AppState,
    auth: &AuthUser,
    id: Option<Uuid>,
    input: &QueueInput,
) -> ApiResult<Queue> {
    let emergency = settings::get(&state.db, auth.tenant)
        .await?
        .emergency_numbers;
    let queue = queues::save(&state.db, auth.tenant, id, input, &emergency).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        if id.is_some() { "update" } else { "create" },
        "queue",
        Some(queue.id.to_string()),
        json!({"name": queue.name, "number": queue.number, "members": queue.members.len()}),
    )
    .await?;
    state.queue_sync.notify_one();
    Ok(queue)
}

/// Creates a call queue (admin).
#[utoipa::path(post, path = "/api/v1/queues", tag = "routing", request_body = QueueInput, responses((status = 200, body = Queue)))]
pub async fn create_queue(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<QueueInput>,
) -> ApiResult<Json<Queue>> {
    auth.require(Role::Admin)?;
    Ok(Json(save(&state, &auth, None, &input).await?))
}

/// Returns a call queue (operator or admin).
#[utoipa::path(get, path = "/api/v1/queues/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 200, body = Queue)))]
pub async fn get_queue(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Queue>> {
    auth.require(Role::Operator)?;
    Ok(Json(queues::get(&state.db, auth.tenant, id).await?))
}

/// Updates a call queue (admin).
#[utoipa::path(put, path = "/api/v1/queues/{id}", tag = "routing", params(("id" = Uuid, Path)), request_body = QueueInput, responses((status = 200, body = Queue)))]
pub async fn update_queue(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<QueueInput>,
) -> ApiResult<Json<Queue>> {
    auth.require(Role::Admin)?;
    Ok(Json(save(&state, &auth, Some(id), &input).await?))
}

/// Deletes a call queue (admin).
#[utoipa::path(delete, path = "/api/v1/queues/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_queue(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let queue = queues::get(&state.db, auth.tenant, id).await?;
    queues::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "queue",
        Some(id.to_string()),
        json!({"name": queue.name}),
    )
    .await?;
    state.queue_sync.notify_one();
    Ok(StatusCode::NO_CONTENT)
}
