//! Backup targets (ADR 0031): local folders, SMB shares, S3 buckets, FTP,
//! WebDAV and SFTP servers that receive copies of the backup archives. Each
//! target has its own schedule and retention. The server copies the archives
//! with rclone (`talkops_api::backup_targets`). Passwords and keys are stored
//! encrypted (ADR 0008) and never returned.

use std::collections::HashSet;

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

/// S3 providers that the UI offers. rclone knows more; `Other` covers any
/// S3-compatible service.
pub const S3_PROVIDERS: &[&str] = &[
    "AWS",
    "Ceph",
    "Cloudflare",
    "DigitalOcean",
    "Hetzner",
    "IDrive",
    "IONOS",
    "Minio",
    "OVHcloud",
    "Rclone",
    "Scaleway",
    "SeaweedFS",
    "Synology",
    "Wasabi",
    "Other",
];

/// WebDAV server types that rclone handles specially.
pub const WEBDAV_VENDORS: &[&str] = &["nextcloud", "owncloud", "infinitescale", "rclone", "other"];

/// TLS mode of an FTP connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FtpTls {
    /// `AUTH TLS` on the normal port (FTPES), the recommended mode.
    Explicit,
    /// TLS from the first byte (FTPS, usually port 990).
    Implicit,
    /// Plain FTP. Needs `allow_insecure`.
    None,
}

/// Protocol and connection settings of a target, without secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TargetConfig {
    /// A directory in the container, below `TALKOPS_BACKUP_TARGET_ROOTS`
    /// (USB disk, NFS or SMB volume mounted by the host).
    Local,
    /// SMB 2/3 share, without a kernel mount.
    Smb {
        host: String,
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        user: String,
        #[serde(default)]
        domain: String,
    },
    /// S3 bucket at AWS or an S3-compatible service.
    S3 {
        provider: String,
        /// Required for all providers except AWS.
        #[serde(default)]
        endpoint: String,
        #[serde(default)]
        region: String,
        access_key_id: String,
        /// Path-style URLs (`endpoint/bucket`), needed by most self-hosted services.
        #[serde(default)]
        path_style: bool,
    },
    Ftp {
        host: String,
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        user: String,
        tls: FtpTls,
    },
    Webdav {
        /// Base URL, e.g. `https://cloud.example.com/remote.php/dav/files/USER`.
        url: String,
        vendor: String,
        #[serde(default)]
        user: String,
    },
    Sftp {
        host: String,
        #[serde(default)]
        port: Option<u16>,
        user: String,
        /// Pinned host keys, `algorithm base64-key`, comma-separated.
        #[serde(default)]
        host_keys: String,
    },
}

impl TargetConfig {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Smb { .. } => "smb",
            Self::S3 { .. } => "s3",
            Self::Ftp { .. } => "ftp",
            Self::Webdav { .. } => "webdav",
            Self::Sftp { .. } => "sftp",
        }
    }

    /// The archive leaves this host.
    pub fn is_remote(&self) -> bool {
        !matches!(self, Self::Local)
    }
}

/// Passwords and keys of a target. Never returned by the API.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TargetSecret {
    /// SMB, FTP, WebDAV and SFTP password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// S3 secret access key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
    /// SFTP private key in PEM format (OpenSSH, unencrypted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
}

impl std::fmt::Debug for TargetSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TargetSecret(..)")
    }
}

impl TargetSecret {
    fn fields(&mut self) -> [&mut Option<String>; 3] {
        [
            &mut self.password,
            &mut self.secret_access_key,
            &mut self.private_key,
        ]
    }

    pub fn is_empty(&self) -> bool {
        self.password.is_none() && self.secret_access_key.is_none() && self.private_key.is_none()
    }

    /// Applies a change: a missing field keeps the stored value, an empty
    /// field removes it.
    pub fn merge(mut self, change: Option<&TargetSecret>) -> TargetSecret {
        if let Some(change) = change {
            let mut change = change.clone();
            for (stored, new) in self.fields().into_iter().zip(change.fields()) {
                if let Some(value) = new.take() {
                    *stored = Some(value).filter(|v| !v.is_empty());
                }
            }
        }
        self
    }
}

/// When a target receives a new archive (local time of the container).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Schedule {
    /// Bit 0 = Monday ... bit 6 = Sunday. 0 = only manual runs.
    pub weekdays: i16,
    /// Time of the first run of the day.
    pub hour: i16,
    pub minute: i16,
    /// 0 = once a day; otherwise repeat every N hours until midnight.
    pub every_hours: i16,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            weekdays: 127,
            hour: 3,
            minute: 30,
            every_hours: 0,
        }
    }
}

impl Schedule {
    fn validate(&self) -> CoreResult<()> {
        if !(0..=127).contains(&self.weekdays)
            || !(0..=23).contains(&self.hour)
            || !(0..=59).contains(&self.minute)
            || !(0..=23).contains(&self.every_hours)
        {
            return Err(CoreError::Validation("invalid schedule".into()));
        }
        Ok(())
    }

    fn runs_on(&self, day: NaiveDate) -> bool {
        self.weekdays & (1 << day.weekday().num_days_from_monday()) != 0
    }

    /// Run times of one day, in order.
    pub fn times(&self) -> Vec<NaiveTime> {
        let first = u32::from(self.hour.clamp(0, 23) as u16) * 60
            + u32::from(self.minute.clamp(0, 59) as u16);
        let step = u32::from(self.every_hours.clamp(0, 23) as u16) * 60;
        let mut out = Vec::new();
        let mut at = first;
        while at < 24 * 60 {
            out.push(NaiveTime::from_hms_opt(at / 60, at % 60, 0).expect("valid time"));
            if step == 0 {
                break;
            }
            at += step;
        }
        out
    }

    /// The latest planned run at or before `now`, on the same day.
    pub fn latest(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        if !self.runs_on(now.date()) {
            return None;
        }
        self.times()
            .into_iter()
            .rev()
            .find(|t| *t <= now.time())
            .map(|t| now.date().and_time(t))
    }

    /// The next planned run after `now`, within the next eight days.
    pub fn next(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        (0..8).find_map(|offset| {
            let day = now.date() + Duration::days(offset);
            if !self.runs_on(day) {
                return None;
            }
            self.times()
                .into_iter()
                .map(|t| day.and_time(t))
                .find(|at| *at > now)
        })
    }

    /// Whether a run is due: a planned time has passed since `since`
    /// (the last run, or the creation of the target).
    pub fn due(&self, since: NaiveDateTime, now: NaiveDateTime) -> bool {
        self.latest(now).is_some_and(|at| at > since)
    }
}

/// Which archives a target keeps (similar to the Synology "smart recycle").
/// An archive stays if one of the rules selects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Retention {
    /// Always keep the latest N archives (at least 1).
    pub keep_last: i16,
    /// Keep the latest archive of each day for N days.
    pub keep_daily: i16,
    /// Keep the latest archive of each week for N weeks.
    pub keep_weekly: i16,
    /// Keep the latest archive of each month for N months.
    pub keep_monthly: i16,
    /// Keep the latest archive of each year for N years.
    pub keep_yearly: i16,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            keep_last: 3,
            keep_daily: 7,
            keep_weekly: 4,
            keep_monthly: 12,
            keep_yearly: 0,
        }
    }
}

impl Retention {
    fn validate(&self) -> CoreResult<()> {
        if !(1..=1000).contains(&self.keep_last)
            || !(0..=3660).contains(&self.keep_daily)
            || !(0..=520).contains(&self.keep_weekly)
            || !(0..=1200).contains(&self.keep_monthly)
            || !(0..=100).contains(&self.keep_yearly)
        {
            return Err(CoreError::Validation("invalid retention".into()));
        }
        Ok(())
    }

    /// Indices of the archives to keep. `stamps` are the creation times of
    /// the archives (local time, from their names); `now` is the local time.
    pub fn keep(&self, stamps: &[NaiveDateTime], now: NaiveDateTime) -> HashSet<usize> {
        let mut order: Vec<usize> = (0..stamps.len()).collect();
        order.sort_by(|a, b| stamps[*b].cmp(&stamps[*a]));
        let mut kept: HashSet<usize> = order
            .iter()
            .take(self.keep_last.max(1) as usize)
            .copied()
            .collect();
        // Archives from the future (clock change) are never deleted.
        kept.extend(order.iter().filter(|i| stamps[**i] > now).copied());

        let today = now.date();
        let month_index = |d: NaiveDate| d.year() * 12 + d.month0() as i32;
        let mut bucket = |period: i16, key: &dyn Fn(NaiveDate) -> Option<i64>| {
            if period <= 0 {
                return;
            }
            let mut seen = HashSet::new();
            for i in &order {
                if let Some(k) = key(stamps[*i].date())
                    && seen.insert(k)
                {
                    kept.insert(*i);
                }
            }
        };
        let days = i64::from(self.keep_daily);
        bucket(self.keep_daily, &|d| {
            ((today - d).num_days() < days).then(|| d.num_days_from_ce().into())
        });
        let weeks = i64::from(self.keep_weekly);
        bucket(self.keep_weekly, &|d| {
            let monday =
                |d: NaiveDate| d - Duration::days(d.weekday().num_days_from_monday() as i64);
            let age = (monday(today) - monday(d)).num_days() / 7;
            (age < weeks).then(|| monday(d).num_days_from_ce().into())
        });
        let months = i32::from(self.keep_monthly);
        bucket(self.keep_monthly, &|d| {
            (month_index(today) - month_index(d) < months).then(|| month_index(d).into())
        });
        let years = i32::from(self.keep_yearly);
        bucket(self.keep_yearly, &|d| {
            (today.year() - d.year() < years).then(|| d.year().into())
        });
        kept
    }
}

/// Where and how a target connects. Used for saved targets, the folder
/// browser and restore before setup.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct TargetConnection {
    pub config: TargetConfig,
    /// Folder on the target. S3: `bucket/prefix`, SMB: `share/folder`,
    /// local: absolute path.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub allow_insecure: bool,
    #[serde(default)]
    pub secret: TargetSecret,
}

fn valid_text(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.chars().any(char::is_control)
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '[' | ']'))
}

fn valid_url(url: &str, allow_insecure: bool) -> CoreResult<()> {
    if !crate::transcription_api::valid_url(url) {
        return Err(CoreError::Validation(
            "the URL must start with https:// or http://".into(),
        ));
    }
    if url.starts_with("http://") && !allow_insecure {
        return Err(CoreError::Validation(
            "http:// sends data without encryption; allow insecure transport or use https://"
                .into(),
        ));
    }
    Ok(())
}

/// Removes surrounding slashes and checks the segments of a target path.
pub fn normalize_path(path: &str, absolute: bool) -> CoreResult<String> {
    let path = path.trim();
    if path.len() > 1024 || path.chars().any(|c| c.is_control() || c == '\\') {
        return Err(CoreError::Validation("invalid path".into()));
    }
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.iter().any(|s| *s == "." || *s == "..") {
        return Err(CoreError::Validation(
            "the path must not contain . or ..".into(),
        ));
    }
    if absolute {
        if !path.starts_with('/') {
            return Err(CoreError::Validation(
                "a local target needs an absolute path".into(),
            ));
        }
        Ok(format!("/{}", segments.join("/")))
    } else {
        Ok(segments.join("/"))
    }
}

impl TargetConnection {
    /// Checks the settings and normalizes the path. `for_browse` allows an
    /// incomplete path (no bucket or share yet) and an SFTP target without
    /// a pinned host key, so the browser can list buckets and the test can
    /// pin the key.
    pub fn validate(&mut self, for_browse: bool) -> CoreResult<()> {
        let insecure = self.allow_insecure;
        let check_port = |p: &Option<u16>| {
            if *p == Some(0) {
                Err(CoreError::Validation("invalid port".into()))
            } else {
                Ok(())
            }
        };
        let check_host = |h: &mut String| {
            *h = h.trim().to_owned();
            if valid_host(h) {
                Ok(())
            } else {
                Err(CoreError::Validation("invalid host name".into()))
            }
        };
        let check_user = |u: &mut String, required: bool| {
            *u = u.trim().to_owned();
            if !valid_text(u, 200) || (required && u.is_empty()) {
                Err(CoreError::Validation("invalid user name".into()))
            } else {
                Ok(())
            }
        };
        let needs_root_segment = match &mut self.config {
            TargetConfig::Local => {
                self.path = normalize_path(&self.path, true)?;
                false
            }
            TargetConfig::Smb {
                host,
                port,
                user,
                domain,
            } => {
                check_host(host)?;
                check_port(port)?;
                check_user(user, false)?;
                *domain = domain.trim().to_owned();
                if !valid_text(domain, 100) {
                    return Err(CoreError::Validation("invalid domain".into()));
                }
                true
            }
            TargetConfig::S3 {
                provider,
                endpoint,
                region,
                access_key_id,
                ..
            } => {
                if !S3_PROVIDERS.contains(&provider.as_str()) {
                    return Err(CoreError::Validation("unknown S3 provider".into()));
                }
                *endpoint = endpoint.trim().trim_end_matches('/').to_owned();
                if endpoint.is_empty() {
                    if provider != "AWS" {
                        return Err(CoreError::Validation(
                            "this S3 provider needs an endpoint URL".into(),
                        ));
                    }
                } else {
                    valid_url(endpoint, insecure)?;
                }
                *region = region.trim().to_owned();
                if region.len() > 64
                    || !region
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    return Err(CoreError::Validation("invalid region".into()));
                }
                *access_key_id = access_key_id.trim().to_owned();
                if access_key_id.is_empty()
                    || access_key_id.len() > 200
                    || access_key_id
                        .chars()
                        .any(|c| c.is_whitespace() || c.is_control())
                {
                    return Err(CoreError::Validation("invalid access key ID".into()));
                }
                if self.secret.secret_access_key.is_none() {
                    return Err(CoreError::Validation("enter the secret access key".into()));
                }
                true
            }
            TargetConfig::Ftp {
                host,
                port,
                user,
                tls,
            } => {
                check_host(host)?;
                check_port(port)?;
                check_user(user, false)?;
                if *tls == FtpTls::None && !insecure {
                    return Err(CoreError::Validation(
                        "plain FTP sends passwords without encryption; allow insecure transport or use TLS".into(),
                    ));
                }
                false
            }
            TargetConfig::Webdav { url, vendor, user } => {
                *url = url.trim().trim_end_matches('/').to_owned();
                valid_url(url, insecure)?;
                if !WEBDAV_VENDORS.contains(&vendor.as_str()) {
                    return Err(CoreError::Validation("unknown WebDAV server type".into()));
                }
                check_user(user, false)?;
                false
            }
            TargetConfig::Sftp {
                host,
                port,
                user,
                host_keys,
            } => {
                check_host(host)?;
                check_port(port)?;
                check_user(user, true)?;
                *host_keys = host_keys
                    .split(',')
                    .map(str::trim)
                    .filter(|k| !k.is_empty())
                    .collect::<Vec<_>>()
                    .join(",");
                let valid_key = |k: &str| {
                    let mut parts = k.split(' ');
                    matches!(
                        (parts.next(), parts.next(), parts.next()),
                        (Some(a), Some(b), None)
                            if !a.is_empty() && a.len() <= 64 && a.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '@' | '.'))
                            && !b.is_empty() && b.len() <= 4096 && b.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
                    )
                };
                if host_keys.split(',').filter(|k| !k.is_empty()).count() > 16
                    || !host_keys
                        .split(',')
                        .filter(|k| !k.is_empty())
                        .all(valid_key)
                {
                    return Err(CoreError::Validation("invalid host key".into()));
                }
                if host_keys.is_empty() && !for_browse {
                    return Err(CoreError::Validation(
                        "test the connection and accept the host key of the SFTP server".into(),
                    ));
                }
                if self.secret.password.is_none() && self.secret.private_key.is_none() {
                    return Err(CoreError::Validation(
                        "enter a password or a private key".into(),
                    ));
                }
                false
            }
        };
        if !matches!(self.config, TargetConfig::Local) {
            // SFTP paths without a leading slash start in the home folder.
            let rooted = matches!(self.config, TargetConfig::Sftp { .. })
                && self.path.trim().starts_with('/');
            self.path = normalize_path(&self.path, false)?;
            if rooted {
                self.path.insert(0, '/');
            }
            if needs_root_segment && self.path.is_empty() && !for_browse {
                return Err(CoreError::Validation(match self.config {
                    TargetConfig::S3 { .. } => "select a bucket".into(),
                    _ => "select a share".into(),
                }));
            }
        }
        for value in [
            &self.secret.password,
            &self.secret.secret_access_key,
            &self.secret.private_key,
        ]
        .into_iter()
        .flatten()
        {
            if value.len() > 16384 {
                return Err(CoreError::Validation("secret value too long".into()));
            }
        }
        Ok(())
    }
}

/// A target as shown in the UI (without secrets).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct BackupTarget {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub config: TargetConfig,
    pub path: String,
    pub allow_insecure: bool,
    pub include_recordings: bool,
    pub schedule: Schedule,
    pub retention: Retention,
    pub bwlimit_kbps: Option<i32>,
    /// A password or key is stored.
    pub has_secret: bool,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_file: Option<String>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct BackupTargetInput {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub config: TargetConfig,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub allow_insecure: bool,
    #[serde(default = "yes")]
    pub include_recordings: bool,
    #[serde(default)]
    pub schedule: Schedule,
    #[serde(default)]
    pub retention: Retention,
    #[serde(default)]
    pub bwlimit_kbps: Option<i32>,
    /// Changed secrets: a missing field keeps the stored value, `""` removes it.
    #[serde(default)]
    pub secret: Option<TargetSecret>,
}

fn yes() -> bool {
    true
}

#[derive(FromRow)]
struct Row {
    id: Uuid,
    name: String,
    enabled: bool,
    settings: Json<TargetConfig>,
    path: String,
    allow_insecure: bool,
    include_recordings: bool,
    weekdays: i16,
    hour: i16,
    minute: i16,
    every_hours: i16,
    keep_last: i16,
    keep_daily: i16,
    keep_weekly: i16,
    keep_monthly: i16,
    keep_yearly: i16,
    bwlimit_kbps: Option<i32>,
    has_secret: bool,
    last_run_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
    last_file: Option<String>,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<Row> for BackupTarget {
    fn from(r: Row) -> Self {
        Self {
            id: r.id,
            name: r.name,
            enabled: r.enabled,
            config: r.settings.0,
            path: r.path,
            allow_insecure: r.allow_insecure,
            include_recordings: r.include_recordings,
            schedule: Schedule {
                weekdays: r.weekdays,
                hour: r.hour,
                minute: r.minute,
                every_hours: r.every_hours,
            },
            retention: Retention {
                keep_last: r.keep_last,
                keep_daily: r.keep_daily,
                keep_weekly: r.keep_weekly,
                keep_monthly: r.keep_monthly,
                keep_yearly: r.keep_yearly,
            },
            bwlimit_kbps: r.bwlimit_kbps,
            has_secret: r.has_secret,
            last_run_at: r.last_run_at,
            last_success_at: r.last_success_at,
            last_file: r.last_file,
            last_error: r.last_error,
            created_at: r.created_at,
        }
    }
}

const COLUMNS: &str = "id, name, enabled, settings, path, allow_insecure, include_recordings,
    weekdays, hour, minute, every_hours, keep_last, keep_daily, keep_weekly, keep_monthly,
    keep_yearly, bwlimit_kbps, secret_enc IS NOT NULL AS has_secret, last_run_at,
    last_success_at, last_file, last_error, created_at";

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<BackupTarget>> {
    let sql = format!("SELECT {COLUMNS} FROM backup_targets WHERE tenant_id = $1 ORDER BY name");
    let rows: Vec<Row> = sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<BackupTarget> {
    let sql = format!("SELECT {COLUMNS} FROM backup_targets WHERE tenant_id = $1 AND id = $2");
    let row: Row = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(row.into())
}

/// The decrypted secrets of a target.
pub async fn secret(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<TargetSecret> {
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT secret_enc FROM backup_targets WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .fetch_one(pool)
    .await?;
    match stored {
        None => Ok(TargetSecret::default()),
        Some(enc) => serde_json::from_str(&secrets.decrypt(&enc)?)
            .map_err(|_| CoreError::Validation("stored target secret is invalid".into())),
    }
}

/// The saved connection of a target, with decrypted secrets.
pub async fn connection(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<(BackupTarget, TargetConnection)> {
    let target = get(pool, tenant, id).await?;
    let secret = secret(pool, tenant, secrets, id).await?;
    let conn = TargetConnection {
        config: target.config.clone(),
        path: target.path.clone(),
        allow_insecure: target.allow_insecure,
        secret,
    };
    Ok((target, conn))
}

/// Checks an input against the stored secret; returns the checked
/// connection (with the merged secret).
pub fn check(input: &BackupTargetInput, stored: TargetSecret) -> CoreResult<TargetConnection> {
    let name = input.name.trim();
    if name.is_empty() || !valid_text(name, 100) {
        return Err(CoreError::Validation(
            "name must have 1 to 100 characters".into(),
        ));
    }
    input.schedule.validate()?;
    input.retention.validate()?;
    if input
        .bwlimit_kbps
        .is_some_and(|b| !(1..=10_000_000).contains(&b))
    {
        return Err(CoreError::Validation("invalid bandwidth limit".into()));
    }
    let mut conn = TargetConnection {
        config: input.config.clone(),
        path: input.path.clone(),
        allow_insecure: input.allow_insecure,
        secret: stored.merge(input.secret.as_ref()),
    };
    conn.validate(false)?;
    Ok(conn)
}

fn encrypt_secret(secrets: &SecretBox, secret: &TargetSecret) -> CoreResult<Option<String>> {
    if secret.is_empty() {
        return Ok(None);
    }
    Ok(Some(secrets.encrypt(
        &serde_json::to_string(secret).expect("secret JSON"),
    )?))
}

pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    input: &BackupTargetInput,
) -> CoreResult<BackupTarget> {
    let conn = check(input, TargetSecret::default())?;
    write(pool, tenant, secrets, None, input, &conn).await
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
    input: &BackupTargetInput,
) -> CoreResult<BackupTarget> {
    let stored = secret(pool, tenant, secrets, id).await?;
    let conn = check(input, stored)?;
    write(pool, tenant, secrets, Some(id), input, &conn).await
}

async fn write(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Option<Uuid>,
    input: &BackupTargetInput,
    conn: &TargetConnection,
) -> CoreResult<BackupTarget> {
    let secret_enc = encrypt_secret(secrets, &conn.secret)?;
    let sql = match id {
        None => format!(
            "INSERT INTO backup_targets (tenant_id, id, name, enabled, kind, settings, path,
                 allow_insecure, include_recordings, weekdays, hour, minute, every_hours,
                 keep_last, keep_daily, keep_weekly, keep_monthly, keep_yearly, bwlimit_kbps,
                 secret_enc)
             VALUES ($1, COALESCE($2, gen_random_uuid()), $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                 $18, $19, $20)
             RETURNING {COLUMNS}"
        ),
        Some(_) => format!(
            "UPDATE backup_targets SET name = $3, enabled = $4, kind = $5, settings = $6,
                 path = $7, allow_insecure = $8, include_recordings = $9, weekdays = $10,
                 hour = $11, minute = $12, every_hours = $13, keep_last = $14,
                 keep_daily = $15, keep_weekly = $16, keep_monthly = $17, keep_yearly = $18,
                 bwlimit_kbps = $19, secret_enc = $20, updated_at = now()
             WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
        ),
    };
    let row: Row = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(input.name.trim())
        .bind(input.enabled)
        .bind(conn.config.kind())
        .bind(Json(&conn.config))
        .bind(&conn.path)
        .bind(conn.allow_insecure)
        .bind(input.include_recordings)
        .bind(input.schedule.weekdays)
        .bind(input.schedule.hour)
        .bind(input.schedule.minute)
        .bind(input.schedule.every_hours)
        .bind(input.retention.keep_last)
        .bind(input.retention.keep_daily)
        .bind(input.retention.keep_weekly)
        .bind(input.retention.keep_monthly)
        .bind(input.retention.keep_yearly)
        .bind(input.bwlimit_kbps)
        .bind(secret_enc)
        .fetch_one(pool)
        .await?;
    Ok(row.into())
}

pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let done = sqlx::query("DELETE FROM backup_targets WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Records the outcome of a run (`Ok(file)` or `Err(message)`).
pub async fn record_run(pool: &PgPool, id: Uuid, result: Result<&str, &str>) -> CoreResult<()> {
    let (file, error) = match result {
        Ok(f) => (Some(f), None),
        Err(e) => (None, Some(e.chars().take(1000).collect::<String>())),
    };
    let row: Option<(TenantId, String)> = sqlx::query_as(
        "UPDATE backup_targets SET last_run_at = now(),
             last_success_at = CASE WHEN $2::text IS NULL THEN last_success_at ELSE now() END,
             last_file = COALESCE($2, last_file), last_error = $3
         WHERE id = $1
         RETURNING tenant_id, name",
    )
    .bind(id)
    .bind(file)
    .bind(&error)
    .fetch_optional(pool)
    .await?;
    if let Some((tenant, name)) = row {
        let (event, params) = match &error {
            None => ("backup.target_ok", serde_json::json!({ "name": name })),
            Some(e) => (
                "backup.target_failed",
                serde_json::json!({ "name": name, "error": e }),
            ),
        };
        crate::notifications::emit_logged(pool, tenant, event, &id.to_string(), params).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn schedule_times_and_due() {
        let daily = Schedule::default();
        assert_eq!(daily.times().len(), 1);
        // 2026-10-15 is a Thursday.
        assert_eq!(daily.latest(at("2026-10-15 03:29")), None);
        assert_eq!(
            daily.latest(at("2026-10-15 09:00")),
            Some(at("2026-10-15 03:30"))
        );
        assert!(daily.due(at("2026-10-14 03:30"), at("2026-10-15 03:30")));
        assert!(!daily.due(at("2026-10-15 03:31"), at("2026-10-15 23:00")));
        assert_eq!(
            daily.next(at("2026-10-15 03:30")),
            Some(at("2026-10-16 03:30"))
        );

        let every6 = Schedule {
            weekdays: 0b001_1111, // Monday to Friday
            hour: 1,
            minute: 0,
            every_hours: 6,
        };
        let times: Vec<String> = every6
            .times()
            .iter()
            .map(|t| t.format("%H:%M").to_string())
            .collect();
        assert_eq!(times, ["01:00", "07:00", "13:00", "19:00"]);
        assert_eq!(
            every6.latest(at("2026-10-15 12:59")),
            Some(at("2026-10-15 07:00"))
        );
        assert!(every6.due(at("2026-10-15 07:00"), at("2026-10-15 13:00")));
        // Saturday: no run; the next run is on Monday.
        assert_eq!(every6.latest(at("2026-10-17 12:00")), None);
        assert_eq!(
            every6.next(at("2026-10-16 19:00")),
            Some(at("2026-10-19 01:00"))
        );

        let manual = Schedule {
            weekdays: 0,
            ..Schedule::default()
        };
        assert_eq!(manual.next(at("2026-10-15 00:00")), None);
        assert!(!manual.due(at("2020-01-01 00:00"), at("2026-10-15 12:00")));
    }

    #[test]
    fn smart_retention() {
        let now = at("2026-10-15 04:00");
        // One archive per day for 400 days.
        let stamps: Vec<NaiveDateTime> = (0..400)
            .map(|d| at("2026-10-15 03:30") - Duration::days(d))
            .collect();
        let rule = Retention {
            keep_last: 1,
            keep_daily: 7,
            keep_weekly: 4,
            keep_monthly: 12,
            keep_yearly: 0,
        };
        let kept = rule.keep(&stamps, now);
        let mut days: Vec<String> = kept
            .iter()
            .map(|i| stamps[*i].format("%Y-%m-%d").to_string())
            .collect();
        days.sort();
        days.reverse();
        // Seven days, then Sundays of the last four weeks, then month ends.
        assert_eq!(
            days,
            [
                "2026-10-15",
                "2026-10-14",
                "2026-10-13",
                "2026-10-12",
                "2026-10-11",
                "2026-10-10",
                "2026-10-09",
                "2026-10-04",
                "2026-09-30",
                "2026-09-27",
                "2026-08-31",
                "2026-07-31",
                "2026-06-30",
                "2026-05-31",
                "2026-04-30",
                "2026-03-31",
                "2026-02-28",
                "2026-01-31",
                "2025-12-31",
                "2025-11-30",
            ]
        );

        let count_only = Retention {
            keep_last: 3,
            keep_daily: 0,
            keep_weekly: 0,
            keep_monthly: 0,
            keep_yearly: 0,
        };
        assert_eq!(count_only.keep(&stamps, now).len(), 3);
        assert!(count_only.keep(&stamps, now).contains(&0));

        // Several archives per day: only the latest of the day stays.
        let hourly = vec![
            at("2026-10-15 01:00"),
            at("2026-10-15 03:00"),
            at("2026-10-14 23:00"),
            at("2026-10-14 01:00"),
            at("2026-10-16 00:00"), // future
        ];
        let rule = Retention {
            keep_last: 1,
            keep_daily: 2,
            ..count_only
        };
        let kept = rule.keep(&hourly, now);
        assert_eq!(kept, HashSet::from([1, 2, 4]));
    }

    #[test]
    fn secret_merge() {
        let stored = TargetSecret {
            password: Some("old".into()),
            secret_access_key: Some("key".into()),
            private_key: None,
        };
        let change = TargetSecret {
            password: Some("new".into()),
            secret_access_key: Some(String::new()),
            private_key: None,
        };
        let merged = stored.clone().merge(Some(&change));
        assert_eq!(merged.password.as_deref(), Some("new"));
        assert_eq!(merged.secret_access_key, None);
        assert_eq!(stored.clone().merge(None), stored);
        assert_eq!(format!("{stored:?}"), "TargetSecret(..)");
    }

    fn conn(config: TargetConfig, path: &str) -> TargetConnection {
        TargetConnection {
            config,
            path: path.into(),
            allow_insecure: false,
            secret: TargetSecret::default(),
        }
    }

    #[test]
    fn connection_validation() {
        let mut local = conn(TargetConfig::Local, "/var/lib/talkops/targets//usb/");
        local.validate(false).unwrap();
        assert_eq!(local.path, "/var/lib/talkops/targets/usb");
        assert!(
            conn(TargetConfig::Local, "relative")
                .validate(false)
                .is_err()
        );
        assert!(
            conn(TargetConfig::Local, "/var/lib/talkops/targets/../../etc")
                .validate(false)
                .is_err()
        );

        let smb = TargetConfig::Smb {
            host: " nas.local ".into(),
            port: None,
            user: "backup".into(),
            domain: String::new(),
        };
        let mut c = conn(smb.clone(), "/talkops/");
        c.validate(false).unwrap();
        assert_eq!(c.path, "talkops");
        assert!(conn(smb.clone(), "").validate(false).is_err());
        assert!(conn(smb, "").validate(true).is_ok());
        let bad_host = TargetConfig::Smb {
            host: "nas/../x".into(),
            port: None,
            user: String::new(),
            domain: String::new(),
        };
        assert!(conn(bad_host, "share").validate(false).is_err());

        let s3 = TargetConfig::S3 {
            provider: "Minio".into(),
            endpoint: "http://minio:9000".into(),
            region: String::new(),
            access_key_id: "AKIA".into(),
            path_style: true,
        };
        let mut c = conn(s3.clone(), "bucket/talkops");
        c.secret.secret_access_key = Some("s".into());
        assert!(
            c.clone().validate(false).is_err(),
            "http needs allow_insecure"
        );
        c.allow_insecure = true;
        c.validate(false).unwrap();
        assert!(conn(s3, "bucket").validate(false).is_err(), "secret key");

        let ftp = TargetConfig::Ftp {
            host: "ftp.example.com".into(),
            port: Some(21),
            user: "u".into(),
            tls: FtpTls::None,
        };
        assert!(conn(ftp.clone(), "").validate(false).is_err());
        let mut c = conn(ftp, "");
        c.allow_insecure = true;
        c.validate(false).unwrap();

        let dav = TargetConfig::Webdav {
            url: "https://cloud.example.com/remote.php/dav/files/u/".into(),
            vendor: "nextcloud".into(),
            user: "u".into(),
        };
        let mut c = conn(dav, "Backups");
        c.validate(false).unwrap();
        assert!(matches!(&c.config, TargetConfig::Webdav { url, .. } if !url.ends_with('/')));

        let sftp = TargetConfig::Sftp {
            host: "backup.example.com".into(),
            port: None,
            user: "u".into(),
            host_keys: String::new(),
        };
        let mut c = conn(sftp, "talkops");
        c.secret.password = Some("p".into());
        assert!(c.clone().validate(false).is_err(), "host key required");
        c.validate(true).unwrap();
        let pinned = TargetConfig::Sftp {
            host: "backup.example.com".into(),
            port: None,
            user: "u".into(),
            host_keys: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA==, ".into(),
        };
        let mut c = conn(pinned, "talkops");
        c.secret.password = Some("p".into());
        c.validate(false).unwrap();
        let injected = TargetConfig::Sftp {
            host: "backup.example.com".into(),
            port: None,
            user: "u".into(),
            host_keys: "ssh-ed25519 AAAA\nproxy_command=x".into(),
        };
        let mut c = conn(injected, "talkops");
        c.secret.password = Some("p".into());
        assert!(c.validate(true).is_err());
    }

    #[test]
    fn config_json_shape() {
        let json = serde_json::to_value(TargetConfig::Ftp {
            host: "h".into(),
            port: None,
            user: String::new(),
            tls: FtpTls::Explicit,
        })
        .unwrap();
        assert_eq!(json["kind"], "ftp");
        assert_eq!(json["tls"], "explicit");
        let local: TargetConfig = serde_json::from_str(r#"{"kind":"local"}"#).unwrap();
        assert_eq!(local, TargetConfig::Local);
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn store_targets_with_encrypted_secrets(pool: PgPool) {
        let secrets = SecretBox::from_hex(&"ab".repeat(32)).unwrap();
        let tenant = TenantId::DEFAULT;
        let input = BackupTargetInput {
            name: "NAS".into(),
            enabled: true,
            config: TargetConfig::Smb {
                host: "nas.local".into(),
                port: None,
                user: "backup".into(),
                domain: String::new(),
            },
            path: "talkops".into(),
            allow_insecure: false,
            include_recordings: false,
            schedule: Schedule::default(),
            retention: Retention::default(),
            bwlimit_kbps: Some(1000),
            secret: Some(TargetSecret {
                password: Some("smb-password".into()),
                ..TargetSecret::default()
            }),
        };
        let created = create(&pool, tenant, &secrets, &input).await.unwrap();
        assert!(created.has_secret);
        assert!(!created.include_recordings);
        let raw: String = sqlx::query_scalar("SELECT secret_enc FROM backup_targets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!raw.contains("smb-password"));
        assert!(
            serde_json::to_string(&created)
                .unwrap()
                .find("smb-password")
                .is_none()
        );

        // An update without a secret keeps the stored password.
        let mut change = input.clone();
        change.name = "NAS 2".into();
        change.secret = None;
        let updated = update(&pool, tenant, &secrets, created.id, &change)
            .await
            .unwrap();
        assert_eq!(updated.name, "NAS 2");
        let (_, conn) = connection(&pool, tenant, &secrets, created.id)
            .await
            .unwrap();
        assert_eq!(conn.secret.password.as_deref(), Some("smb-password"));

        // The name is unique per tenant.
        assert!(matches!(
            create(&pool, tenant, &secrets, &change).await,
            Err(CoreError::Conflict(_))
        ));

        record_run(&pool, created.id, Err("connection refused"))
            .await
            .unwrap();
        let failed = get(&pool, tenant, created.id).await.unwrap();
        assert_eq!(failed.last_error.as_deref(), Some("connection refused"));
        assert!(failed.last_success_at.is_none());
        record_run(&pool, created.id, Ok("talkops-20261015-033000.tar.gz.age"))
            .await
            .unwrap();
        let ok = get(&pool, tenant, created.id).await.unwrap();
        assert!(ok.last_error.is_none() && ok.last_success_at.is_some());
        assert_eq!(list(&pool, tenant).await.unwrap().len(), 1);
        delete(&pool, tenant, created.id).await.unwrap();
        assert!(matches!(
            delete(&pool, tenant, created.id).await,
            Err(CoreError::NotFound)
        ));
    }
}
