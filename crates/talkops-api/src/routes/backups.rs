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
use talkops_core::portainer::{self, PortainerConnection, PortainerInput};
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::backup::{self, BackupConfig, BackupError, BackupFile};
use crate::error::{ApiError, ApiResult};
use crate::portainer::{Api, PortainerError, Stack};
use crate::recovery::{self, DeploymentInfo, DeploymentInput};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_backups, start_backup))
        .routes(routes!(update_backup_settings))
        .routes(routes!(import_deployment))
        .routes(routes!(get_portainer, save_portainer, delete_portainer))
        .routes(routes!(probe_portainer))
        .routes(routes!(portainer_stacks))
        .routes(routes!(sync_portainer))
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
    pub configuration: Option<DeploymentInfo>,
    pub configuration_error: Option<String>,
    pub encryption_configured: bool,
    /// Mounted deployment files take precedence over imports and Portainer.
    pub mounted: bool,
    /// Automatic discovery of deployment files (`TALKOPS_DEPLOYMENT_FILES=auto`).
    pub auto_discovery: bool,
    pub portainer: Option<PortainerConnection>,
}

async fn overview(state: &AppState, auth: &AuthUser) -> ApiResult<BackupOverview> {
    let cfg = config(state)?;
    let (configuration, configuration_error) = match recovery::capture(cfg) {
        Ok((_, info)) => (Some(info), None),
        Err(err) => (None, Some(err.to_string())),
    };
    Ok(BackupOverview {
        settings: backups::get(&state.db, auth.tenant).await?,
        files: backup::list(&cfg.dir).await?,
        running: backup::is_busy(),
        configuration,
        configuration_error,
        encryption_configured: cfg.password.is_some(),
        mounted: recovery::mounted_active(cfg),
        auto_discovery: recovery::auto_discovery(cfg),
        portainer: portainer::get(&state.db, auth.tenant).await?,
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
    let (deployment, _) = recovery::capture(&cfg)?;
    if !deployment.files.is_empty() && cfg.password.is_none() {
        return Err(ApiError::Conflict(
            "configure a separate backup password before capturing deployment files".into(),
        ));
    }
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

/// Import the exact Portainer stack and environment files. Stored encrypted at rest.
#[utoipa::path(put, path = "/api/v1/backups/deployment", tag = "system", request_body = DeploymentInput, responses((status = 200, body = DeploymentInfo)))]
pub async fn import_deployment(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(mut input): Json<DeploymentInput>,
) -> ApiResult<Json<DeploymentInfo>> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    if recovery::mounted_active(cfg) {
        return Err(ApiError::Conflict(
            "mounted deployment files take precedence; update those files".into(),
        ));
    }
    input.origin = None;
    recovery::validate_configuration(&input)?;
    if input
        .files
        .values()
        .any(|s| recovery::env_value(s, "TALKOPS_BACKUP_PASSWORD").is_some_and(|p| !p.is_empty()))
    {
        return Err(ApiError::BadRequest(
            "keep the backup password outside imported deployment files".into(),
        ));
    }
    recovery::store_import(cfg, &state.secrets, &input)?;
    let info = recovery::deployment_info(&input, "imported", chrono::Utc::now());
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "backup_deployment",
        None,
        json!({"files": info.files, "complete":info.complete}),
    )
    .await?;
    Ok(Json(info))
}

impl From<PortainerError> for ApiError {
    fn from(err: PortainerError) -> Self {
        match err {
            PortainerError::Invalid(m) => ApiError::BadRequest(m),
            other => ApiError::Device(other.to_string()),
        }
    }
}

/// The Portainer connection, if any (without the token).
#[utoipa::path(get, path = "/api/v1/backups/portainer", tag = "system", responses((status = 200, body = Option<PortainerConnection>)))]
pub async fn get_portainer(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Option<PortainerConnection>>> {
    auth.require(Role::Admin)?;
    Ok(Json(portainer::get(&state.db, auth.tenant).await?))
}

/// Saves the Portainer connection and reads the stack once.
#[utoipa::path(put, path = "/api/v1/backups/portainer", tag = "system", request_body = PortainerInput, responses((status = 200, body = DeploymentInfo), (status = 502)))]
pub async fn save_portainer(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<PortainerInput>,
) -> ApiResult<Json<DeploymentInfo>> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    if recovery::mounted_active(cfg) {
        return Err(ApiError::Conflict(
            "mounted deployment files take precedence; update those files".into(),
        ));
    }
    let conn = portainer::save(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "backup_portainer",
        None,
        json!({"url": conn.url, "stack_id": conn.stack_id, "pinned": !conn.cert_sha256.is_empty()}),
    )
    .await?;
    Ok(Json(
        crate::portainer::sync(cfg, &state.db, &state.secrets, auth.tenant).await?,
    ))
}

/// Removes the Portainer connection. The last imported copy stays.
#[utoipa::path(delete, path = "/api/v1/backups/portainer", tag = "system", responses((status = 204)))]
pub async fn delete_portainer(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    portainer::delete(&state.db, auth.tenant).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "backup_portainer",
        None,
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize, ToSchema)]
pub struct PortainerProbe {
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub struct PortainerCertificate {
    /// SHA-256 of the server certificate (hex); `None` for http://.
    pub cert_sha256: Option<String>,
    /// The certificate is valid for a public CA; no pin is needed.
    pub trusted: bool,
}

/// Reads the certificate of a Portainer server for pinning. Sends no token.
#[utoipa::path(post, path = "/api/v1/backups/portainer/certificate", tag = "system", request_body = PortainerProbe, responses((status = 200, body = PortainerCertificate), (status = 502)))]
pub async fn probe_portainer(
    auth: AuthUser,
    Json(input): Json<PortainerProbe>,
) -> ApiResult<Json<PortainerCertificate>> {
    auth.require(Role::Admin)?;
    let url = portainer::check_url(&input.url)?;
    let cert_sha256 = crate::portainer::probe_certificate(&url).await?;
    // A public CA works without a pin; the stack list tells.
    let api = Api {
        url: &url,
        token: "",
        cert_sha256: "",
    };
    let trusted = cert_sha256.is_some()
        && !matches!(
            api.stacks().await,
            Err(PortainerError::Http(_) | PortainerError::CertificateMismatch)
        );
    Ok(Json(PortainerCertificate {
        cert_sha256,
        trusted,
    }))
}

#[derive(serde::Deserialize, ToSchema)]
pub struct PortainerStacksRequest {
    pub url: String,
    #[serde(default)]
    pub cert_sha256: String,
    /// `null` uses the stored token.
    #[serde(default)]
    pub token: Option<String>,
}

/// Lists the stacks the token can read.
#[utoipa::path(post, path = "/api/v1/backups/portainer/stacks", tag = "system", request_body = PortainerStacksRequest, responses((status = 200, body = Vec<Stack>), (status = 502)))]
pub async fn portainer_stacks(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<PortainerStacksRequest>,
) -> ApiResult<Json<Vec<Stack>>> {
    auth.require(Role::Admin)?;
    let url = portainer::check_url(&input.url)?;
    let cert = portainer::check_fingerprint(&input.cert_sha256)?;
    let token = match input
        .token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => {
            portainer::check_token(t)?;
            t.to_owned()
        }
        None => portainer::token(&state.db, auth.tenant, &state.secrets)
            .await
            .map_err(|_| ApiError::BadRequest("enter the access token".into()))?,
    };
    let api = Api {
        url: &url,
        token: &token,
        cert_sha256: &cert,
    };
    Ok(Json(api.stacks().await?))
}

/// Reads the stack from Portainer now.
#[utoipa::path(post, path = "/api/v1/backups/portainer/sync", tag = "system", responses((status = 200, body = DeploymentInfo), (status = 502)))]
pub async fn sync_portainer(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<DeploymentInfo>> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    let info = crate::portainer::sync(cfg, &state.db, &state.secrets, auth.tenant).await?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "sync",
        "backup_portainer",
        None,
        json!({"files": info.files, "complete": info.complete}),
    )
    .await;
    Ok(Json(info))
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
            (
                header::CONTENT_TYPE,
                if name.ends_with(".age") {
                    "application/octet-stream"
                } else {
                    "application/gzip"
                }
                .to_owned(),
            ),
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
