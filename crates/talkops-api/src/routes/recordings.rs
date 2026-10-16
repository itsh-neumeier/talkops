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
use talkops_core::transcription_jobs::{self as tjobs, EngineChoice, Scope};
use talkops_core::transcription_profiles::{self as profiles, Purpose};
use talkops_core::{audit, extensions};
use talkops_core::{settings::TranscriptionQuality, transcription_api};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use talkops_core::roles::Permission;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_recording, delete_recording))
        .routes(routes!(recording_audio))
        .routes(routes!(recording_transcript))
        .routes(routes!(search))
        .routes(routes!(transcribe_options, transcribe_again))
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
    if mine || auth.can(Permission::RecordingsAccessAll) {
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
    auth.require_perm(Permission::RecordingsDelete)?;
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
    let scope = if auth.can(Permission::RecordingsAccessAll) {
        None
    } else {
        own = own_extensions(&state, &auth).await?;
        Some(own.as_slice())
    };
    Ok(Json(
        recordings::search(&state.db, auth.tenant, &q.q, scope, q.limit.unwrap_or(50)).await?,
    ))
}

/// A model that can transcribe a recording again.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TranscribeOption {
    pub choice: EngineChoice,
    /// Cloud connection name; empty for local models.
    pub provider: String,
    /// Estimated processing time in seconds (without the queue).
    pub eta_secs: f32,
    /// The estimate comes from measured runs on this server.
    pub measured: bool,
    /// Estimated cost in USD (cloud models with a known price).
    pub cost_usd: Option<f64>,
    /// The engine of the automatic first pass.
    pub default: bool,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TranscribeOptions {
    pub audio_secs: i32,
    /// Estimated wait for the jobs before a new one.
    pub queue_secs: f32,
    /// Engine of the current transcript, e.g. `whisper:large-v3-turbo-q5_0`.
    pub current_engine: Option<String>,
    pub options: Vec<TranscribeOption>,
}

/// Cloud models that can transcribe a file (not the realtime models).
fn file_models(kind: &str) -> Vec<String> {
    if kind != "openai" {
        return Vec::new();
    }
    profiles::OPENAI_PRICES
        .iter()
        .map(|(m, _)| *m)
        .filter(|m| !m.contains("live") && !m.contains("realtime") && !m.contains("diarize"))
        .map(str::to_owned)
        .collect()
}

/// Models to transcribe a recording again, with time and cost estimates.
#[utoipa::path(get, path = "/api/v1/recordings/{id}/transcribe", tag = "recordings", params(("id" = Uuid, Path)), responses((status = 200, body = TranscribeOptions)))]
pub async fn transcribe_options(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TranscribeOptions>> {
    auth.require_perm(Permission::RecordingsRetranscribe)?;
    let (rec, _) = accessible(&state, &auth, id).await?;
    let speeds = tjobs::speeds(&state.db).await?;
    let default = profiles::engine(&state.db, auth.tenant, Purpose::Recording)
        .await?
        .map(|e| tjobs::engine_key(&e));
    let profile_list = profiles::list(&state.db, auth.tenant).await?;
    let audio = rec.duration_secs.max(1) as f32;
    let mut options = Vec::new();
    let mut push = |choice: EngineChoice, provider: String, price: Option<f64>| {
        let key = choice.key();
        let measured = speeds.get(&key).copied();
        options.push(TranscribeOption {
            eta_secs: audio * measured.unwrap_or_else(|| tjobs::default_speed(&key)),
            measured: measured.is_some(),
            cost_usd: price.map(|p| audio as f64 / 60.0 * p),
            default: default.as_deref() == Some(key.as_str()),
            choice,
            provider,
        });
    };
    for q in [
        TranscriptionQuality::Base,
        TranscriptionQuality::Fast,
        TranscriptionQuality::German,
        TranscriptionQuality::Accurate,
        TranscriptionQuality::Best,
    ] {
        push(
            EngineChoice::Local {
                model: q.as_str().to_owned(),
            },
            String::new(),
            None,
        );
    }
    for provider in transcription_api::list(&state.db, auth.tenant).await? {
        let mut models = file_models(&provider.kind);
        for p in profile_list
            .iter()
            .filter(|p| p.engine == "cloud" && p.provider_id == Some(provider.id) && !p.realtime)
        {
            if !models.contains(&p.model) {
                models.push(p.model.clone());
            }
        }
        for model in models {
            let price = profile_list
                .iter()
                .find(|p| p.provider_id == Some(provider.id) && p.model == model)
                .and_then(|p| p.price_per_minute)
                .or_else(|| profiles::list_price(&provider.kind, &model));
            push(
                EngineChoice::Cloud {
                    provider_id: provider.id,
                    model,
                },
                provider.name.clone(),
                price,
            );
        }
    }
    let active = tjobs::list(&state.db, auth.tenant, &Scope::All, false).await?;
    let queue_secs = active
        .iter()
        .filter_map(|j| j.eta_secs)
        .fold(0.0f32, f32::max);
    let current_engine =
        recordings::transcript_of(&state.db, auth.tenant, Source::Recording(rec.id))
            .await
            .ok()
            .map(|t| t.engine)
            .filter(|e| !e.is_empty());
    Ok(Json(TranscribeOptions {
        audio_secs: rec.duration_secs,
        queue_secs,
        current_engine,
        options,
    }))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct TranscribeRequest {
    pub engine: EngineChoice,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TranscribeStarted {
    pub job_id: Uuid,
}

/// Transcribes a recording again with the selected model. The current
/// transcript stays until the new one is complete.
#[utoipa::path(post, path = "/api/v1/recordings/{id}/transcribe", tag = "recordings", params(("id" = Uuid, Path)), request_body = TranscribeRequest, responses((status = 200, body = TranscribeStarted)))]
pub async fn transcribe_again(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<TranscribeRequest>,
) -> ApiResult<Json<TranscribeStarted>> {
    auth.require_perm(Permission::RecordingsRetranscribe)?;
    if !talkops_core::settings::get(&state.db, auth.tenant)
        .await?
        .transcription_enabled
    {
        return Err(ApiError::BadRequest("transcription is switched off".into()));
    }
    let (rec, _) = accessible(&state, &auth, id).await?;
    let job_id =
        tjobs::enqueue_manual(&state.db, auth.tenant, rec.id, &input.engine, auth.id).await?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "transcribe",
        "recording",
        Some(rec.id.to_string()),
        json!({"engine": input.engine.key(), "job": job_id}),
    )
    .await;
    Ok(Json(TranscribeStarted { job_id }))
}
