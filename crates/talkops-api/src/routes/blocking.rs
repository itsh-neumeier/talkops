//! Call blocking: own list (operator), anonymous callers and PhoneBlock
//! settings (admin).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::blocking::{self, BlockSettings, BlockSettingsInput, CallBlock, CallBlockInput};
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_blocks, create_block))
        .routes(routes!(delete_block))
        .routes(routes!(get_block_settings, update_block_settings))
        .routes(routes!(test_number))
}

/// Blocked numbers and prefixes.
#[utoipa::path(get, path = "/api/v1/call-blocks", tag = "routing", responses((status = 200, body = Vec<CallBlock>)))]
pub async fn list_blocks(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<CallBlock>>> {
    auth.require(Role::Operator)?;
    Ok(Json(blocking::list(&state.db, auth.tenant).await?))
}

/// Blocks a number (`+4930123456`) or prefix (`+49900*`).
#[utoipa::path(post, path = "/api/v1/call-blocks", tag = "routing", request_body = CallBlockInput, responses((status = 201, body = CallBlock)))]
pub async fn create_block(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CallBlockInput>,
) -> ApiResult<(StatusCode, Json<CallBlock>)> {
    auth.require(Role::Operator)?;
    let block = blocking::create(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "call_block",
        Some(block.id.to_string()),
        json!({"pattern": block.pattern, "label": block.label}),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(block)))
}

#[utoipa::path(delete, path = "/api/v1/call-blocks/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_block(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Operator)?;
    blocking::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "call_block",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/v1/call-blocks/settings", tag = "routing", responses((status = 200, body = BlockSettings)))]
pub async fn get_block_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<BlockSettings>> {
    auth.require(Role::Operator)?;
    Ok(Json(blocking::get_settings(&state.db, auth.tenant).await?))
}

/// Anonymous callers and PhoneBlock (the API token is stored encrypted).
#[utoipa::path(put, path = "/api/v1/call-blocks/settings", tag = "routing", request_body = BlockSettingsInput, responses((status = 200, body = BlockSettings)))]
pub async fn update_block_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<BlockSettingsInput>,
) -> ApiResult<Json<BlockSettings>> {
    auth.require(Role::Admin)?;
    let settings =
        blocking::update_settings(&state.db, auth.tenant, &input, &state.secrets).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "call_block_settings",
        None,
        json!({
            "block_anonymous": settings.block_anonymous,
            "phoneblock_enabled": settings.phoneblock_enabled,
            "phoneblock_min_votes": settings.phoneblock_min_votes,
            "token_changed": input.phoneblock_token.is_some(),
        }),
    )
    .await?;
    Ok(Json(settings))
}

#[derive(Deserialize, ToSchema)]
pub struct TestRequest {
    /// International number, e.g. `+4930123456`.
    pub number: String,
}

#[derive(Serialize, ToSchema)]
pub struct TestResult {
    pub blocked: bool,
    /// `anonymous`, `list:<pattern>` or `phoneblock:<votes>`.
    pub reason: Option<String>,
}

/// Would a call from this number be blocked?
#[utoipa::path(post, path = "/api/v1/call-blocks/test", tag = "routing", request_body = TestRequest, responses((status = 200, body = TestResult)))]
pub async fn test_number(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TestRequest>,
) -> ApiResult<Json<TestResult>> {
    auth.require(Role::Operator)?;
    let number = blocking::normalize_pattern(&req.number)
        .ok()
        .filter(|n| !n.ends_with('*'))
        .ok_or_else(|| {
            ApiError::BadRequest("number must be international, e.g. +4930123456".into())
        })?;
    let blocked = state
        .spam
        .check(&state.db, auth.tenant, Some(&number))
        .await?;
    Ok(Json(TestResult {
        blocked: blocked.is_some(),
        reason: blocked.map(|b| b.reason()),
    }))
}
