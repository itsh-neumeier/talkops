//! Authorized, on-demand previews of already recorded calls.

use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde_json::json;
use talkops_core::roles::Permission;
use talkops_core::{audit, extensions, live_transcripts, settings};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    AppState, active_calls,
    auth::AuthUser,
    error::{ApiError, ApiResult},
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(preview, stop))
}

#[utoipa::path(post, path = "/api/v1/telephony/calls/{uuid}/transcript", tag = "recordings", params(("uuid" = Uuid, Path)), responses((status = 200, body = live_transcripts::Preview)))]
pub async fn preview(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(uuid): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    auth.require_perm(Permission::CallsLiveTranscript)?;
    let call = active_calls::load(&state, auth.tenant, Some(uuid))
        .await?
        .into_iter()
        .find(|c| c.call.uuid == uuid.to_string())
        .ok_or(ApiError::NotFound)?;
    let own = extensions::list_for_user(&state.db, auth.tenant, auth.id).await?;
    let mine = own
        .iter()
        .any(|e| call.caller.extension_id == Some(e.id) || call.called.extension_id == Some(e.id));
    if !mine && !auth.can(Permission::CallsLiveTranscriptAll) {
        return Err(ApiError::NotFound);
    }
    if !call.transcript_available
        || !settings::get(&state.db, auth.tenant)
            .await?
            .transcription_enabled
    {
        return Err(ApiError::Conflict(
            "live transcription requires an answered recorded call and enabled transcription"
                .into(),
        ));
    }
    let (preview, created) = live_transcripts::request(
        &state.db,
        auth.tenant,
        auth.id,
        uuid,
        &call.metadata.recording,
        call.answered_at.ok_or(ApiError::NotFound)?,
    )
    .await?;
    if created {
        audit::record(
            &state.db,
            &auth.actor(),
            "read_live_transcript",
            "call",
            Some(uuid.to_string()),
            json!({"own": mine}),
        )
        .await?;
    }
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(preview)))
}

#[utoipa::path(delete, path = "/api/v1/telephony/calls/{uuid}/transcript", tag = "recordings", params(("uuid" = Uuid, Path)), responses((status = 204)))]
pub async fn stop(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(uuid): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::CallsLiveTranscript)?;
    live_transcripts::stop(&state.db, auth.tenant, auth.id, uuid).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "close_live_transcript",
        "call",
        Some(uuid.to_string()),
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
