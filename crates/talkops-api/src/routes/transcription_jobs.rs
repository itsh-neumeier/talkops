//! Transcription jobs: the list with progress and estimates, pause,
//! resume, stop and priority, and the resource usage of the media workers.
//! `transcriptions.manage` covers all jobs; `transcriptions.view` the jobs
//! that the user started or of their own extensions.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::roles::Permission;
use talkops_core::transcription_jobs::{self as tjobs, Action, JobView, Scope, WorkerStatus};
use talkops_core::{audit, extensions};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::active_calls::{Directory, JobParties};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_jobs))
        .routes(routes!(act_on_job))
        .routes(routes!(list_workers))
}

async fn scope(state: &AppState, auth: &AuthUser) -> ApiResult<Scope> {
    if auth.can(Permission::TranscriptionsManage) {
        return Ok(Scope::All);
    }
    auth.require_perm(Permission::TranscriptionsView)?;
    Ok(Scope::Own {
        user: auth.id,
        extensions: extensions::list_for_user(&state.db, auth.tenant, auth.id)
            .await?
            .into_iter()
            .map(|e| e.id)
            .collect(),
    })
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct JobsQuery {
    /// `true`: finished, failed and stopped jobs of the last 7 days (at most
    /// 1000), with the parties resolved through the phone book.
    #[serde(default)]
    pub history: bool,
}

/// A transcription job in the list. In the history it also has the parties
/// of the call or voicemail, resolved like the call log.
#[derive(Serialize, ToSchema)]
pub struct JobEntry {
    #[serde(flatten)]
    pub job: JobView,
    #[serde(flatten)]
    pub parties: JobParties,
}

/// Transcription jobs: running, paused and queued, or the history.
#[utoipa::path(get, path = "/api/v1/transcription-jobs", tag = "recordings", params(JobsQuery), responses((status = 200, body = [JobEntry])))]
pub async fn list_jobs(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<JobsQuery>,
) -> ApiResult<Json<Vec<JobEntry>>> {
    let scope = scope(&state, &auth).await?;
    let jobs = tjobs::list(&state.db, auth.tenant, &scope, q.history).await?;
    // The active list refreshes every 2 s; only the history resolves names.
    let directory = if q.history && !jobs.is_empty() {
        Some(Directory::load(&state, auth.tenant).await?)
    } else {
        None
    };
    Ok(Json(
        jobs.into_iter()
            .map(|mut job| {
                let parties = directory
                    .as_ref()
                    .map(|d| d.job_parties(&job))
                    .unwrap_or_default();
                if job.direction.is_none() {
                    job.direction = parties.direction;
                }
                JobEntry { job, parties }
            })
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct JobAction {
    pub action: Action,
}

/// Pauses, resumes, stops or prioritizes a transcription job.
#[utoipa::path(post, path = "/api/v1/transcription-jobs/{id}", tag = "recordings", params(("id" = Uuid, Path)), request_body = JobAction, responses((status = 204)))]
pub async fn act_on_job(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<JobAction>,
) -> ApiResult<StatusCode> {
    let scope = scope(&state, &auth).await?;
    if matches!(scope, Scope::Own { .. })
        && !tjobs::list(&state.db, auth.tenant, &scope, false)
            .await?
            .iter()
            .any(|j| j.id == id)
    {
        return Err(ApiError::NotFound);
    }
    tjobs::act(&state.db, auth.tenant, id, input.action).await?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "control",
        "transcription_job",
        Some(id.to_string()),
        json!({"action": format!("{:?}", input.action).to_lowercase()}),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Resource usage of the media workers (reported in the last minute).
#[utoipa::path(get, path = "/api/v1/transcription-workers", tag = "recordings", responses((status = 200, body = [WorkerStatus])))]
pub async fn list_workers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<WorkerStatus>>> {
    auth.require_perm(Permission::TranscriptionsManage)?;
    Ok(Json(tjobs::workers(&state.db).await?))
}
