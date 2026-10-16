//! Storage targets (ADR 0036): connections to local folders, SMB, S3, FTP,
//! WebDAV and SFTP that several features use. Manage targets, test
//! connections and browse folders. Backup jobs (`backup_targets`) select a
//! storage target.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::roles::Permission;
use talkops_core::storage_targets::{
    self, StorageTarget, StorageTargetInput, StorageUsage, TargetConfig, TargetConnection,
    TargetSecret,
};
use talkops_core::tenant::TenantId;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::backup::BackupConfig;
use crate::backup_targets as runner;
use crate::error::{ApiError, ApiResult};
use crate::rclone::{Entry, RcloneError, TestReport};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_targets, create_target))
        .routes(routes!(update_target, delete_target))
        .routes(routes!(test_draft))
        .routes(routes!(test_saved))
        .routes(routes!(browse_target))
        .routes(routes!(create_folder))
}

/// Storage targets need the transfer settings of the backup configuration
/// (rclone, local roots).
pub(crate) fn config(state: &AppState) -> ApiResult<&std::sync::Arc<BackupConfig>> {
    state
        .backup
        .as_ref()
        .ok_or_else(|| ApiError::Conflict("backups are not configured".into()))
}

/// Maps transfer errors: the target failed (502) or rclone is missing.
pub fn rclone_error(err: RcloneError) -> ApiError {
    match err {
        RcloneError::NotFound => {
            ApiError::BadRequest("folder or file not found on the target".into())
        }
        RcloneError::Missing(m) => ApiError::Conflict(format!("rclone is not available: {m}")),
        other => ApiError::Device(other.to_string()),
    }
}

#[derive(Serialize, ToSchema)]
pub struct StorageTargetView {
    #[serde(flatten)]
    pub target: StorageTarget,
    /// Jobs and features that use this target.
    pub used_by: Vec<StorageUsage>,
}

#[derive(Serialize, ToSchema)]
pub struct StorageOverview {
    pub targets: Vec<StorageTargetView>,
    /// A backup password is set; remote backup jobs require it.
    pub encryption_configured: bool,
    /// Allowed directories of local targets.
    pub roots: Vec<String>,
}

/// Storage targets with their users. Users with `backups.manage` can read
/// the list to select a target for a backup job.
#[utoipa::path(get, path = "/api/v1/storage-targets", tag = "system", responses((status = 200, body = StorageOverview)))]
pub async fn list_targets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<StorageOverview>> {
    if !auth.can(Permission::BackupsManage) {
        auth.require_perm(Permission::StorageManage)?;
    }
    let cfg = config(&state)?;
    let mut users = storage_targets::usages(&state.db, auth.tenant).await?;
    let targets = storage_targets::list(&state.db, auth.tenant)
        .await?
        .into_iter()
        .map(|target| StorageTargetView {
            used_by: users.remove(&target.id).unwrap_or_default(),
            target,
        })
        .collect();
    Ok(Json(StorageOverview {
        targets,
        encryption_configured: cfg.password.is_some(),
        roots: cfg
            .target_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    }))
}

fn audit_details(target: &StorageTarget) -> serde_json::Value {
    json!({
        "name": target.name,
        "kind": target.config.kind(),
        "path": target.path,
        "allow_insecure": target.allow_insecure,
        "bwlimit_kbps": target.bwlimit_kbps,
    })
}

async fn check_input(
    state: &AppState,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &StorageTargetInput,
) -> ApiResult<()> {
    let cfg = config(state)?;
    let stored = match id {
        Some(id) => storage_targets::secret(&state.db, tenant, &state.secrets, id).await?,
        None => TargetSecret::default(),
    };
    let conn = storage_targets::check(input, stored)?;
    runner::check_connection(cfg, &conn)?;
    // Enabled backup jobs on this target need encrypted archives.
    if let Some(id) = id {
        let jobs = talkops_core::backup_targets::list(&state.db, tenant).await?;
        if jobs.iter().any(|j| j.storage_target_id == id && j.enabled) {
            runner::check_encryption(cfg, &conn.config)
                .map_err(|e| ApiError::Conflict(e.to_string()))?;
        }
    }
    Ok(())
}

/// Adds a storage target.
#[utoipa::path(post, path = "/api/v1/storage-targets", tag = "system", request_body = StorageTargetInput, responses((status = 201, body = StorageTarget), (status = 409)))]
pub async fn create_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<StorageTargetInput>,
) -> ApiResult<(StatusCode, Json<StorageTarget>)> {
    auth.require_perm(Permission::StorageManage)?;
    check_input(&state, auth.tenant, None, &input).await?;
    let target = storage_targets::create(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "storage_target",
        Some(target.id.to_string()),
        audit_details(&target),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(target)))
}

/// Changes a storage target; missing secret fields keep the stored values.
/// The change applies to all jobs that use the target.
#[utoipa::path(put, path = "/api/v1/storage-targets/{id}", tag = "system", params(("id" = Uuid, Path)), request_body = StorageTargetInput, responses((status = 200, body = StorageTarget)))]
pub async fn update_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<StorageTargetInput>,
) -> ApiResult<Json<StorageTarget>> {
    auth.require_perm(Permission::StorageManage)?;
    check_input(&state, auth.tenant, Some(id), &input).await?;
    let target =
        storage_targets::update(&state.db, auth.tenant, &state.secrets, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "storage_target",
        Some(id.to_string()),
        audit_details(&target),
    )
    .await?;
    Ok(Json(target))
}

/// Deletes a storage target. A target that a job uses cannot be deleted
/// (`409`). Files on the target stay.
#[utoipa::path(delete, path = "/api/v1/storage-targets/{id}", tag = "system", params(("id" = Uuid, Path)), responses((status = 204), (status = 409)))]
pub async fn delete_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::StorageManage)?;
    let target = storage_targets::get(&state.db, auth.tenant, id).await?;
    storage_targets::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "storage_target",
        Some(id.to_string()),
        json!({"name": target.name, "kind": target.config.kind()}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A connection from the editor, possibly not saved yet.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DraftConnection {
    /// Saved storage target whose stored secrets fill the missing secret
    /// fields.
    #[serde(default)]
    pub id: Option<Uuid>,
    pub config: TargetConfig,
    /// The folder to use or list.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub allow_insecure: bool,
    #[serde(default)]
    pub secret: Option<TargetSecret>,
}

impl DraftConnection {
    /// The checked connection; stored secrets come from `stored`.
    pub fn resolve(self, cfg: &BackupConfig, stored: TargetSecret) -> ApiResult<TargetConnection> {
        let mut conn = TargetConnection {
            config: self.config,
            path: self.path,
            allow_insecure: self.allow_insecure,
            secret: stored.merge(self.secret.as_ref()),
        };
        if matches!(conn.config, TargetConfig::Local) && conn.path.trim().is_empty() {
            conn.path = "/".into();
            conn.validate(true)?;
            conn.path.clear();
            return Ok(conn);
        }
        conn.validate(true)?;
        runner::check_connection(cfg, &conn)?;
        Ok(conn)
    }
}

async fn resolve(
    state: &AppState,
    tenant: TenantId,
    draft: DraftConnection,
) -> ApiResult<TargetConnection> {
    let cfg = config(state)?;
    let stored = match draft.id {
        Some(id) => storage_targets::secret(&state.db, tenant, &state.secrets, id).await?,
        None => TargetSecret::default(),
    };
    draft.resolve(cfg, stored)
}

#[derive(Serialize, ToSchema)]
pub struct BrowseResult {
    /// Folders first, then files, by name.
    pub entries: Vec<Entry>,
    /// The folder has more entries than the list shows.
    pub truncated: bool,
    /// SFTP: host keys accepted on this first connection.
    pub host_keys: Option<String>,
    pub fingerprint: Option<String>,
}

/// Lists one folder level of a target (folders and files) for the folder
/// browser. Local targets with an empty path list the allowed roots.
pub async fn browse(cfg: &BackupConfig, mut conn: TargetConnection) -> ApiResult<BrowseResult> {
    if matches!(conn.config, TargetConfig::Local) && conn.path.is_empty() {
        return Ok(BrowseResult {
            entries: cfg
                .target_roots
                .iter()
                .map(|p| Entry {
                    name: p.display().to_string(),
                    is_dir: true,
                    size: 0,
                    modified: None,
                })
                .collect(),
            truncated: false,
            host_keys: None,
            fingerprint: None,
        });
    }
    let mut pinned = (None, None);
    if let TargetConfig::Sftp { host_keys, .. } = &conn.config
        && host_keys.is_empty()
    {
        let (keys, print) = cfg.rclone.pin_host_key(&conn).await.map_err(rclone_error)?;
        if let TargetConfig::Sftp { host_keys, .. } = &mut conn.config {
            host_keys.clone_from(&keys);
        }
        pinned = (Some(keys), Some(print));
    }
    let (entries, truncated) = match cfg.rclone.list(&conn, "").await {
        Ok(list) => list,
        Err(RcloneError::NotFound) => (Vec::new(), false),
        Err(err) => return Err(rclone_error(err)),
    };
    Ok(BrowseResult {
        entries,
        truncated,
        host_keys: pinned.0,
        fingerprint: pinned.1,
    })
}

/// Lists a folder of a target (folder browser).
#[utoipa::path(post, path = "/api/v1/storage-targets/browse", tag = "system", request_body = DraftConnection, responses((status = 200, body = BrowseResult), (status = 502)))]
pub async fn browse_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(draft): Json<DraftConnection>,
) -> ApiResult<Json<BrowseResult>> {
    auth.require_perm(Permission::StorageManage)?;
    let conn = resolve(&state, auth.tenant, draft).await?;
    Ok(Json(browse(config(&state)?, conn).await?))
}

/// Tests a connection from the editor: creates the folder, writes, reads
/// and deletes a test file. SFTP: returns the host key to accept.
#[utoipa::path(post, path = "/api/v1/storage-targets/test", tag = "system", request_body = DraftConnection, responses((status = 200, body = TestReport), (status = 502)))]
pub async fn test_draft(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(draft): Json<DraftConnection>,
) -> ApiResult<Json<TestReport>> {
    auth.require_perm(Permission::StorageManage)?;
    let conn = resolve(&state, auth.tenant, draft).await?;
    let cfg = config(&state)?;
    if matches!(conn.config, TargetConfig::Local) && conn.path.is_empty() {
        return Err(ApiError::BadRequest("select a folder".into()));
    }
    Ok(Json(cfg.rclone.test(&conn).await.map_err(rclone_error)?))
}

/// Tests the saved connection of a target and records the result.
#[utoipa::path(post, path = "/api/v1/storage-targets/{id}/test", tag = "system", params(("id" = Uuid, Path)), responses((status = 200, body = StorageTarget)))]
pub async fn test_saved(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<StorageTarget>> {
    auth.require_perm(Permission::StorageManage)?;
    let cfg = config(&state)?;
    let (_, conn) = storage_targets::connection(&state.db, auth.tenant, &state.secrets, id).await?;
    let result = match runner::check_connection(cfg, &conn) {
        Ok(()) => cfg
            .rclone
            .test(&conn)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    storage_targets::record_test(
        &state.db,
        auth.tenant,
        id,
        result.as_ref().map(|_| ()).map_err(String::as_str),
    )
    .await?;
    Ok(Json(
        storage_targets::get(&state.db, auth.tenant, id).await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewFolder {
    /// The parent folder.
    pub connection: DraftConnection,
    pub name: String,
}

/// Creates a folder in the folder browser.
#[utoipa::path(post, path = "/api/v1/storage-targets/folders", tag = "system", request_body = NewFolder, responses((status = 204), (status = 502)))]
pub async fn create_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<NewFolder>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::StorageManage)?;
    let name = input.name.trim();
    if name.is_empty()
        || name.len() > 255
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
    {
        return Err(ApiError::BadRequest("invalid folder name".into()));
    }
    let mut draft = input.connection;
    draft.path = if draft.path.trim_end_matches('/').is_empty() {
        name.to_owned()
    } else {
        format!("{}/{name}", draft.path.trim_end_matches('/'))
    };
    let conn = resolve(&state, auth.tenant, draft).await?;
    if matches!(conn.config, TargetConfig::Local) && conn.path.is_empty() {
        return Err(ApiError::BadRequest("select a folder".into()));
    }
    config(&state)?
        .rclone
        .mkdir(&conn, "")
        .await
        .map_err(rclone_error)?;
    Ok(StatusCode::NO_CONTENT)
}
