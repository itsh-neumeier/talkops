//! Smart Attendants: call flows for incoming calls (destination kind `ivr`).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::json;
use talkops_core::attendant::{self, Attendant, AttendantInput};
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
        .routes(routes!(list_attendants, create_attendant))
        .routes(routes!(get_attendant, update_attendant, delete_attendant))
}

/// Lists Smart Attendants (operator or admin).
#[utoipa::path(get, path = "/api/v1/attendants", tag = "routing", responses((status = 200, body = [Attendant])))]
pub async fn list_attendants(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Attendant>>> {
    auth.require(Role::Operator)?;
    Ok(Json(attendant::list(&state.db, auth.tenant).await?))
}

async fn save(
    state: &AppState,
    auth: &AuthUser,
    id: Option<Uuid>,
    input: &AttendantInput,
) -> ApiResult<Attendant> {
    let s = settings::get(&state.db, auth.tenant).await?;
    let saved = attendant::save(&state.db, auth.tenant, id, input, &s.emergency_numbers).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        if id.is_some() { "update" } else { "create" },
        "attendant",
        Some(saved.id.to_string()),
        json!({"name": saved.name, "number": saved.number, "steps": saved.flow.walk().len()}),
    )
    .await?;
    Ok(saved)
}

/// Creates a Smart Attendant (admin).
#[utoipa::path(post, path = "/api/v1/attendants", tag = "routing", request_body = AttendantInput, responses((status = 200, body = Attendant)))]
pub async fn create_attendant(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<AttendantInput>,
) -> ApiResult<Json<Attendant>> {
    auth.require(Role::Admin)?;
    Ok(Json(save(&state, &auth, None, &input).await?))
}

/// Returns a Smart Attendant (operator or admin).
#[utoipa::path(get, path = "/api/v1/attendants/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 200, body = Attendant)))]
pub async fn get_attendant(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Attendant>> {
    auth.require(Role::Operator)?;
    Ok(Json(attendant::get(&state.db, auth.tenant, id).await?))
}

/// Updates a Smart Attendant (admin).
#[utoipa::path(put, path = "/api/v1/attendants/{id}", tag = "routing", params(("id" = Uuid, Path)), request_body = AttendantInput, responses((status = 200, body = Attendant)))]
pub async fn update_attendant(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<AttendantInput>,
) -> ApiResult<Json<Attendant>> {
    auth.require(Role::Admin)?;
    Ok(Json(save(&state, &auth, Some(id), &input).await?))
}

/// Deletes a Smart Attendant (admin).
#[utoipa::path(delete, path = "/api/v1/attendants/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_attendant(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let old = attendant::get(&state.db, auth.tenant, id).await?;
    attendant::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "attendant",
        Some(id.to_string()),
        json!({"name": old.name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
