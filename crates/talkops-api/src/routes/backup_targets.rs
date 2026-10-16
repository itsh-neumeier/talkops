//! Backup targets (ADR 0031, ADR 0036): backup jobs that copy archives to a
//! storage target. Manage jobs, run a job now, list, download and delete
//! its archives. Connections, tests and the folder browser are in
//! `storage_targets`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::backup_targets::{self, BackupTarget, BackupTargetInput};
use talkops_core::roles::Permission;
use talkops_core::storage_targets;
use talkops_core::tenant::TenantId;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::storage_targets::{config, rclone_error};
use crate::AppState;
use crate::auth::AuthUser;
use crate::backup;
use crate::backup_targets::{self as runner, Activity};
use crate::error::{ApiError, ApiResult};
use crate::rclone::Entry;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_targets, create_target))
        .routes(routes!(update_target, delete_target))
        .routes(routes!(run_target))
        .routes(routes!(list_archives))
        .routes(routes!(fetch_archive))
        .routes(routes!(delete_archive))
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
    auth.require_perm(Permission::BackupsManage)?;
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
        "storage_target_id": target.storage_target_id,
        "storage": target.storage_name,
        "folder": target.folder,
        "enabled": target.enabled,
        "include_recordings": target.include_recordings,
        "schedule": target.schedule,
        "retention": target.retention,
    })
}

async fn check_input(
    state: &AppState,
    tenant: TenantId,
    input: &BackupTargetInput,
) -> ApiResult<()> {
    let cfg = config(state)?;
    let folder = backup_targets::check(input)?;
    let (storage, mut conn) =
        storage_targets::connection(&state.db, tenant, &state.secrets, input.storage_target_id)
            .await
            .map_err(|e| match e {
                talkops_core::error::CoreError::NotFound => {
                    ApiError::BadRequest("unknown storage target".into())
                }
                other => other.into(),
            })?;
    conn.path = storage_targets::join_path(&storage.path, &folder);
    runner::check_connection(cfg, &conn)?;
    if input.enabled {
        runner::check_encryption(cfg, &conn.config)
            .map_err(|e| ApiError::Conflict(e.to_string()))?;
    }
    Ok(())
}

/// Adds a backup job on a storage target. Remote storage targets need a
/// backup password.
#[utoipa::path(post, path = "/api/v1/backup-targets", tag = "system", request_body = BackupTargetInput, responses((status = 201, body = BackupTarget), (status = 409)))]
pub async fn create_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<BackupTargetInput>,
) -> ApiResult<(StatusCode, Json<BackupTarget>)> {
    auth.require_perm(Permission::BackupsManage)?;
    check_input(&state, auth.tenant, &input).await?;
    let target = backup_targets::create(&state.db, auth.tenant, &input).await?;
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

/// Changes a backup job (schedule, retention, storage target, folder).
#[utoipa::path(put, path = "/api/v1/backup-targets/{id}", tag = "system", params(("id" = Uuid, Path)), request_body = BackupTargetInput, responses((status = 200, body = BackupTarget)))]
pub async fn update_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<BackupTargetInput>,
) -> ApiResult<Json<BackupTarget>> {
    auth.require_perm(Permission::BackupsManage)?;
    check_input(&state, auth.tenant, &input).await?;
    let target = backup_targets::update(&state.db, auth.tenant, id, &input).await?;
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
    auth.require_perm(Permission::BackupsManage)?;
    let target = backup_targets::get(&state.db, auth.tenant, id).await?;
    backup_targets::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "backup_target",
        Some(id.to_string()),
        json!({"name": target.name, "storage": target.storage_name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Starts a backup to this target now (`202`); the list shows the result.
#[utoipa::path(post, path = "/api/v1/backup-targets/{id}/run", tag = "system", params(("id" = Uuid, Path)), responses((status = 202), (status = 409)))]
pub async fn run_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::BackupsManage)?;
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
    auth.require_perm(Permission::BackupsManage)?;
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
    auth.require_perm(Permission::BackupsManage)?;
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
    auth.require_perm(Permission::BackupsManage)?;
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
