//! Backups: one `talkops-YYYYMMDD-HHMMSS.tar.gz` with a database dump
//! (`pg_dump` custom format) and the data volumes. A restore replaces the
//! database and the volumes' contents with the archive's. Complete archives
//! use the password-encrypted age envelope and the .tar.gz.age extension.
//!
//! Archive layout:
//! ```text
//! manifest.json          format, versions, volumes, SHA-256 inventory, deployment status
//! database.dump          pg_dump --format=custom
//! files/<volume>/...     voicemail, sounds, recordings, snapshots, provisioning, cdr-spool
//! deployment/<file>      selected Compose and environment files (encrypted archives only)
//! ```

use crate::rclone::Rclone;
use crate::recovery::{self, DeploymentInfo, FileInfo};
use age::secrecy::SecretString;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use talkops_core::backups;
use talkops_core::tenant::TenantId;

use crate::config::Storage;

const FORMAT: u32 = 2;
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
    pub password: Option<SecretString>,
    pub deployment_dir: Option<PathBuf>,
    pub deployment_files: Vec<String>,
    pub deployment_key: Option<SecretString>,
    pub project_name: String,
    pub profiles: String,
    pub restore_max_bytes: u64,
    /// Transfer tool for backup targets.
    pub rclone: Rclone,
    /// Allowed directories of local backup targets.
    pub target_roots: Vec<PathBuf>,
}

impl BackupConfig {
    pub fn from_storage(s: &Storage) -> Result<Self> {
        let password = recovery::password(
            s.backup_password.as_deref(),
            s.backup_password_file.as_deref(),
        )?;
        Ok(Self {
            database_url: s.database_url.clone(),
            dir: s.backup_dir.clone(),
            pg_bin_dir: s.pg_bin_dir.clone(),
            password,
            deployment_dir: s
                .deployment_dir
                .clone()
                .filter(|p| !p.as_os_str().is_empty()),
            deployment_files: s.deployment_files.clone(),
            deployment_key: std::env::var("TALKOPS_SECRET_KEY")
                .ok()
                .map(SecretString::from),
            project_name: s.compose_project.clone(),
            profiles: s.compose_profiles.clone(),
            restore_max_bytes: s.restore_max_bytes,
            rclone: Rclone::new(s.rclone.clone()),
            target_roots: s
                .backup_target_roots
                .iter()
                .filter(|p| !p.as_os_str().is_empty())
                .cloned()
                .collect(),
            volumes: vec![
                ("fax", s.fax_dir.clone()),
                ("voicemail", s.voicemail_dir.clone()),
                ("sounds", s.sounds_dir.clone()),
                ("recordings", s.recordings_dir.clone()),
                ("snapshots", s.snapshots_dir.clone()),
                ("provisioning", s.provisioning_dir.clone()),
                ("cdr-spool", s.cdr_spool_dir.clone()),
            ],
        })
    }

    pub fn tool(&self, name: &str) -> PathBuf {
        match &self.pg_bin_dir {
            Some(dir) => dir.join(name),
            None => PathBuf::from(name),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Manifest {
    pub format: u32,
    pub talkops_version: String,
    pub created_at: DateTime<Utc>,
    /// Latest applied migration.
    pub schema_version: i64,
    pub volumes: Vec<String>,
    #[serde(default)]
    pub inventory: BTreeMap<String, FileInfo>,
    #[serde(default)]
    pub deployment: Option<DeploymentInfo>,
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
    name.strip_suffix(".age")
        .unwrap_or(name)
        .strip_prefix(PREFIX)
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

/// Cross-process lock. The file remains; the kernel releases its lock on exit.
fn file_lock(dir: &Path) -> Result<File> {
    std::fs::create_dir_all(dir)?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".backup.lock"))?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|_| BackupError::Busy)?;
    Ok(file)
}

fn append_source<W: Write>(
    tar: &mut tar::Builder<W>,
    source: &Path,
    name: &str,
    inventory: &mut BTreeMap<String, FileInfo>,
    entries: &mut usize,
) -> Result<()> {
    let meta = std::fs::symlink_metadata(source)?;
    if meta.is_symlink() {
        return Ok(());
    } // Do not archive links or their targets.
    *entries += 1;
    if *entries > recovery::MAX_ENTRIES {
        return Err(BackupError::Invalid(
            "backup exceeds the archive entry limit".into(),
        ));
    }
    if meta.is_dir() {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o700);
        header.set_cksum();
        tar.append_data(&mut header, name, std::io::empty())?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let component = entry.file_name();
            let component = component
                .to_str()
                .ok_or_else(|| BackupError::Invalid("non-UTF-8 archive path".into()))?;
            if component.contains(['\\', ':']) {
                return Err(BackupError::Invalid("unsupported archive path".into()));
            }
            append_source(
                tar,
                &entry.path(),
                &format!("{name}/{component}"),
                inventory,
                entries,
            )?;
        }
    } else if meta.is_file() {
        // Hash the exact bytes read by tar, rather than a separate file read.
        struct HashReader {
            file: File,
            hash: sha2::Sha256,
            size: u64,
        }
        impl Read for HashReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                use sha2::Digest;
                let n = self.file.read(buf)?;
                self.hash.update(&buf[..n]);
                self.size += n as u64;
                Ok(n)
            }
        }
        use sha2::Digest;
        let mut reader = HashReader {
            file: File::open(source)?,
            hash: sha2::Sha256::new(),
            size: 0,
        };
        let mut header = tar::Header::new_gnu();
        header.set_size(meta.len());
        header.set_mode(0o600);
        header.set_cksum();
        tar.append_data(&mut header, name, &mut reader)?;
        inventory.insert(
            name.into(),
            FileInfo {
                size: reader.size,
                sha256: hex::encode(reader.hash.finalize()),
            },
        );
    }
    Ok(())
}

fn write_archive<W: Write>(
    out: W,
    dump: &Path,
    volumes: &[(&str, PathBuf)],
    deployment: &recovery::DeploymentInput,
    mut manifest: Manifest,
) -> Result<W> {
    let gz = flate2::write::GzEncoder::new(out, flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    let mut entries = 0;
    append_source(
        &mut tar,
        dump,
        "database.dump",
        &mut manifest.inventory,
        &mut entries,
    )?;
    for (vol, dir) in volumes {
        if dir.is_dir() {
            if std::fs::symlink_metadata(dir)?.is_symlink() {
                return Err(BackupError::Invalid(
                    "backup volume root cannot be a symlink".into(),
                ));
            }
            append_source(
                &mut tar,
                dir,
                &format!("files/{vol}"),
                &mut manifest.inventory,
                &mut entries,
            )?;
        }
    }
    use sha2::Digest;
    for (name, value) in &deployment.files {
        let name = format!("deployment/{name}");
        recovery::append_bytes(&mut tar, &name, value.as_bytes())?;
        manifest.inventory.insert(
            name,
            FileInfo {
                size: value.len() as u64,
                sha256: hex::encode(sha2::Sha256::digest(value.as_bytes())),
            },
        );
    }
    let json = serde_json::to_vec_pretty(&manifest).expect("manifest");
    if json.len() as u64 > recovery::MANIFEST_LIMIT
        || entries + deployment.files.len() + 1 > recovery::MAX_ENTRIES
    {
        return Err(BackupError::Invalid(
            "backup exceeds manifest or entry limits; reduce included data".into(),
        ));
    }
    recovery::append_bytes(&mut tar, "manifest.json", &json)?;
    Ok(tar.into_inner()?.finish()?)
}

/// The creation time in an archive name (local time).
pub fn stamp(name: &str) -> Option<chrono::NaiveDateTime> {
    if !valid_name(name) {
        return None;
    }
    let stamp = &name[PREFIX.len()..PREFIX.len() + 15];
    chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S").ok()
}

/// Writes a checksum inventory and encrypts the archive when a password is set.
pub async fn create(cfg: &BackupConfig, db: &PgPool, include_recordings: bool) -> Result<String> {
    create_in(cfg, db, include_recordings, &cfg.dir).await
}

/// Like [`create`], but writes the archive into `out_dir`: a directory in
/// the backup directory (same file system), e.g. for archives that only
/// go to backup targets.
pub async fn create_in(
    cfg: &BackupConfig,
    db: &PgPool,
    include_recordings: bool,
    out_dir: &Path,
) -> Result<String> {
    // The stack from Portainer, when connected (ADR 0032).
    crate::portainer::refresh(cfg, db).await;
    let _guard = BUSY.try_lock().map_err(|_| BackupError::Busy)?;
    let _file_lock = file_lock(&cfg.dir)?;
    let _db_lock = talkops_core::db::operation_guard(&cfg.database_url, false).await?;
    let (deployment, mut info) = recovery::capture(cfg)?;
    if !deployment.files.is_empty() && cfg.deployment_key.is_none() {
        return Err(BackupError::Invalid(
            "TALKOPS_SECRET_KEY is required when capturing deployment files".into(),
        ));
    }
    if !deployment.files.is_empty() && cfg.password.is_none() {
        return Err(BackupError::Invalid(
            "deployment files require TALKOPS_BACKUP_PASSWORD".into(),
        ));
    }
    let now = Utc::now();
    let name = format!(
        "{PREFIX}{}{SUFFIX}{}",
        now.with_timezone(&Local).format("%Y%m%d-%H%M%S"),
        if cfg.password.is_some() { ".age" } else { "" }
    );
    if out_dir.join(&name).exists() {
        return Err(BackupError::Busy);
    }
    let temp = tempfile::tempdir_in(&cfg.dir)?;
    let dump = temp.path().join("database.dump");
    run_tool(
        cfg,
        "pg_dump",
        &[
            "--format=custom".as_ref(),
            "--no-owner".as_ref(),
            "--no-privileges".as_ref(),
            "--file".as_ref(),
            dump.as_os_str(),
        ],
    )
    .await?;
    info.postgres_version = sqlx::query_scalar("SELECT current_setting('server_version')")
        .fetch_one(db)
        .await?;
    let volumes: Vec<_> = cfg
        .volumes
        .iter()
        .filter(|(n, _)| include_recordings || *n != "recordings")
        .cloned()
        .collect();
    let manifest = Manifest {
        format: FORMAT,
        talkops_version: talkops_core::VERSION.into(),
        created_at: now,
        schema_version: schema_version(db).await?,
        volumes: volumes.iter().map(|(n, _)| (*n).into()).collect(),
        inventory: BTreeMap::new(),
        deployment: Some(info),
    };
    let part = temp.path().join("archive.part");
    let target = out_dir.join(&name);
    let password = cfg.password.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = recovery::private_file(&part)?;
        if let Some(password) = password {
            let mut recipient = age::scrypt::Recipient::new(password);
            recipient.set_work_factor(16);
            let writer =
                age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
                    .map_err(|_| {
                        BackupError::Invalid("cannot initialize backup encryption".into())
                    })?
                    .wrap_output(file)?;
            write_archive(writer, &dump, &volumes, &deployment, manifest)?
                .finish()?
                .sync_all()?;
        } else {
            write_archive(file, &dump, &volumes, &deployment, manifest)?.sync_all()?;
        }
        std::fs::rename(part, target)?;
        drop(temp);
        Ok(())
    })
    .await
    .map_err(|_| BackupError::Invalid("backup task failed".into()))??;
    Ok(name)
}

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
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    if !source.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Requires stopped writers; stages and validates every file before replacement.
pub async fn restore(cfg: &BackupConfig, db: &PgPool, archive: &Path) -> Result<Manifest> {
    let _guard = BUSY.try_lock().map_err(|_| BackupError::Busy)?;
    let _file_lock = file_lock(&cfg.dir)?;
    let _db_lock = talkops_core::db::operation_guard(&cfg.database_url, true)
        .await
        .map_err(|_| {
            BackupError::Invalid(
                "stop TalkOps and the media worker before restore; another restore may be running"
                    .into(),
            )
        })?;
    let (src, password, limit) = (
        archive.to_owned(),
        cfg.password.clone(),
        cfg.restore_max_bytes,
    );
    let prepared =
        tokio::task::spawn_blocking(move || recovery::prepare(&src, password.as_ref(), limit))
            .await
            .map_err(|_| BackupError::Invalid("restore validation task failed".into()))??;
    if let Some(deployment) = &prepared.manifest.deployment {
        if deployment.files.iter().any(|n| n == ".env") {
            use age::secrecy::ExposeSecret;
            let env = std::fs::read_to_string(prepared.root.path().join("deployment/.env"))?;
            if let Some(expected) = recovery::env_value(&env, "TALKOPS_SECRET_KEY")
                .filter(|k| !k.is_empty() && !k.contains('$'))
            {
                let actual = cfg.deployment_key.as_ref().ok_or_else(|| BackupError::Invalid("apply the exported TALKOPS_SECRET_KEY before restoring this complete backup".into()))?;
                if hex::decode(expected.trim()).ok()
                    != hex::decode(actual.expose_secret().trim()).ok()
                {
                    return Err(BackupError::Invalid("apply the original application key from the exported configuration before restore".into()));
                }
            }
        }
    }
    if prepared
        .manifest
        .volumes
        .iter()
        .any(|n| !cfg.volumes.iter().any(|(v, _)| v == n))
    {
        return Err(BackupError::Invalid(
            "a backup volume has no restore target".into(),
        ));
    }
    let dump = prepared.root.path().join("database.dump");
    run_tool(cfg, "pg_restore", &["--list".as_ref(), dump.as_os_str()]).await?;
    run_tool(
        cfg,
        "pg_restore",
        &[
            "--file".as_ref(),
            if cfg!(windows) {
                "NUL".as_ref()
            } else {
                "/dev/null".as_ref()
            },
            dump.as_os_str(),
        ],
    )
    .await?;
    for (name, target) in &cfg.volumes {
        if prepared.manifest.volumes.iter().any(|v| v == name) {
            if std::fs::symlink_metadata(target).is_ok_and(|m| m.is_symlink()) {
                return Err(BackupError::Invalid(
                    "restore target cannot be a symlink".into(),
                ));
            }
            std::fs::create_dir_all(target)?;
            let bytes: u64 = prepared
                .manifest
                .inventory
                .iter()
                .filter(|(p, _)| p.starts_with(&format!("files/{name}/")))
                .map(|(_, f)| f.size)
                .sum();
            if fs2::available_space(target)? < bytes {
                return Err(BackupError::Invalid(
                    "insufficient space in restore target".into(),
                ));
            }
        }
    }
    let state_path = cfg.dir.join("restore-state.json");
    let state = |phase: &str| -> Result<()> {
        std::fs::write(
            &state_path,
            serde_json::to_vec(&serde_json::json!({"phase":phase,"updated_at":Utc::now()}))
                .expect("state"),
        )?;
        Ok(())
    };
    state("database")?;
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
            dump.as_os_str(),
        ],
    )
    .await?;
    talkops_core::db::migrate(db)
        .await
        .map_err(|e| BackupError::Invalid(format!("migrations after restore: {e}")))?;
    sqlx::query("TRUNCATE sessions, login_challenges")
        .execute(db)
        .await?;
    state("volumes")?;
    let volumes = cfg.volumes.clone();
    let manifest = tokio::task::spawn_blocking(move || -> Result<Manifest> {
        for (name, target) in volumes {
            if prepared.manifest.volumes.iter().any(|v| v == name) {
                clear_dir(&target)?;
                copy_tree(&prepared.root.path().join("files").join(name), &target)?;
            }
        }
        Ok(prepared.manifest)
    })
    .await
    .map_err(|_| BackupError::Invalid("volume restore task failed".into()))??;
    talkops_core::audit::record(db, &talkops_core::audit::Actor {tenant:TenantId::DEFAULT,user_id:None,ip:None}, "restore", "backup", None, serde_json::json!({"backup_created_at":manifest.created_at,"talkops_version":manifest.talkops_version,"volumes":manifest.volumes}))
        .await.map_err(|_| BackupError::Invalid("restore completed but audit recording failed".into()))?;
    state("complete")?;
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

/// Checks every minute whether the daily backup or a backup target is due.
/// A target that is due in the same minute as the daily backup receives
/// that archive; other runs create an archive only for the targets.
pub fn spawn(db: PgPool, cfg: Arc<BackupConfig>, secrets: talkops_core::crypto::SecretBox) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let tenant = TenantId::DEFAULT;
            let Ok(settings) = backups::get(&db, tenant).await else {
                continue;
            };
            let mut fresh = None;
            if due(&settings, Local::now()) && !is_busy() {
                match run_scheduled(&cfg, &db, tenant).await {
                    Ok(name) => {
                        tracing::info!(file = %name, "backup written");
                        fresh = Some((name, settings.include_recordings));
                    }
                    Err(err) => tracing::warn!(error = %err, "backup failed"),
                }
            }
            crate::backup_targets::tick(&cfg, &db, &secrets, tenant, fresh).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_env() {
        assert!(valid_name("talkops-20261012-030000.tar.gz"));
        assert!(valid_name("talkops-20261012-030000.tar.gz.age"));
        assert!(!valid_name("talkops-20261012-030000.tar.gz.age/../x"));
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
    }
}
