//! First-setup preflight. Actual restore remains an offline host operation.
use super::auth::RequestMeta;
use crate::{
    AppState,
    backup::Manifest,
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
use talkops_core::{tenant::TenantId, users};
use tokio::io::AsyncWriteExt;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct RestoreInspection {
    pub manifest: Manifest,
    /// Base64 gzip bundle for explicit download. Contains deployment secrets.
    pub deployment_bundle: Option<String>,
}
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(inspect))
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
}
#[utoipa::path(post, path = "/api/v1/setup/restore/inspect", tag = "auth", responses((status = 200, body = RestoreInspection), (status = 403), (status = 409)))]
pub async fn inspect(
    State(state): State<AppState>,
    meta: RequestMeta,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResult<Response> {
    if !state.limiter.allow(&format!(
        "restore:{}",
        meta.ip.as_deref().unwrap_or("unknown")
    )) {
        return Err(ApiError::TooManyRequests);
    }
    let expected = state.setup_token.as_deref().ok_or(ApiError::Forbidden)?;
    let sent = headers
        .get("x-talkops-setup-token")
        .and_then(|s| s.to_str().ok())
        .unwrap_or_default();
    if !crate::util::constant_time_eq(sent.as_bytes(), expected.as_bytes()) {
        return Err(ApiError::Forbidden);
    }
    let cfg = state
        .backup
        .as_ref()
        .ok_or_else(|| ApiError::Conflict("backups are not configured".into()))?;
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
    let limit = cfg.restore_max_bytes.min(8 * 1024 * 1024 * 1024);
    let result = tokio::task::spawn_blocking(move || {
        let prepared = recovery::prepare(&archive, password.as_ref(), limit)?;
        let bundle = prepared
            .bundle()?
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b));
        drop(upload);
        Ok::<_, crate::backup::BackupError>(RestoreInspection {
            manifest: prepared.manifest,
            deployment_bundle: bundle,
        })
    })
    .await
    .map_err(|_| ApiError::BadRequest("archive inspection failed".into()))??;
    tx.commit().await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(result)).into_response())
}
