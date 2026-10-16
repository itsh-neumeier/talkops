//! First-setup preflight. Actual restore remains an offline host operation.
use super::auth::RequestMeta;
use super::backup_targets::{BrowseResult, DraftConnection};
use crate::{
    AppState,
    backup::{BackupConfig, Manifest},
    error::{ApiError, ApiResult},
    recovery,
};
use axum::response::{IntoResponse, Response};
use axum::{
    Json,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{HeaderMap, header},
};
use base64::Engine;
use talkops_core::backup_targets::{TargetConnection, TargetSecret};
use talkops_core::{tenant::TenantId, users};
use tokio::io::AsyncWriteExt;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct RestoreInspection {
    pub manifest: Manifest,
    /// Base64 gzip bundle for explicit download. Contains deployment secrets.
    pub deployment_bundle: Option<String>,
    /// Archive name in the backup directory (archives from a backup target).
    pub archive: Option<String>,
}
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(inspect))
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
        .merge(
            OpenApiRouter::new()
                .routes(routes!(browse_target))
                .routes(routes!(inspect_target)),
        )
}

/// Inspection limit: staging uses the temporary directory of the container.
const INSPECT_LIMIT: u64 = 8 * 1024 * 1024 * 1024;

/// Checks the setup token. Wrong tokens count against a rate limit per
/// client; correct ones do not, so the folder browser stays usable.
fn check_token(state: &AppState, meta: &RequestMeta, headers: &HeaderMap) -> ApiResult<()> {
    let key = format!("restore:{}", meta.ip.as_deref().unwrap_or("unknown"));
    if state.limiter.blocked(&key) {
        return Err(ApiError::TooManyRequests);
    }
    let expected = state.setup_token.as_deref().ok_or(ApiError::Forbidden)?;
    let sent = headers
        .get("x-talkops-setup-token")
        .and_then(|s| s.to_str().ok())
        .unwrap_or_default();
    if !crate::util::constant_time_eq(sent.as_bytes(), expected.as_bytes()) {
        state.limiter.allow(&key);
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Holds the setup lock while no account exists.
async fn setup_lock(state: &AppState) -> ApiResult<sqlx::Transaction<'static, sqlx::Postgres>> {
    let mut tx = state.db.begin().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(7231001)")
        .fetch_one(&mut *tx)
        .await?;
    if !locked {
        return Err(ApiError::Conflict("setup is already running".into()));
    }
    if users::count(&mut *tx, TenantId::DEFAULT).await? != 0 {
        return Err(ApiError::Conflict("setup already completed".into()));
    }
    Ok(tx)
}

/// Ends the setup transaction: commit on success, otherwise an explicit
/// rollback, so the lock is free for the next attempt (a dropped
/// transaction releases it only when its connection is used again).
async fn end_setup<T>(
    tx: sqlx::Transaction<'static, sqlx::Postgres>,
    result: ApiResult<T>,
) -> ApiResult<T> {
    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(err) => {
            let _ = tx.rollback().await;
            Err(err)
        }
    }
}

fn backup_config(state: &AppState) -> ApiResult<&std::sync::Arc<BackupConfig>> {
    state
        .backup
        .as_ref()
        .ok_or_else(|| ApiError::Conflict("backups are not configured".into()))
}

/// A connection entered in the restore assistant. Nothing is stored.
fn setup_connection(cfg: &BackupConfig, mut draft: DraftConnection) -> ApiResult<TargetConnection> {
    draft.id = None;
    draft.resolve(cfg, TargetSecret::default())
}

#[utoipa::path(post, path = "/api/v1/setup/restore/inspect", tag = "auth", responses((status = 200, body = RestoreInspection), (status = 403), (status = 409)))]
pub async fn inspect(
    State(state): State<AppState>,
    meta: RequestMeta,
    headers: HeaderMap,
    multipart: Multipart,
) -> ApiResult<Response> {
    check_token(&state, &meta, &headers)?;
    let cfg = backup_config(&state)?;
    let tx = setup_lock(&state).await?;
    let result = inspect_upload(cfg, multipart).await;
    let result = end_setup(tx, result).await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(result)).into_response())
}

async fn inspect_upload(
    cfg: &BackupConfig,
    mut multipart: Multipart,
) -> ApiResult<RestoreInspection> {
    let upload = tempfile::tempdir().map_err(|e| ApiError::Internal(e.to_string()))?;
    let archive = upload.path().join("upload");
    let mut password = None;
    let mut got_archive = false;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("invalid archive upload".into()))?
    {
        match field.name() {
            Some("password") if password.is_none() => {
                let mut bytes = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|_| ApiError::BadRequest("invalid password field".into()))?
                {
                    if bytes.len() + chunk.len() > 4096 {
                        return Err(ApiError::BadRequest("password too large".into()));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                password = Some(
                    String::from_utf8(bytes)
                        .map_err(|_| ApiError::BadRequest("invalid password".into()))?,
                );
            }
            Some("archive") if !got_archive => {
                got_archive = true;
                let file = recovery::private_file(&archive)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                let mut out = tokio::fs::File::from_std(file);
                let mut size = 0usize;
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|_| ApiError::BadRequest("archive upload failed".into()))?
                {
                    size += chunk.len();
                    if size > 512 * 1024 * 1024 {
                        return Err(ApiError::BadRequest(
                            "use the host tool for archives larger than 512 MiB".into(),
                        ));
                    }
                    out.write_all(&chunk)
                        .await
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                }
                out.flush()
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            _ => {
                return Err(ApiError::BadRequest(
                    "unexpected or duplicate upload field".into(),
                ));
            }
        }
    }
    if !got_archive {
        return Err(ApiError::BadRequest("select an archive".into()));
    }
    let password = recovery::password(password.as_deref(), None)?;
    let limit = cfg.restore_max_bytes.min(INSPECT_LIMIT);
    let result = tokio::task::spawn_blocking(move || {
        let prepared = recovery::prepare(&archive, password.as_ref(), limit)?;
        let bundle = prepared
            .bundle()?
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b));
        drop(upload);
        Ok::<_, crate::backup::BackupError>(RestoreInspection {
            manifest: prepared.manifest,
            deployment_bundle: bundle,
            archive: None,
        })
    })
    .await
    .map_err(|_| ApiError::BadRequest("archive inspection failed".into()))??;
    Ok(result)
}

/// Lists a folder of a backup target before setup (setup token). The
/// connection data is not stored.
#[utoipa::path(post, path = "/api/v1/setup/restore/target/browse", tag = "auth", request_body = DraftConnection, responses((status = 200, body = BrowseResult), (status = 403), (status = 409)))]
pub async fn browse_target(
    State(state): State<AppState>,
    meta: RequestMeta,
    headers: HeaderMap,
    Json(draft): Json<DraftConnection>,
) -> ApiResult<Json<BrowseResult>> {
    check_token(&state, &meta, &headers)?;
    let cfg = backup_config(&state)?;
    let conn = setup_connection(cfg, draft)?;
    let tx = setup_lock(&state).await?;
    let result = super::backup_targets::browse(cfg, conn).await;
    Ok(Json(end_setup(tx, result).await?))
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct TargetArchive {
    /// The folder with the archive.
    pub connection: DraftConnection,
    /// Archive name, e.g. `talkops-20261015-033000.tar.gz.age`.
    pub name: String,
    /// Backup password of an encrypted archive.
    #[serde(default)]
    pub password: Option<String>,
}

/// Downloads an archive from a backup target into the backup directory
/// and inspects it (setup token). An archive that is already there is
/// used again, e.g. after a wrong password. The offline restore then uses
/// it with `recover.sh restore --backup NAME`.
#[utoipa::path(post, path = "/api/v1/setup/restore/target/inspect", tag = "auth", request_body = TargetArchive, responses((status = 200, body = RestoreInspection), (status = 403), (status = 409)))]
pub async fn inspect_target(
    State(state): State<AppState>,
    meta: RequestMeta,
    headers: HeaderMap,
    Json(input): Json<TargetArchive>,
) -> ApiResult<Response> {
    check_token(&state, &meta, &headers)?;
    let cfg = backup_config(&state)?.clone();
    if !crate::backup::valid_name(&input.name) {
        return Err(ApiError::BadRequest("not a backup archive name".into()));
    }
    if input.password.as_ref().is_some_and(|p| p.len() > 4096) {
        return Err(ApiError::BadRequest("password too large".into()));
    }
    let conn = setup_connection(&cfg, input.connection)?;
    let password = recovery::password(input.password.as_deref(), None)?;
    let tx = setup_lock(&state).await?;
    let result = inspect_from_target(cfg, conn, input.name, password).await;
    let result = end_setup(tx, result).await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(result)).into_response())
}

async fn inspect_from_target(
    cfg: std::sync::Arc<BackupConfig>,
    conn: TargetConnection,
    name: String,
    password: Option<age::secrecy::SecretString>,
) -> ApiResult<RestoreInspection> {
    let limit = cfg.restore_max_bytes.min(INSPECT_LIMIT);
    let archive = cfg.dir.join(&name);
    if !archive.exists() {
        crate::backup_targets::fetch(&cfg.rclone, &conn, &name, &cfg.dir, limit).await?;
    }
    let result = tokio::task::spawn_blocking(move || {
        let prepared = recovery::prepare(&archive, password.as_ref(), limit)?;
        let bundle = prepared
            .bundle()?
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b));
        Ok::<_, crate::backup::BackupError>(RestoreInspection {
            manifest: prepared.manifest,
            deployment_bundle: bundle,
            archive: Some(name),
        })
    })
    .await
    .map_err(|_| ApiError::BadRequest("archive inspection failed".into()))??;
    Ok(result)
}
