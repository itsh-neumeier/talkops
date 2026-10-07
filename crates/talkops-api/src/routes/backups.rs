//! Backups (admin): schedule, run now, download and delete archives.
//! Restores run offline with `talkops restore` (see docs).

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::backups::{self, BackupSettings, BackupSettingsInput};
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::backup::{self, BackupConfig, BackupError, BackupFile};
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_backups, start_backup))
        .routes(routes!(update_backup_settings))
        .routes(routes!(download_backup, delete_backup))
}

impl From<BackupError> for ApiError {
    fn from(err: BackupError) -> Self {
        match err {
            BackupError::Busy => ApiError::Conflict(err.to_string()),
            BackupError::Invalid(m) => ApiError::BadRequest(m),
            other => ApiError::Internal(other.to_string()),
        }
    }
}

fn config(state: &AppState) -> ApiResult<&std::sync::Arc<BackupConfig>> {
    state
        .backup
        .as_ref()
        .ok_or_else(|| ApiError::Conflict("backups are not configured".into()))
}

#[derive(Serialize, ToSchema)]
pub struct BackupOverview {
    pub settings: BackupSettings,
    /// Archives, newest first.
    pub files: Vec<BackupFile>,
    /// A backup is being written right now.
    pub running: bool,
}

async fn overview(state: &AppState, auth: &AuthUser) -> ApiResult<BackupOverview> {
    let cfg = config(state)?;
    Ok(BackupOverview {
        settings: backups::get(&state.db, auth.tenant).await?,
        files: backup::list(&cfg.dir).await?,
        running: backup::is_busy(),
    })
}

/// Backup schedule, last run and stored archives.
#[utoipa::path(get, path = "/api/v1/backups", tag = "system", responses((status = 200, body = BackupOverview)))]
pub async fn list_backups(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<BackupOverview>> {
    auth.require(Role::Admin)?;
    Ok(Json(overview(&state, &auth).await?))
}

/// Starts a backup in the background (`202`); poll the list for the result.
#[utoipa::path(post, path = "/api/v1/backups", tag = "system", responses((status = 202), (status = 409)))]
pub async fn start_backup(State(state): State<AppState>, auth: AuthUser) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?.clone();
    if backup::is_busy() {
        return Err(BackupError::Busy.into());
    }
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "backup",
        None,
        json!({}),
    )
    .await;
    let (db, tenant) = (state.db.clone(), auth.tenant);
    tokio::spawn(async move {
        match backup::run_scheduled(&cfg, &db, tenant).await {
            Ok(name) => tracing::info!(file = %name, "backup written"),
            Err(err) => tracing::warn!(error = %err, "backup failed"),
        }
    });
    Ok(StatusCode::ACCEPTED)
}

/// Changes the backup schedule.
#[utoipa::path(put, path = "/api/v1/backups/settings", tag = "system", request_body = BackupSettingsInput, responses((status = 200, body = BackupSettings)))]
pub async fn update_backup_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<BackupSettingsInput>,
) -> ApiResult<Json<BackupSettings>> {
    auth.require(Role::Admin)?;
    let settings = backups::update(&state.db, auth.tenant, &input).await?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "backup_settings",
        None,
        json!({
            "enabled": settings.enabled,
            "hour": settings.hour,
            "keep": settings.keep,
            "include_recordings": settings.include_recordings,
        }),
    )
    .await;
    Ok(Json(settings))
}

fn checked_name(name: &str) -> ApiResult<&str> {
    if backup::valid_name(name) {
        Ok(name)
    } else {
        Err(ApiError::NotFound)
    }
}

/// Downloads an archive. It contains every secret TalkOps stores (encrypted
/// with `TALKOPS_SECRET_KEY`) and all recordings: keep it safe.
#[utoipa::path(get, path = "/api/v1/backups/{name}", tag = "system", params(("name" = String, Path)), responses((status = 200, content_type = "application/gzip", body = Vec<u8>)))]
pub async fn download_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(name): Path<String>,
) -> ApiResult<Response> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    let name = checked_name(&name)?;
    let file = tokio::fs::File::open(cfg.dir.join(name))
        .await
        .map_err(|_| ApiError::NotFound)?;
    let len = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .len();
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "download",
        "backup",
        Some(name.to_owned()),
        json!({}),
    )
    .await;
    Ok((
        [
            (header::CONTENT_TYPE, "application/gzip".to_owned()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(file)),
    )
        .into_response())
}

/// Deletes an archive.
#[utoipa::path(delete, path = "/api/v1/backups/{name}", tag = "system", params(("name" = String, Path)), responses((status = 204)))]
pub async fn delete_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(name): Path<String>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    let name = checked_name(&name)?;
    tokio::fs::remove_file(cfg.dir.join(name))
        .await
        .map_err(|_| ApiError::NotFound)?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "backup",
        Some(name.to_owned()),
        json!({}),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
