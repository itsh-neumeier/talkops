//! Automatic audit export and audit retention (ADR 0037).
//!
//! Each minute the task checks the schedule of the tenant. A due run writes
//! all audit entries after the last exported entry into one new file (CSV
//! or JSON Lines), uploads it to the storage target (ADR 0036) with a
//! `.sha256` file next to it, records the SHA-256 checksum and applies the
//! file retention. Once an hour, the task deletes audit entries older than
//! the retention period. While the export is on, it deletes only entries
//! that an export file contains.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Utc};
use sha2::Digest;
use sqlx::PgPool;
use talkops_core::audit::{self, ExportFormat};
use talkops_core::audit_export::{self, ExportFile};
use talkops_core::crypto::SecretBox;
use talkops_core::storage_targets::{self, TargetConnection, join_path};
use talkops_core::tenant::TenantId;

use crate::backup::BackupConfig;
use crate::rclone::RcloneError;

/// The most entries in one export file. The next run continues.
pub const BATCH: i64 = 50_000;

/// Only one export run at a time.
static RUNNER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub fn is_running() -> bool {
    RUNNER.try_lock().is_err()
}

fn checksum(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

/// Deletes export files that the file retention does not keep, with their
/// checksum files. Returns the number of deleted export files.
async fn prune(
    cfg: &BackupConfig,
    conn: &TargetConnection,
    retention: &talkops_core::backup_targets::Retention,
) -> Result<usize, RcloneError> {
    let entries = match cfg.rclone.list(conn, "").await {
        Ok((entries, _)) => entries,
        Err(RcloneError::NotFound) => return Ok(0),
        Err(err) => return Err(err),
    };
    let names: std::collections::HashSet<&str> = entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.name.as_str())
        .collect();
    let files: Vec<(&str, chrono::NaiveDateTime)> = names
        .iter()
        .filter_map(|n| audit_export::file_stamp(n).map(|s| (*n, s)))
        .collect();
    let stamps: Vec<_> = files.iter().map(|(_, s)| *s).collect();
    let keep = retention.keep(&stamps, Local::now().naive_local());
    let mut deleted = 0;
    for (i, (name, _)) in files.iter().enumerate() {
        if keep.contains(&i) {
            continue;
        }
        cfg.rclone.delete(conn, name).await?;
        let sum = format!("{name}.sha256");
        if names.contains(sum.as_str()) {
            cfg.rclone.delete(conn, &sum).await?;
        }
        deleted += 1;
    }
    Ok(deleted)
}

/// Writes the new audit entries to the storage target. Returns the written
/// file, or `None` when there are no new entries.
pub async fn run(
    cfg: &BackupConfig,
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
) -> Result<Option<ExportFile>, String> {
    let settings = audit_export::get(db, tenant)
        .await
        .map_err(|e| e.to_string())?;
    let target_id = settings
        .storage_target_id
        .ok_or_else(|| "no storage target selected".to_owned())?;
    let (target, mut conn) = storage_targets::connection(db, tenant, secrets, target_id)
        .await
        .map_err(|e| e.to_string())?;
    conn.path = join_path(&target.path, &settings.folder);
    crate::backup_targets::check_connection(cfg, &conn).map_err(|e| e.to_string())?;

    let entries = audit::after(db, tenant, settings.last_exported_id, BATCH)
        .await
        .map_err(|e| e.to_string())?;
    let (Some(first), Some(last)) = (entries.first(), entries.last()) else {
        return Ok(None);
    };
    let content = match settings.format {
        ExportFormat::Csv => audit::to_csv(&entries),
        ExportFormat::Json => audit::to_json_lines(&entries),
    };
    let sha256 = checksum(content.as_bytes());
    let name = audit_export::file_name(
        Local::now().naive_local(),
        first.id,
        last.id,
        settings.format,
    );

    // The root filesystem is read-only; stage the files in the backups volume.
    std::fs::create_dir_all(&cfg.dir).map_err(|e| e.to_string())?;
    let staging = tempfile::Builder::new()
        .prefix(".audit-")
        .tempdir_in(&cfg.dir)
        .map_err(|e| e.to_string())?;
    let local = staging.path().join(&name);
    std::fs::write(&local, content.as_bytes()).map_err(|e| e.to_string())?;
    let sum_name = format!("{name}.sha256");
    let sum_local = staging.path().join(&sum_name);
    std::fs::write(&sum_local, format!("{sha256}  {name}\n")).map_err(|e| e.to_string())?;

    cfg.rclone
        .upload(&conn, &local, &name, target.bwlimit_kbps)
        .await
        .map_err(|e| e.to_string())?;
    cfg.rclone
        .upload(&conn, &sum_local, &sum_name, target.bwlimit_kbps)
        .await
        .map_err(|e| e.to_string())?;
    let file = ExportFile {
        id: 0,
        storage_target_id: Some(target_id),
        file: name,
        first_id: first.id,
        last_id: last.id,
        entries: entries.len() as i32,
        bytes: content.len() as i64,
        sha256,
        created_at: Utc::now(),
    };
    audit_export::record_file(db, tenant, &file)
        .await
        .map_err(|e| e.to_string())?;
    match prune(cfg, &conn, &settings.retention).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(count = n, "old audit export files deleted"),
        Err(err) => tracing::warn!(error = %err, "cannot apply audit export file retention"),
    }
    Ok(Some(file))
}

/// Runs the export and records the outcome.
async fn run_and_record(cfg: &BackupConfig, db: &PgPool, secrets: &SecretBox, tenant: TenantId) {
    let result = run(cfg, db, secrets, tenant).await;
    match &result {
        Ok(Some(file)) => {
            tracing::info!(file = %file.file, entries = file.entries, "audit entries exported")
        }
        Ok(None) => {}
        Err(err) => tracing::warn!(error = %err, "audit export failed"),
    }
    if let Err(err) = audit_export::record_run(db, tenant, result.err().as_deref()).await {
        tracing::warn!(error = %err, "cannot record the audit export run");
    }
}

/// Starts a run now in the background; `false` if one is active.
pub fn start(cfg: Arc<BackupConfig>, db: PgPool, secrets: SecretBox, tenant: TenantId) -> bool {
    let Ok(guard) = RUNNER.try_lock() else {
        return false;
    };
    tokio::spawn(async move {
        let _guard = guard;
        run_and_record(&cfg, &db, &secrets, tenant).await;
    });
    true
}

/// Starts the export when it is due (called every minute).
pub async fn tick(cfg: &Arc<BackupConfig>, db: &PgPool, secrets: &SecretBox, tenant: TenantId) {
    let Ok(settings) = audit_export::get(db, tenant).await else {
        return;
    };
    let Some(created) = settings.created_at else {
        return;
    };
    let since = settings.last_run_at.unwrap_or(created);
    if settings.export_enabled
        && settings.schedule.due(
            since.with_timezone(&Local).naive_local(),
            Local::now().naive_local(),
        )
    {
        start(cfg.clone(), db.clone(), secrets.clone(), tenant);
    }
}

/// Applies the retention of audit entries. Returns the number of deleted
/// entries.
pub async fn apply_retention(
    db: &PgPool,
    tenant: TenantId,
) -> talkops_core::error::CoreResult<u64> {
    let settings = audit_export::get(db, tenant).await?;
    let exported = settings.export_enabled.then_some(settings.last_exported_id);
    audit::prune(db, tenant, settings.retention_days, exported).await
}

/// The background task: export every minute when due, retention hourly.
pub fn spawn(db: PgPool, cfg: Option<Arc<BackupConfig>>, secrets: SecretBox) {
    tokio::spawn(async move {
        let mut minute = 0u32;
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let tenant = TenantId::DEFAULT;
            if let Some(cfg) = &cfg {
                tick(cfg, &db, &secrets, tenant).await;
            }
            if minute % 60 == 0 {
                match apply_retention(&db, tenant).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(count = n, "expired audit entries deleted"),
                    Err(err) => tracing::warn!(error = %err, "audit retention failed"),
                }
            }
            minute = minute.wrapping_add(1);
        }
    });
}
