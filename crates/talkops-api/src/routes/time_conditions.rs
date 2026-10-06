//! Time conditions and the public holiday calendar.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::holidays::{self, Holiday};
use talkops_core::time_conditions::{self, State as TcState, TimeCondition, TimeConditionInput};
use talkops_core::users::Role;
use talkops_core::{audit, settings};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_conditions, create_condition))
        .routes(routes!(get_condition, update_condition, delete_condition))
        .routes(routes!(set_override))
        .routes(routes!(list_holidays))
}

/// A time condition with its current state.
#[derive(Serialize, ToSchema)]
pub struct TimeConditionView {
    #[serde(flatten)]
    pub condition: TimeCondition,
    pub state: TcState,
}

async fn view(
    state: &AppState,
    auth: &AuthUser,
    tc: TimeCondition,
) -> ApiResult<TimeConditionView> {
    let zone = settings::get(&state.db, auth.tenant).await?.timezone;
    Ok(TimeConditionView {
        state: tc.state(chrono::Utc::now(), &zone),
        condition: tc,
    })
}

/// Lists time conditions with their current state (operator or admin).
#[utoipa::path(get, path = "/api/v1/time-conditions", tag = "routing", responses((status = 200, body = [TimeConditionView])))]
pub async fn list_conditions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<TimeConditionView>>> {
    auth.require(Role::Operator)?;
    let mut out = Vec::new();
    for tc in time_conditions::list(&state.db, auth.tenant).await? {
        out.push(view(&state, &auth, tc).await?);
    }
    Ok(Json(out))
}

/// Creates a time condition (admin).
#[utoipa::path(post, path = "/api/v1/time-conditions", tag = "routing", request_body = TimeConditionInput, responses((status = 200, body = TimeConditionView)))]
pub async fn create_condition(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TimeConditionInput>,
) -> ApiResult<Json<TimeConditionView>> {
    auth.require(Role::Admin)?;
    let emergency = settings::get(&state.db, auth.tenant)
        .await?
        .emergency_numbers;
    let tc = time_conditions::create(&state.db, auth.tenant, &input, &emergency).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "time_condition",
        Some(tc.id.to_string()),
        json!({"name": tc.name, "number": tc.number}),
    )
    .await?;
    Ok(Json(view(&state, &auth, tc).await?))
}

/// Returns a time condition (operator or admin).
#[utoipa::path(get, path = "/api/v1/time-conditions/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 200, body = TimeConditionView)))]
pub async fn get_condition(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TimeConditionView>> {
    auth.require(Role::Operator)?;
    let tc = time_conditions::get(&state.db, auth.tenant, id).await?;
    Ok(Json(view(&state, &auth, tc).await?))
}

/// Updates a time condition (admin).
#[utoipa::path(put, path = "/api/v1/time-conditions/{id}", tag = "routing", params(("id" = Uuid, Path)), request_body = TimeConditionInput, responses((status = 200, body = TimeConditionView)))]
pub async fn update_condition(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<TimeConditionInput>,
) -> ApiResult<Json<TimeConditionView>> {
    auth.require(Role::Admin)?;
    let emergency = settings::get(&state.db, auth.tenant)
        .await?
        .emergency_numbers;
    let tc = time_conditions::update(&state.db, auth.tenant, id, &input, &emergency).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "time_condition",
        Some(id.to_string()),
        json!({"name": tc.name, "number": tc.number}),
    )
    .await?;
    Ok(Json(view(&state, &auth, tc).await?))
}

/// Deletes a time condition (admin).
#[utoipa::path(delete, path = "/api/v1/time-conditions/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_condition(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let tc = time_conditions::get(&state.db, auth.tenant, id).await?;
    time_conditions::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "time_condition",
        Some(id.to_string()),
        json!({"name": tc.name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct OverrideInput {
    /// `auto`, `open` or `closed`.
    pub r#override: String,
}

/// Forces a time condition open or closed, or back to its schedule
/// (operator or admin – e.g. reception switching to the emergency service).
#[utoipa::path(put, path = "/api/v1/time-conditions/{id}/override", tag = "routing", params(("id" = Uuid, Path)), request_body = OverrideInput, responses((status = 200, body = TimeConditionView)))]
pub async fn set_override(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<OverrideInput>,
) -> ApiResult<Json<TimeConditionView>> {
    auth.require(Role::Operator)?;
    let tc = time_conditions::set_override(&state.db, auth.tenant, id, &input.r#override).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "override",
        "time_condition",
        Some(id.to_string()),
        json!({"name": tc.name, "override": tc.r#override}),
    )
    .await?;
    Ok(Json(view(&state, &auth, tc).await?))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct HolidayQuery {
    /// `DE` or `DE-XX`; omitted: list the regions.
    pub region: Option<String>,
    pub year: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct HolidayCalendar {
    /// Available regions as (code, name).
    pub regions: Vec<(String, String)>,
    pub holidays: Vec<Holiday>,
}

/// Public holidays of a region and year (any logged-in user).
#[utoipa::path(get, path = "/api/v1/holidays", tag = "routing", params(HolidayQuery), responses((status = 200, body = HolidayCalendar)))]
pub async fn list_holidays(
    _auth: AuthUser,
    Query(q): Query<HolidayQuery>,
) -> ApiResult<Json<HolidayCalendar>> {
    let year = q
        .year
        .unwrap_or_else(|| chrono::Datelike::year(&chrono::Utc::now()));
    if !(1990..=2100).contains(&year) {
        return Err(ApiError::BadRequest("year out of range".into()));
    }
    let holidays = match q.region.as_deref() {
        Some(r) if holidays::is_region(r) => holidays::holidays(r, year),
        Some(r) => return Err(ApiError::BadRequest(format!("unknown region `{r}`"))),
        None => Vec::new(),
    };
    Ok(Json(HolidayCalendar {
        regions: holidays::REGIONS
            .iter()
            .map(|(c, n)| (c.to_string(), n.to_string()))
            .collect(),
        holidays,
    }))
}
