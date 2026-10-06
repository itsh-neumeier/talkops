//! Door stations: configuration (admin), opening the door, live picture and
//! event log (every user – a door opener is for the whole household or
//! office), and the token-protected hook for Home Assistant.

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::doors::{self, DoorEvent, DoorStation, DoorStationInput};
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::doors::DoorCtx;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_stations, create_station))
        .routes(routes!(get_station, update_station, delete_station))
        .routes(routes!(open_door))
        .routes(routes!(live_snapshot))
        .routes(routes!(test_station))
        .routes(routes!(new_token, revoke_token))
        .routes(routes!(list_events))
        .routes(routes!(event_snapshot))
}

/// Door stations (every user; connection details only for admins).
#[utoipa::path(get, path = "/api/v1/door-stations", tag = "doors", responses((status = 200, body = [DoorStation])))]
pub async fn list_stations(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<DoorStation>>> {
    let mut list = doors::list(&state.db, auth.tenant).await?;
    if auth.role < Role::Admin {
        for s in &mut list {
            s.host.clear();
            s.username.clear();
        }
        // Users only need to know whether the station can be opened.
        list.retain(|s| s.enabled);
    }
    Ok(Json(list))
}

/// Creates a door station (admin).
#[utoipa::path(post, path = "/api/v1/door-stations", tag = "doors", request_body = DoorStationInput, responses((status = 200, body = DoorStation)))]
pub async fn create_station(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<DoorStationInput>,
) -> ApiResult<Json<DoorStation>> {
    auth.require(Role::Admin)?;
    let station = doors::create(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "door_station",
        Some(station.id.to_string()),
        json!({"name": station.name, "host": station.host}),
    )
    .await?;
    Ok(Json(station))
}

/// A door station (admin).
#[utoipa::path(get, path = "/api/v1/door-stations/{id}", tag = "doors", params(("id" = Uuid, Path)), responses((status = 200, body = DoorStation)))]
pub async fn get_station(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DoorStation>> {
    auth.require(Role::Admin)?;
    Ok(Json(doors::get(&state.db, auth.tenant, id).await?))
}

/// Updates a door station (admin). Password and webhook: omit to keep.
#[utoipa::path(put, path = "/api/v1/door-stations/{id}", tag = "doors", params(("id" = Uuid, Path)), request_body = DoorStationInput, responses((status = 200, body = DoorStation)))]
pub async fn update_station(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<DoorStationInput>,
) -> ApiResult<Json<DoorStation>> {
    auth.require(Role::Admin)?;
    let station = doors::update(&state.db, auth.tenant, &state.secrets, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "door_station",
        Some(id.to_string()),
        json!({"name": station.name, "host": station.host,
               "password_changed": input.password.is_some(),
               "webhook_changed": input.webhook_url.is_some()}),
    )
    .await?;
    Ok(Json(station))
}

/// Deletes a door station and its event log (admin). The extension stays.
#[utoipa::path(delete, path = "/api/v1/door-stations/{id}", tag = "doors", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_station(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let station = doors::get(&state.db, auth.tenant, id).await?;
    let files = doors::delete(&state.db, auth.tenant, id).await?;
    crate::doors::remove_files(state.media.snapshots.clone(), files).await;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "door_station",
        Some(id.to_string()),
        json!({"name": station.name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct OpenInput {
    /// 1 (default) or 2 (second lock).
    #[serde(default = "first_door")]
    pub door: i16,
}

fn first_door() -> i16 {
    1
}

/// Opens a door (every user).
#[utoipa::path(post, path = "/api/v1/door-stations/{id}/open", tag = "doors", params(("id" = Uuid, Path)), request_body = OpenInput, responses((status = 204), (status = 502, description = "the door station failed")))]
pub async fn open_door(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<OpenInput>,
) -> ApiResult<StatusCode> {
    let station = doors::get(&state.db, auth.tenant, id).await?;
    if !station.enabled {
        return Err(ApiError::NotFound);
    }
    let result = DoorCtx::from(&state)
        .open(
            auth.tenant,
            &station,
            input.door,
            json!({ "user": auth.username }),
        )
        .await;
    audit::record(
        &state.db,
        &auth.actor(),
        "open",
        "door_station",
        Some(id.to_string()),
        json!({"name": station.name, "door": input.door, "ok": result.is_ok()}),
    )
    .await?;
    result?;
    Ok(StatusCode::NO_CONTENT)
}

fn jpeg(bytes: impl Into<Body>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes.into(),
    )
        .into_response()
}

/// Current camera picture (every user).
#[utoipa::path(get, path = "/api/v1/door-stations/{id}/snapshot", tag = "doors", params(("id" = Uuid, Path)), responses((status = 200, content_type = "image/jpeg", body = Vec<u8>)))]
pub async fn live_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let station = doors::get(&state.db, auth.tenant, id).await?;
    if !station.enabled {
        return Err(ApiError::NotFound);
    }
    Ok(jpeg(DoorCtx::from(&state).snapshot(id).await?))
}

#[derive(Serialize, ToSchema)]
pub struct TestResult {
    /// Device type reported by the station, e.g. `VTO2202F-P`.
    pub model: String,
}

/// Checks the HTTP access of a station (admin).
#[utoipa::path(post, path = "/api/v1/door-stations/{id}/test", tag = "doors", params(("id" = Uuid, Path)), responses((status = 200, body = TestResult)))]
pub async fn test_station(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TestResult>> {
    auth.require(Role::Admin)?;
    doors::get(&state.db, auth.tenant, id).await?;
    let model = DoorCtx::from(&state).test(id).await?;
    Ok(Json(TestResult { model }))
}

#[derive(Serialize, ToSchema)]
pub struct NewToken {
    /// Shown only once; send as `Authorization: Bearer <token>`.
    pub token: String,
}

/// Creates a new token for the open hook, replacing the old one (admin).
#[utoipa::path(post, path = "/api/v1/door-stations/{id}/token", tag = "doors", params(("id" = Uuid, Path)), responses((status = 200, body = NewToken)))]
pub async fn new_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<NewToken>> {
    auth.require(Role::Admin)?;
    let token = doors::new_api_token(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "new_token",
        "door_station",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    Ok(Json(NewToken { token }))
}

/// Revokes the open hook token (admin).
#[utoipa::path(delete, path = "/api/v1/door-stations/{id}/token", tag = "doors", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn revoke_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    doors::revoke_api_token(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "revoke_token",
        "door_station",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct EventQuery {
    pub door_station_id: Option<Uuid>,
    pub limit: Option<i64>,
}

/// Door event log, newest first (every user).
#[utoipa::path(get, path = "/api/v1/door-events", tag = "doors", params(EventQuery), responses((status = 200, body = [DoorEvent])))]
pub async fn list_events(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<EventQuery>,
) -> ApiResult<Json<Vec<DoorEvent>>> {
    Ok(Json(
        doors::list_events(
            &state.db,
            auth.tenant,
            q.door_station_id,
            q.limit.unwrap_or(50),
        )
        .await?,
    ))
}

/// The snapshot taken for an event (every user).
#[utoipa::path(get, path = "/api/v1/door-events/{id}/snapshot", tag = "doors", params(("id" = Uuid, Path)), responses((status = 200, content_type = "image/jpeg", body = Vec<u8>)))]
pub async fn event_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let event = doors::get_event(&state.db, auth.tenant, id).await?;
    let file = event.snapshot.ok_or(ApiError::NotFound)?;
    let bytes = tokio::fs::read(state.media.snapshots.join(file))
        .await
        .map_err(|_| ApiError::NotFound)?;
    Ok(jpeg(bytes))
}

#[derive(Deserialize, Default)]
pub struct HookInput {
    #[serde(default)]
    pub door: Option<i16>,
}

/// `POST /hooks/door/{id}/open` with `Authorization: Bearer <token>`:
/// opens the door for Home Assistant and similar automations.
pub async fn hook_open(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Option<Json<HookInput>>,
) -> Response {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();
    if token.is_empty() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let station = match doors::by_api_token(&state.db, id, token).await {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(err) => return ApiError::from(err).into_response(),
    };
    let (tenant, station) = station;
    let door = body.and_then(|Json(b)| b.door).unwrap_or(1);
    match DoorCtx::from(&state)
        .open(tenant, &station, door, json!({ "token": true }))
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}
