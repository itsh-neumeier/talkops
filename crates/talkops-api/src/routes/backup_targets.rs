//! Backup targets (admin, ADR 0029): manage targets, test connections,
//! browse folders, run a target now, list, download and delete its archives.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::backup_targets::{
    self, BackupTarget, BackupTargetInput, TargetConfig, TargetConnection, TargetSecret,
};
use talkops_core::tenant::TenantId;
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::backup::{self, BackupConfig};
use crate::backup_targets::{self as runner, Activity};
use crate::error::{ApiError, ApiResult};
use crate::rclone::{Entry, RcloneError, TestReport};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_targets, create_target))
        .routes(routes!(update_target, delete_target))
        .routes(routes!(test_target))
        .routes(routes!(browse_target))
        .routes(routes!(create_folder))
        .routes(routes!(run_target))
        .routes(routes!(list_archives))
        .routes(routes!(fetch_archive))
        .routes(routes!(delete_archive))
}

fn config(state: &AppState) -> ApiResult<&std::sync::Arc<BackupConfig>> {
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
pub struct TargetView {
    #[serde(flatten)]
    pub target: BackupTarget,
    /// Next planned run; `None` for manual targets.
    pub next_run_at: Option<DateTime<Utc>>,
    /// Current or last upload or download since the server start.
    pub activity: Option<Activity>,
}

#[derive(Serialize, ToSchema)]
pub struct TargetsOverview {
    pub targets: Vec<TargetView>,
    /// A backup password is set; remote targets require it.
    pub encryption_configured: bool,
    /// Allowed directories of local targets.
    pub roots: Vec<String>,
}

fn next_run(target: &BackupTarget) -> Option<DateTime<Utc>> {
    target
        .schedule
        .next(Local::now().naive_local())
        .and_then(|n| n.and_local_timezone(Local).earliest())
        .map(|n| n.with_timezone(&Utc))
}

/// Backup targets with their status.
#[utoipa::path(get, path = "/api/v1/backup-targets", tag = "system", responses((status = 200, body = TargetsOverview)))]
pub async fn list_targets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<TargetsOverview>> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    let targets = backup_targets::list(&state.db, auth.tenant)
        .await?
        .into_iter()
        .map(|target| TargetView {
            next_run_at: if target.enabled {
                next_run(&target)
            } else {
                None
            },
            activity: runner::activity(target.id),
            target,
        })
        .collect();
    Ok(Json(TargetsOverview {
        targets,
        encryption_configured: cfg.password.is_some(),
        roots: cfg
            .target_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    }))
}

fn audit_details(target: &BackupTarget) -> serde_json::Value {
    json!({
        "name": target.name,
        "kind": target.config.kind(),
        "path": target.path,
        "enabled": target.enabled,
        "schedule": target.schedule,
        "retention": target.retention,
    })
}

async fn check_input(
    state: &AppState,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &BackupTargetInput,
) -> ApiResult<()> {
    let cfg = config(state)?;
    let stored = match id {
        Some(id) => backup_targets::secret(&state.db, tenant, &state.secrets, id).await?,
        None => TargetSecret::default(),
    };
    let conn = backup_targets::check(input, stored)?;
    runner::check_connection(cfg, &conn)?;
    if input.enabled {
        runner::check_encryption(cfg, &conn.config)
            .map_err(|e| ApiError::Conflict(e.to_string()))?;
    }
    Ok(())
}

/// Adds a backup target. Remote targets need a backup password.
#[utoipa::path(post, path = "/api/v1/backup-targets", tag = "system", request_body = BackupTargetInput, responses((status = 201, body = BackupTarget), (status = 409)))]
pub async fn create_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<BackupTargetInput>,
) -> ApiResult<(StatusCode, Json<BackupTarget>)> {
    auth.require(Role::Admin)?;
    check_input(&state, auth.tenant, None, &input).await?;
    let target = backup_targets::create(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "backup_target",
        Some(target.id.to_string()),
        audit_details(&target),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(target)))
}

/// Changes a backup target; missing secret fields keep the stored values.
#[utoipa::path(put, path = "/api/v1/backup-targets/{id}", tag = "system", params(("id" = Uuid, Path)), request_body = BackupTargetInput, responses((status = 200, body = BackupTarget)))]
pub async fn update_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<BackupTargetInput>,
) -> ApiResult<Json<BackupTarget>> {
    auth.require(Role::Admin)?;
    check_input(&state, auth.tenant, Some(id), &input).await?;
    let target = backup_targets::update(&state.db, auth.tenant, &state.secrets, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "backup_target",
        Some(id.to_string()),
        audit_details(&target),
    )
    .await?;
    Ok(Json(target))
}

/// Deletes a backup target. Archives on the target stay.
#[utoipa::path(delete, path = "/api/v1/backup-targets/{id}", tag = "system", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let target = backup_targets::get(&state.db, auth.tenant, id).await?;
    backup_targets::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "backup_target",
        Some(id.to_string()),
        json!({"name": target.name, "kind": target.config.kind()}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A connection from the editor, possibly not saved yet.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DraftConnection {
    /// Saved target whose stored secrets fill the missing secret fields.
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
        Some(id) => backup_targets::secret(&state.db, tenant, &state.secrets, id).await?,
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
#[utoipa::path(post, path = "/api/v1/backup-targets/browse", tag = "system", request_body = DraftConnection, responses((status = 200, body = BrowseResult), (status = 502)))]
pub async fn browse_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(draft): Json<DraftConnection>,
) -> ApiResult<Json<BrowseResult>> {
    auth.require(Role::Admin)?;
    let conn = resolve(&state, auth.tenant, draft).await?;
    Ok(Json(browse(config(&state)?, conn).await?))
}

/// Tests a connection: creates the folder, writes, reads and deletes a
/// test file. SFTP: returns the host key to accept.
#[utoipa::path(post, path = "/api/v1/backup-targets/test", tag = "system", request_body = DraftConnection, responses((status = 200, body = TestReport), (status = 502)))]
pub async fn test_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(draft): Json<DraftConnection>,
) -> ApiResult<Json<TestReport>> {
    auth.require(Role::Admin)?;
    let conn = resolve(&state, auth.tenant, draft).await?;
    let cfg = config(&state)?;
    if matches!(conn.config, TargetConfig::Local) && conn.path.is_empty() {
        return Err(ApiError::BadRequest("select a folder".into()));
    }
    Ok(Json(cfg.rclone.test(&conn).await.map_err(rclone_error)?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewFolder {
    /// The parent folder.
    pub connection: DraftConnection,
    pub name: String,
}

/// Creates a folder in the folder browser.
#[utoipa::path(post, path = "/api/v1/backup-targets/folders", tag = "system", request_body = NewFolder, responses((status = 204), (status = 502)))]
pub async fn create_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<NewFolder>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
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

/// Starts a backup to this target now (`202`); the list shows the result.
#[utoipa::path(post, path = "/api/v1/backup-targets/{id}/run", tag = "system", params(("id" = Uuid, Path)), responses((status = 202), (status = 409)))]
pub async fn run_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?.clone();
    let target = backup_targets::get(&state.db, auth.tenant, id).await?;
    runner::check_encryption(&cfg, &target.config)
        .map_err(|e| ApiError::Conflict(e.to_string()))?;
    if !runner::start(
        cfg,
        state.db.clone(),
        state.secrets.clone(),
        auth.tenant,
        target.clone(),
    ) {
        return Err(ApiError::Conflict(
            "a backup target run is already active".into(),
        ));
    }
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "run",
        "backup_target",
        Some(id.to_string()),
        json!({"name": target.name}),
    )
    .await;
    Ok(StatusCode::ACCEPTED)
}

/// TalkOps archives on a target, newest first.
#[utoipa::path(get, path = "/api/v1/backup-targets/{id}/archives", tag = "system", params(("id" = Uuid, Path)), responses((status = 200, body = Vec<Entry>), (status = 502)))]
pub async fn list_archives(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<Entry>>> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    let (_, conn) = backup_targets::connection(&state.db, auth.tenant, &state.secrets, id).await?;
    runner::check_connection(cfg, &conn)?;
    match runner::archives(&cfg.rclone, &conn).await {
        Ok(list) => Ok(Json(list)),
        Err(err) => Err(ApiError::Device(err.to_string())),
    }
}

fn checked_name(name: &str) -> ApiResult<&str> {
    if backup::valid_name(name) {
        Ok(name)
    } else {
        Err(ApiError::NotFound)
    }
}

/// Downloads an archive from the target into the local backup directory
/// (`202`). Then restore it offline as described in the backup guide.
#[utoipa::path(post, path = "/api/v1/backup-targets/{id}/archives/{name}/fetch", tag = "system", params(("id" = Uuid, Path), ("name" = String, Path)), responses((status = 202), (status = 409)))]
pub async fn fetch_archive(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?.clone();
    let name = checked_name(&name)?.to_owned();
    if cfg.dir.join(&name).exists() {
        return Err(ApiError::Conflict(
            "an archive with this name is already on this server".into(),
        ));
    }
    let (target, conn) =
        backup_targets::connection(&state.db, auth.tenant, &state.secrets, id).await?;
    runner::check_connection(&cfg, &conn)?;
    if !runner::start_fetch(cfg, target.clone(), conn, name.clone()) {
        return Err(ApiError::Conflict("this target is busy".into()));
    }
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "fetch",
        "backup_target",
        Some(id.to_string()),
        json!({"name": target.name, "file": name}),
    )
    .await;
    Ok(StatusCode::ACCEPTED)
}

/// Deletes an archive on the target.
#[utoipa::path(delete, path = "/api/v1/backup-targets/{id}/archives/{name}", tag = "system", params(("id" = Uuid, Path), ("name" = String, Path)), responses((status = 204), (status = 502)))]
pub async fn delete_archive(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let cfg = config(&state)?;
    let name = checked_name(&name)?;
    let (target, conn) =
        backup_targets::connection(&state.db, auth.tenant, &state.secrets, id).await?;
    runner::check_connection(cfg, &conn)?;
    cfg.rclone.delete(&conn, name).await.map_err(rclone_error)?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "backup_target_archive",
        Some(id.to_string()),
        json!({"name": target.name, "file": name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
