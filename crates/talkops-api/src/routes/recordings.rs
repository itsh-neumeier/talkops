//! Call recordings, transcripts and transcript search.
//!
//! Recordings are content, not metadata: users reach the recordings of
//! calls with their own extensions, admins all of them (audited when the
//! call is not their own). Operators are treated like users here.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;
use talkops_core::recordings::{self, Recording, SearchHit, Source, Transcript};
use talkops_core::users::Role;
use talkops_core::{audit, extensions};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_recording, delete_recording))
        .routes(routes!(recording_audio))
        .routes(routes!(recording_transcript))
        .routes(routes!(search))
}

/// Extensions of the logged-in user.
async fn own_extensions(state: &AppState, auth: &AuthUser) -> ApiResult<Vec<Uuid>> {
    Ok(extensions::list_for_user(&state.db, auth.tenant, auth.id)
        .await?
        .into_iter()
        .map(|e| e.id)
        .collect())
}

/// Loads a recording the user may access; `true` if it is one of their calls.
async fn accessible(state: &AppState, auth: &AuthUser, id: Uuid) -> ApiResult<(Recording, bool)> {
    let rec = recordings::get(&state.db, auth.tenant, id).await?;
    let involved = recordings::extensions_of(&state.db, rec.id).await?;
    let own = own_extensions(state, auth).await?;
    let mine = involved.iter().any(|e| own.contains(e));
    if mine || auth.role >= Role::Admin {
        Ok((rec, mine))
    } else {
        // Do not reveal that the recording exists.
        Err(ApiError::NotFound)
    }
}

async fn audit_access(state: &AppState, auth: &AuthUser, rec: &Recording, action: &str) {
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        action,
        "recording",
        Some(rec.id.to_string()),
        json!({"call_uuid": rec.call_uuid}),
    )
    .await;
}

/// A recording's metadata.
#[utoipa::path(get, path = "/api/v1/recordings/{id}", tag = "recordings", params(("id" = Uuid, Path)), responses((status = 200, body = Recording)))]
pub async fn get_recording(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Recording>> {
    Ok(Json(accessible(&state, &auth, id).await?.0))
}

/// The recording as stereo WAV (left: caller, right: called party).
#[utoipa::path(get, path = "/api/v1/recordings/{id}/audio", tag = "recordings", params(("id" = Uuid, Path)), responses((status = 200, content_type = "audio/wav", body = Vec<u8>)))]
pub async fn recording_audio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let (rec, mine) = accessible(&state, &auth, id).await?;
    if !mine {
        audit_access(&state, &auth, &rec, "listen").await;
    }
    super::voicemail::wav(state.media.recordings.join(&rec.file)).await
}

/// The recording's transcript.
#[utoipa::path(get, path = "/api/v1/recordings/{id}/transcript", tag = "recordings", params(("id" = Uuid, Path)), responses((status = 200, body = Transcript)))]
pub async fn recording_transcript(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Transcript>> {
    let (rec, mine) = accessible(&state, &auth, id).await?;
    if !mine {
        audit_access(&state, &auth, &rec, "read_transcript").await;
    }
    Ok(Json(
        recordings::transcript_of(&state.db, auth.tenant, Source::Recording(rec.id)).await?,
    ))
}

/// Deletes a recording, its file and transcript (admin).
#[utoipa::path(delete, path = "/api/v1/recordings/{id}", tag = "recordings", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_recording(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let rec = recordings::delete(&state.db, auth.tenant, id).await?;
    if let Err(err) = tokio::fs::remove_file(state.media.recordings.join(&rec.file)).await {
        tracing::warn!(error = %err, file = %rec.file, "cannot remove recording file");
    }
    audit_access(&state, &auth, &rec, "delete").await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    /// Search terms (web search syntax: `"exact phrase"`, `-without`, `or`).
    pub q: String,
    pub limit: Option<i64>,
}

/// Full-text search in call and voicemail transcripts. Admins search
/// everything, users the calls and voicemails of their own extensions.
#[utoipa::path(get, path = "/api/v1/search", tag = "recordings", params(SearchQuery), responses((status = 200, body = [SearchHit])))]
pub async fn search(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<Vec<SearchHit>>> {
    let own;
    let scope = if auth.role >= Role::Admin {
        None
    } else {
        own = own_extensions(&state, &auth).await?;
        Some(own.as_slice())
    };
    Ok(Json(
        recordings::search(&state.db, auth.tenant, &q.q, scope, q.limit.unwrap_or(50)).await?,
    ))
}
