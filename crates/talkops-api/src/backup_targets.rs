//! Copies backup archives to backup targets (ADR 0031) and gets archives
//! back for a restore. Each target has its own schedule and retention.
//!
//! A target that is due in the same minute as the daily local backup
//! receives that archive. Otherwise TalkOps writes an archive into a
//! private staging folder in the backup directory, uploads it to all due
//! targets with the same contents (with or without recordings) and deletes
//! it. Retention on a target only deletes files with TalkOps archive names.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use sqlx::PgPool;
use talkops_core::backup_targets::{self, BackupTarget, TargetConfig, TargetConnection};
use talkops_core::crypto::SecretBox;
use talkops_core::tenant::TenantId;
use uuid::Uuid;

use crate::backup::{self, BackupConfig, BackupError};
use crate::rclone::{Entry, Rclone, RcloneError};

/// Only one run that uploads archives at a time.
static RUNNER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What a target does right now (uploads and downloads).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Activity {
    /// `backup` or `fetch`.
    pub action: String,
    pub file: Option<String>,
    pub started_at: DateTime<Utc>,
    /// Result of the last download: `None` while it runs.
    pub error: Option<String>,
    pub done: bool,
}

static ACTIVITY: Mutex<Option<HashMap<Uuid, Activity>>> = Mutex::new(None);

fn with_activity<R>(f: impl FnOnce(&mut HashMap<Uuid, Activity>) -> R) -> R {
    let mut guard = ACTIVITY.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

/// The current or last activity of a target.
pub fn activity(id: Uuid) -> Option<Activity> {
    with_activity(|m| m.get(&id).cloned())
}

/// Marks a target as busy; `false` if it already is.
fn begin(id: Uuid, action: &str, file: Option<String>) -> bool {
    with_activity(|m| {
        if m.get(&id).is_some_and(|a| !a.done) {
            return false;
        }
        m.insert(
            id,
            Activity {
                action: action.into(),
                file,
                started_at: Utc::now(),
                error: None,
                done: false,
            },
        );
        true
    })
}

fn finish(id: Uuid, error: Option<String>) {
    with_activity(|m| {
        if let Some(a) = m.get_mut(&id) {
            a.done = true;
            a.error = error;
        }
    })
}

pub fn is_running() -> bool {
    RUNNER.try_lock().is_err()
}

impl From<RcloneError> for BackupError {
    fn from(err: RcloneError) -> Self {
        match err {
            RcloneError::NotFound => BackupError::Invalid("not found on the backup target".into()),
            other => BackupError::Invalid(other.to_string()),
        }
    }
}

/// Whether `path` is one of the allowed roots or below one, also after
/// links are resolved. The path itself does not need to exist.
pub fn local_allowed(roots: &[PathBuf], path: &Path) -> bool {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    // The deepest existing ancestor, with links resolved.
    let mut existing = path;
    let resolved = loop {
        match existing.canonicalize() {
            Ok(p) => break p.join(path.strip_prefix(existing).unwrap_or(Path::new(""))),
            Err(_) => match existing.parent() {
                Some(parent) => existing = parent,
                None => return false,
            },
        }
    };
    roots.iter().any(|root| {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
        (path.starts_with(root) || path.starts_with(&canonical)) && resolved.starts_with(&canonical)
    })
}

/// Checks the rules that depend on the server: local roots and encryption.
pub fn check_connection(cfg: &BackupConfig, conn: &TargetConnection) -> Result<(), BackupError> {
    if matches!(conn.config, TargetConfig::Local)
        && !local_allowed(&cfg.target_roots, Path::new(&conn.path))
    {
        let roots: Vec<String> = cfg
            .target_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        return Err(BackupError::Invalid(format!(
            "a local target must be below {}",
            if roots.is_empty() {
                "TALKOPS_BACKUP_TARGET_ROOTS".to_owned()
            } else {
                roots.join(", ")
            }
        )));
    }
    Ok(())
}

/// Remote targets receive only encrypted archives.
pub fn check_encryption(cfg: &BackupConfig, config: &TargetConfig) -> Result<(), BackupError> {
    if config.is_remote() && cfg.password.is_none() {
        return Err(BackupError::Invalid(
            "remote backup targets need encrypted archives: set TALKOPS_BACKUP_PASSWORD or TALKOPS_BACKUP_PASSWORD_FILE".into(),
        ));
    }
    Ok(())
}

/// TalkOps archives in the target folder, newest first.
pub async fn archives(rclone: &Rclone, conn: &TargetConnection) -> Result<Vec<Entry>, BackupError> {
    let entries = match rclone.list(conn, "").await {
        Ok((entries, _)) => entries,
        Err(RcloneError::NotFound) => Vec::new(),
        Err(err) => return Err(err.into()),
    };
    let mut out: Vec<Entry> = entries
        .into_iter()
        .filter(|e| !e.is_dir && backup::valid_name(&e.name))
        .collect();
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Deletes archives that the retention of the target does not keep, and
/// upload leftovers older than six hours. Returns the number of deleted files.
pub async fn prune(
    cfg: &BackupConfig,
    target: &BackupTarget,
    conn: &TargetConnection,
) -> Result<usize, BackupError> {
    let entries = match cfg.rclone.list(conn, "").await {
        Ok((entries, _)) => entries,
        Err(RcloneError::NotFound) => return Ok(0),
        Err(err) => return Err(err.into()),
    };
    let files: Vec<&Entry> = entries.iter().filter(|e| !e.is_dir).collect();
    let archives: Vec<(&Entry, chrono::NaiveDateTime)> = files
        .iter()
        .filter_map(|e| backup::stamp(&e.name).map(|s| (*e, s)))
        .collect();
    let stamps: Vec<_> = archives.iter().map(|(_, s)| *s).collect();
    let keep = target.retention.keep(&stamps, Local::now().naive_local());
    let mut deleted = 0;
    for (i, (entry, _)) in archives.iter().enumerate() {
        if !keep.contains(&i) {
            cfg.rclone.delete(conn, &entry.name).await?;
            deleted += 1;
        }
    }
    let stale = Utc::now() - chrono::Duration::hours(6);
    for entry in files {
        if let Some(name) = entry.name.strip_suffix(".partial")
            && backup::valid_name(name)
            && entry.modified.is_some_and(|m| m < stale)
        {
            cfg.rclone.delete(conn, &entry.name).await?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Uploads one archive to one target and applies its retention.
async fn deliver(
    cfg: &BackupConfig,
    target: &BackupTarget,
    conn: &TargetConnection,
    archive: &Path,
) -> Result<String, BackupError> {
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| backup::valid_name(n))
        .ok_or_else(|| BackupError::Invalid("not a backup archive".into()))?
        .to_owned();
    check_connection(cfg, conn)?;
    check_encryption(cfg, &target.config)?;
    if target.config.is_remote() && !name.ends_with(".age") {
        return Err(BackupError::Invalid(
            "remote backup targets receive only encrypted archives".into(),
        ));
    }
    cfg.rclone
        .upload(conn, archive, &name, target.bwlimit_kbps)
        .await?;
    match prune(cfg, target, conn).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(target = %target.name, count = n, "old archives deleted on target"),
        Err(err) => {
            tracing::warn!(target = %target.name, error = %err, "cannot apply target retention")
        }
    }
    Ok(name)
}

/// Writes an archive into a staging folder; waits while another backup runs.
async fn stage(
    cfg: &BackupConfig,
    db: &PgPool,
    include_recordings: bool,
) -> Result<(tempfile::TempDir, PathBuf), BackupError> {
    std::fs::create_dir_all(&cfg.dir)?;
    let dir = tempfile::Builder::new()
        .prefix(".outgoing-")
        .tempdir_in(&cfg.dir)?;
    for attempt in 0..30 {
        match backup::create_in(cfg, db, include_recordings, dir.path()).await {
            Ok(name) => {
                let path = dir.path().join(name);
                return Ok((dir, path));
            }
            Err(BackupError::Busy) if attempt < 29 => {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            Err(err) => return Err(err),
        }
    }
    Err(BackupError::Busy)
}

/// Runs the given targets now. `fresh` is an archive in the backup
/// directory that was just written, with its recordings setting.
pub async fn run(
    cfg: &BackupConfig,
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    targets: Vec<BackupTarget>,
    fresh: Option<(String, bool)>,
) {
    let targets: Vec<BackupTarget> = targets
        .into_iter()
        .filter(|t| begin(t.id, "backup", None))
        .collect();
    for include_recordings in [true, false] {
        let group: Vec<BackupTarget> = targets
            .iter()
            .filter(|t| t.include_recordings == include_recordings)
            .cloned()
            .collect();
        if group.is_empty() {
            continue;
        }
        // The staging folder is deleted when it goes out of scope.
        let mut _staged = None;
        let archive = match &fresh {
            Some((name, rec)) if *rec == include_recordings => cfg.dir.join(name),
            _ => match stage(cfg, db, include_recordings).await {
                Ok((dir, path)) => {
                    _staged = Some(dir);
                    path
                }
                Err(err) => {
                    for t in &group {
                        let msg = err.to_string();
                        let _ = backup_targets::record_run(db, t.id, Err(&msg)).await;
                        finish(t.id, Some(msg));
                    }
                    continue;
                }
            },
        };
        for target in &group {
            let result = match backup_targets::connection(db, tenant, secrets, target.id).await {
                Ok((current, conn)) => deliver(cfg, &current, &conn, &archive).await,
                Err(err) => Err(BackupError::Invalid(err.to_string())),
            };
            match &result {
                Ok(name) => {
                    tracing::info!(target = %target.name, file = %name, "archive copied to backup target");
                    let _ = backup_targets::record_run(db, target.id, Ok(name)).await;
                    finish(target.id, None);
                }
                Err(err) => {
                    tracing::warn!(target = %target.name, error = %err, "backup target failed");
                    let msg = err.to_string();
                    let _ = backup_targets::record_run(db, target.id, Err(&msg)).await;
                    finish(target.id, Some(msg));
                }
            }
        }
    }
}

/// Starts due targets in the background (called every minute).
pub async fn tick(
    cfg: &std::sync::Arc<BackupConfig>,
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    fresh: Option<(String, bool)>,
) {
    let Ok(guard) = RUNNER.try_lock() else {
        return;
    };
    let targets = match backup_targets::list(db, tenant).await {
        Ok(t) => t,
        Err(err) => {
            tracing::warn!(error = %err, "cannot read backup targets");
            return;
        }
    };
    let now = Local::now().naive_local();
    let due: Vec<BackupTarget> = targets
        .into_iter()
        .filter(|t| {
            let since = t.last_run_at.unwrap_or(t.created_at);
            t.enabled
                && t.schedule
                    .due(since.with_timezone(&Local).naive_local(), now)
        })
        .collect();
    if due.is_empty() {
        return;
    }
    let (cfg, db, secrets) = (cfg.clone(), db.clone(), secrets.clone());
    tokio::spawn(async move {
        let _guard = guard;
        run(&cfg, &db, &secrets, tenant, due, fresh).await;
    });
}

/// Starts one target now; `false` if a run is already active.
pub fn start(
    cfg: std::sync::Arc<BackupConfig>,
    db: PgPool,
    secrets: SecretBox,
    tenant: TenantId,
    target: BackupTarget,
) -> bool {
    let Ok(guard) = RUNNER.try_lock() else {
        return false;
    };
    if activity(target.id).is_some_and(|a| !a.done) {
        return false;
    }
    tokio::spawn(async move {
        let _guard = guard;
        run(&cfg, &db, &secrets, tenant, vec![target], None).await;
    });
    true
}

/// Downloads an archive from a target into `dir` (a new file there).
/// Checks the name, the size limit and the free space first. The caller
/// checks the connection ([`check_connection`]).
pub async fn fetch(
    rclone: &Rclone,
    conn: &TargetConnection,
    name: &str,
    dir: &Path,
    max_bytes: u64,
) -> Result<PathBuf, BackupError> {
    if !backup::valid_name(name) {
        return Err(BackupError::Invalid("not a backup archive name".into()));
    }
    std::fs::create_dir_all(dir)?;
    let target = dir.join(name);
    if target.exists() {
        return Err(BackupError::Invalid(
            "an archive with this name is already on this server".into(),
        ));
    }
    let stat = rclone.stat(conn, name).await?;
    let size = u64::try_from(stat.size).unwrap_or(0);
    if size > max_bytes {
        return Err(BackupError::Invalid(
            "the archive is larger than the restore limit".into(),
        ));
    }
    if fs2::available_space(dir)? < size + 64 * 1024 * 1024 {
        return Err(BackupError::Invalid(
            "not enough free space for the archive".into(),
        ));
    }
    let part = dir.join(format!(".fetch-{}.partial", Uuid::new_v4().simple()));
    let result = rclone.download(conn, name, &part).await;
    let checked = result.map_err(BackupError::from).and_then(|()| {
        let len = std::fs::metadata(&part)?.len();
        if len != size {
            return Err(BackupError::Invalid(
                "the downloaded archive has the wrong size".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o600))?;
        }
        if target.exists() {
            return Err(BackupError::Invalid(
                "an archive with this name is already on this server".into(),
            ));
        }
        std::fs::rename(&part, &target)?;
        Ok(())
    });
    if checked.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    checked.map(|()| target)
}

/// Downloads an archive into the backup directory in the background.
pub fn start_fetch(
    cfg: std::sync::Arc<BackupConfig>,
    target: BackupTarget,
    conn: TargetConnection,
    name: String,
) -> bool {
    if !begin(target.id, "fetch", Some(name.clone())) {
        return false;
    }
    tokio::spawn(async move {
        let result = fetch(&cfg.rclone, &conn, &name, &cfg.dir, cfg.restore_max_bytes).await;
        match &result {
            Ok(_) => {
                tracing::info!(target = %target.name, file = %name, "archive downloaded from backup target")
            }
            Err(err) => {
                tracing::warn!(target = %target.name, error = %err, "archive download failed")
            }
        }
        finish(target.id, result.err().map(|e| e.to_string()));
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_roots() {
        let root = tempfile::tempdir().unwrap();
        let roots = vec![root.path().to_owned()];
        assert!(local_allowed(&roots, root.path()));
        assert!(local_allowed(&roots, &root.path().join("usb/talkops")));
        assert!(!local_allowed(&roots, Path::new("/etc")));
        assert!(!local_allowed(&roots, &root.path().join("../x")));
        assert!(!local_allowed(&roots, Path::new("relative")));
        assert!(!local_allowed(&[], root.path()));
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
            assert!(!local_allowed(&roots, &root.path().join("link/sub")));
        }
    }
}
