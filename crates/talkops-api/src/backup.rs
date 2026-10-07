//! Backups: one `talkops-YYYYMMDD-HHMMSS.tar.gz` with a database dump
//! (`pg_dump` custom format) and the data volumes. A restore replaces the
//! database and the volumes' contents with the archive's.
//!
//! Archive layout:
//! ```text
//! manifest.json          format, version, schema version, included volumes
//! database.dump          pg_dump --format=custom
//! files/<volume>/...     voicemail, sounds, recordings, snapshots, provisioning
//! ```

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use talkops_core::backups;
use talkops_core::tenant::TenantId;

use crate::config::Storage;

const FORMAT: u32 = 1;
const PREFIX: &str = "talkops-";
const SUFFIX: &str = ".tar.gz";

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("a backup or restore is already running")]
    Busy,
    #[error("{0}")]
    Invalid(String),
    #[error("{tool} failed: {message}")]
    Tool { tool: &'static str, message: String },
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
}

pub type Result<T, E = BackupError> = std::result::Result<T, E>;

/// What a backup contains and where it goes.
#[derive(Debug, Clone)]
pub struct BackupConfig {
    pub database_url: String,
    pub dir: PathBuf,
    pub pg_bin_dir: Option<PathBuf>,
    /// Volume name in the archive and its directory.
    pub volumes: Vec<(&'static str, PathBuf)>,
}

impl BackupConfig {
    pub fn from_storage(s: &Storage) -> Self {
        Self {
            database_url: s.database_url.clone(),
            dir: s.backup_dir.clone(),
            pg_bin_dir: s.pg_bin_dir.clone(),
            volumes: vec![
                ("voicemail", s.voicemail_dir.clone()),
                ("sounds", s.sounds_dir.clone()),
                ("recordings", s.recordings_dir.clone()),
                ("snapshots", s.snapshots_dir.clone()),
                ("provisioning", s.provisioning_dir.clone()),
            ],
        }
    }

    fn tool(&self, name: &str) -> PathBuf {
        match &self.pg_bin_dir {
            Some(dir) => dir.join(name),
            None => PathBuf::from(name),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub talkops_version: String,
    pub created_at: DateTime<Utc>,
    /// Latest applied migration.
    pub schema_version: i64,
    pub volumes: Vec<String>,
}

/// A backup archive in the backup directory.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct BackupFile {
    pub name: String,
    pub size: u64,
    pub created_at: DateTime<Utc>,
}

/// Only one backup or restore at a time.
static BUSY: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub fn is_busy() -> bool {
    BUSY.try_lock().is_err()
}

/// Whether `name` is an archive name this module creates (no paths).
pub fn valid_name(name: &str) -> bool {
    name.strip_prefix(PREFIX)
        .and_then(|n| n.strip_suffix(SUFFIX))
        .is_some_and(|stamp| {
            stamp.len() == 15
                && stamp.bytes().enumerate().all(|(i, b)| {
                    if i == 8 {
                        b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                })
        })
}

/// Archives in the backup directory, newest first.
pub async fn list(dir: &Path) -> Result<Vec<BackupFile>> {
    let mut out = Vec::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(err) => return Err(err.into()),
    };
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !valid_name(&name) {
            continue;
        }
        let meta = entry.metadata().await?;
        out.push(BackupFile {
            name,
            size: meta.len(),
            created_at: meta.modified().map(DateTime::<Utc>::from)?,
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Deletes the oldest archives beyond `keep`; returns how many.
pub async fn prune(dir: &Path, keep: usize) -> Result<usize> {
    let files = list(dir).await?;
    let mut n = 0;
    for f in files.iter().skip(keep) {
        tokio::fs::remove_file(dir.join(&f.name)).await?;
        n += 1;
    }
    Ok(n)
}

/// libpq environment for a `postgres://` URL, so the password never shows
/// up in a process list.
fn pg_env(database_url: &str) -> Result<Vec<(&'static str, String)>> {
    let url = url::Url::parse(database_url)
        .map_err(|e| BackupError::Invalid(format!("database URL: {e}")))?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        return Err(BackupError::Invalid("database URL: not postgres://".into()));
    }
    let decode = |s: &str| {
        percent_encoding::percent_decode_str(s)
            .decode_utf8_lossy()
            .into_owned()
    };
    let mut env = vec![("PGCONNECT_TIMEOUT", "10".to_owned())];
    if let Some(host) = url.host_str() {
        env.push(("PGHOST", host.trim_matches(['[', ']']).to_owned()));
    }
    if let Some(port) = url.port() {
        env.push(("PGPORT", port.to_string()));
    }
    if !url.username().is_empty() {
        env.push(("PGUSER", decode(url.username())));
    }
    if let Some(pw) = url.password() {
        env.push(("PGPASSWORD", decode(pw)));
    }
    let db = url.path().trim_start_matches('/');
    if !db.is_empty() {
        env.push(("PGDATABASE", decode(db)));
    }
    for (k, v) in url.query_pairs() {
        let var = match k.as_ref() {
            "host" => "PGHOST",
            "port" => "PGPORT",
            "user" => "PGUSER",
            "password" => "PGPASSWORD",
            "dbname" => "PGDATABASE",
            "sslmode" => "PGSSLMODE",
            "sslrootcert" => "PGSSLROOTCERT",
            "application_name" => "PGAPPNAME",
            _ => continue,
        };
        env.retain(|(name, _)| *name != var);
        env.push((var, v.into_owned()));
    }
    Ok(env)
}

async fn run_tool(cfg: &BackupConfig, tool: &'static str, args: &[&std::ffi::OsStr]) -> Result<()> {
    let out = tokio::process::Command::new(cfg.tool(tool))
        .args(args)
        .envs(pg_env(&cfg.database_url)?)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| BackupError::Tool {
            tool,
            message: e.to_string(),
        })?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(BackupError::Tool {
            tool,
            message: stderr.trim().chars().take(500).collect(),
        })
    }
}

async fn schema_version(db: &PgPool) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT COALESCE(max(version), 0) FROM _sqlx_migrations WHERE success")
            .fetch_one(db)
            .await?,
    )
}

/// Removes a file when dropped (temporary dumps, partial archives).
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Writes a new archive; returns its file name.
pub async fn create(cfg: &BackupConfig, db: &PgPool, include_recordings: bool) -> Result<String> {
    let _guard = BUSY.try_lock().map_err(|_| BackupError::Busy)?;
    tokio::fs::create_dir_all(&cfg.dir).await?;
    let now = Utc::now();
    let name = format!(
        "{PREFIX}{}{SUFFIX}",
        now.with_timezone(&Local).format("%Y%m%d-%H%M%S")
    );
    if tokio::fs::try_exists(cfg.dir.join(&name)).await? {
        return Err(BackupError::Busy);
    }
    let dump = TempFile(cfg.dir.join(format!(".{name}.dump")));
    run_tool(
        cfg,
        "pg_dump",
        &[
            "--format=custom".as_ref(),
            "--no-owner".as_ref(),
            "--no-privileges".as_ref(),
            "--file".as_ref(),
            dump.0.as_os_str(),
        ],
    )
    .await?;
    let volumes: Vec<(&'static str, PathBuf)> = cfg
        .volumes
        .iter()
        .filter(|(n, _)| include_recordings || *n != "recordings")
        .cloned()
        .collect();
    let manifest = Manifest {
        format: FORMAT,
        talkops_version: talkops_core::VERSION.to_owned(),
        created_at: now,
        schema_version: schema_version(db).await?,
        volumes: volumes.iter().map(|(n, _)| (*n).to_owned()).collect(),
    };
    let part = TempFile(cfg.dir.join(format!(".{name}.part")));
    let target = cfg.dir.join(&name);
    let part_path = part.0.clone();
    let dump_path = dump.0.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = File::create(&part_path)?;
        let gz = flate2::write::GzEncoder::new(BufWriter::new(file), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        tar.follow_symlinks(false);
        let json = serde_json::to_vec_pretty(&manifest).expect("manifest serializes");
        let mut header = tar::Header::new_gnu();
        header.set_size(json.len() as u64);
        header.set_mode(0o600);
        header.set_mtime(manifest.created_at.timestamp().max(0) as u64);
        header.set_cksum();
        tar.append_data(&mut header, "manifest.json", json.as_slice())?;
        tar.append_path_with_name(&dump_path, "database.dump")?;
        for (vol, dir) in &volumes {
            if dir.is_dir() {
                tar.append_dir_all(format!("files/{vol}"), dir)?;
            }
        }
        let mut out = tar.into_inner()?.finish()?;
        out.flush()?;
        out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        std::fs::rename(&part_path, &target)?;
        Ok(())
    })
    .await
    .map_err(|e| BackupError::Invalid(e.to_string()))??;
    drop(part);
    Ok(name)
}

/// One path component list below `files/<volume>/`, or `None` if unsafe.
fn safe_relative(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

fn open_archive(path: &Path) -> Result<tar::Archive<flate2::read::GzDecoder<BufReader<File>>>> {
    let file = File::open(path)?;
    Ok(tar::Archive::new(flate2::read::GzDecoder::new(
        BufReader::new(file),
    )))
}

/// Reads the manifest and extracts the dump to `dump_to`.
fn read_head(archive: &Path, dump_to: &Path) -> Result<Manifest> {
    let mut manifest = None;
    let mut dumped = false;
    for entry in open_archive(archive)?.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path == Path::new("manifest.json") {
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut std::io::Read::take(&mut entry, 1 << 20), &mut buf)?;
            manifest = Some(
                serde_json::from_slice::<Manifest>(&buf)
                    .map_err(|e| BackupError::Invalid(format!("manifest: {e}")))?,
            );
        } else if path == Path::new("database.dump") {
            entry.unpack(dump_to)?;
            dumped = true;
        }
        if manifest.is_some() && dumped {
            break;
        }
    }
    let manifest = manifest
        .ok_or_else(|| BackupError::Invalid("not a TalkOps backup (no manifest)".into()))?;
    if manifest.format != FORMAT {
        return Err(BackupError::Invalid(format!(
            "unsupported backup format {}",
            manifest.format
        )));
    }
    if !dumped {
        return Err(BackupError::Invalid(
            "the backup has no database dump".into(),
        ));
    }
    Ok(manifest)
}

/// Deletes everything inside `dir` (a volume mount point stays).
fn clear_dir(dir: &Path) -> std::io::Result<()> {
    if !dir.is_dir() {
        return std::fs::create_dir_all(dir);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Replaces the volumes listed in the manifest with the archive's files.
fn restore_files(
    archive: &Path,
    manifest: &Manifest,
    volumes: &[(&'static str, PathBuf)],
) -> Result<()> {
    let targets: Vec<(&str, &PathBuf)> = volumes
        .iter()
        .filter(|(n, _)| manifest.volumes.iter().any(|v| v == n))
        .map(|(n, d)| (*n, d))
        .collect();
    for (_, dir) in &targets {
        clear_dir(dir)?;
    }
    for entry in open_archive(archive)?.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let Ok(rest) = path.strip_prefix("files") else {
            continue;
        };
        let mut parts = rest.components();
        let Some(Component::Normal(vol)) = parts.next() else {
            continue;
        };
        let Some((_, dir)) = targets.iter().find(|(n, _)| *n == vol) else {
            continue;
        };
        let Some(rel) = safe_relative(parts.as_path()) else {
            return Err(BackupError::Invalid(format!(
                "unsafe path {}",
                path.display()
            )));
        };
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            // No links or devices from an archive.
            continue;
        }
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = dir.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&dest)?;
    }
    Ok(())
}

/// Restores an archive: replaces the database (then applies newer
/// migrations) and the contents of the volumes in the archive. Stop the
/// server and the media worker first.
pub async fn restore(cfg: &BackupConfig, db: &PgPool, archive: &Path) -> Result<Manifest> {
    let _guard = BUSY.try_lock().map_err(|_| BackupError::Busy)?;
    tokio::fs::create_dir_all(&cfg.dir).await?;
    let dump = TempFile(
        cfg.dir
            .join(format!(".restore-{}.dump", std::process::id())),
    );
    let (src, dump_path) = (archive.to_owned(), dump.0.clone());
    let manifest = tokio::task::spawn_blocking(move || read_head(&src, &dump_path))
        .await
        .map_err(|e| BackupError::Invalid(e.to_string()))??;
    let latest = talkops_core::db::MIGRATOR
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap_or(0);
    if manifest.schema_version > latest {
        return Err(BackupError::Invalid(format!(
            "the backup is from a newer TalkOps ({}); update first",
            manifest.talkops_version
        )));
    }

    // Empty schema, then the dump; migrations bring an older backup up to date.
    let dbname = pg_env(&cfg.database_url)?
        .into_iter()
        .find(|(k, _)| *k == "PGDATABASE")
        .map(|(_, v)| v)
        .unwrap_or_else(|| "postgres".into());
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(db)
        .await?;
    sqlx::query("CREATE SCHEMA public").execute(db).await?;
    run_tool(
        cfg,
        "pg_restore",
        &[
            "--no-owner".as_ref(),
            "--no-privileges".as_ref(),
            "--exit-on-error".as_ref(),
            "--single-transaction".as_ref(),
            "--dbname".as_ref(),
            dbname.as_ref(),
            dump.0.as_os_str(),
        ],
    )
    .await?;
    talkops_core::db::migrate(db)
        .await
        .map_err(|e| BackupError::Invalid(format!("migrations after restore: {e}")))?;

    let (src, volumes, m) = (archive.to_owned(), cfg.volumes.clone(), manifest);
    let manifest =
        tokio::task::spawn_blocking(move || restore_files(&src, &m, &volumes).map(|()| m))
            .await
            .map_err(|e| BackupError::Invalid(e.to_string()))??;
    Ok(manifest)
}

/// Runs a backup and prunes old ones; records the outcome.
pub async fn run_scheduled(cfg: &BackupConfig, db: &PgPool, tenant: TenantId) -> Result<String> {
    let settings = backups::get(db, tenant)
        .await
        .map_err(|e| BackupError::Invalid(e.to_string()))?;
    match create(cfg, db, settings.include_recordings).await {
        Ok(name) => {
            let _ = backups::record_run(db, tenant, Ok(&name)).await;
            match prune(&cfg.dir, settings.keep.max(1) as usize).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(count = n, "old backups deleted"),
                Err(err) => tracing::warn!(error = %err, "cannot delete old backups"),
            }
            Ok(name)
        }
        Err(err) => {
            let _ = backups::record_run(db, tenant, Err(&err.to_string())).await;
            Err(err)
        }
    }
}

/// Whether the daily backup is due at `now` (local time).
pub fn due(settings: &backups::BackupSettings, now: DateTime<Local>) -> bool {
    if !settings.enabled || now.hour() < settings.hour as u32 {
        return false;
    }
    match settings.last_run_at {
        None => true,
        Some(last) => last.with_timezone(&Local).date_naive() < now.date_naive(),
    }
}

/// Checks every minute whether the daily backup is due.
pub fn spawn(db: PgPool, cfg: Arc<BackupConfig>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let tenant = TenantId::DEFAULT;
            let Ok(settings) = backups::get(&db, tenant).await else {
                continue;
            };
            if !due(&settings, Local::now()) || is_busy() {
                continue;
            }
            match run_scheduled(&cfg, &db, tenant).await {
                Ok(name) => tracing::info!(file = %name, "backup written"),
                Err(err) => tracing::warn!(error = %err, "backup failed"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_env() {
        assert!(valid_name("talkops-20261012-030000.tar.gz"));
        assert!(!valid_name("talkops-20261012-030000.tar.gz/../x"));
        assert!(!valid_name("../talkops-20261012-030000.tar.gz"));
        assert!(!valid_name("talkops-2026101-0300000.tar.gz"));
        let env = pg_env("postgres://tal%40k:p%2Fw@db.local:5433/talk?sslmode=require").unwrap();
        let get = |k| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("PGHOST"), Some("db.local"));
        assert_eq!(get("PGPORT"), Some("5433"));
        assert_eq!(get("PGUSER"), Some("tal@k"));
        assert_eq!(get("PGPASSWORD"), Some("p/w"));
        assert_eq!(get("PGDATABASE"), Some("talk"));
        assert_eq!(get("PGSSLMODE"), Some("require"));
        assert!(pg_env("mysql://x").is_err());
        assert!(safe_relative(Path::new("a/../b")).is_none());
        assert_eq!(safe_relative(Path::new("a/b")), Some(PathBuf::from("a/b")));
    }
}
