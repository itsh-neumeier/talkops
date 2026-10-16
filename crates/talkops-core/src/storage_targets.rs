//! Storage targets (ADR 0036): local folders, SMB shares, S3 buckets, FTP,
//! WebDAV and SFTP servers. Several features use them: backup jobs
//! (`backup_targets`) and, later, other exports. A storage target holds the
//! connection only. Passwords and keys are stored encrypted (ADR 0008) and
//! never returned. The server transfers files with rclone
//! (`talkops_api::rclone`).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
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

pub(crate) fn valid_text(s: &str, max: usize) -> bool {
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

/// Joins a base folder and a sub-folder of a target path.
pub fn join_path(base: &str, folder: &str) -> String {
    match (base.trim_end_matches('/'), folder.trim_matches('/')) {
        (b, "") if b.is_empty() && base.starts_with('/') => "/".into(),
        (b, "") => b.to_owned(),
        ("", f) if base.starts_with('/') => format!("/{f}"),
        ("", f) => f.to_owned(),
        (b, f) => format!("{b}/{f}"),
    }
}

/// A feature that uses storage targets. New features add a value here and a
/// part to [`USAGE_SQL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StoragePurpose {
    /// A backup job (`backup_targets`).
    Backup,
}

impl StoragePurpose {
    pub fn key(self) -> &'static str {
        match self {
            Self::Backup => "backup",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "backup" => Some(Self::Backup),
            _ => None,
        }
    }
}

/// One user of a storage target, e.g. a backup job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct StorageUsage {
    pub purpose: StoragePurpose,
    /// ID of the job or setting that uses the target.
    pub id: Uuid,
    pub name: String,
}

/// All users of the storage targets of a tenant: `(storage_target_id,
/// purpose, id, name)`. Each feature adds one `UNION ALL` part.
const USAGE_SQL: &str = "SELECT storage_target_id, 'backup' AS purpose, id, name
    FROM backup_targets WHERE tenant_id = $1";

/// The users of each storage target of the tenant, by name.
pub async fn usages(
    pool: &PgPool,
    tenant: TenantId,
) -> CoreResult<HashMap<Uuid, Vec<StorageUsage>>> {
    let sql = format!("SELECT * FROM ({USAGE_SQL}) u ORDER BY purpose, name");
    let rows: Vec<(Uuid, String, Uuid, String)> =
        sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?;
    let mut out: HashMap<Uuid, Vec<StorageUsage>> = HashMap::new();
    for (target, purpose, id, name) in rows {
        if let Some(purpose) = StoragePurpose::from_key(&purpose) {
            out.entry(target)
                .or_default()
                .push(StorageUsage { purpose, id, name });
        }
    }
    Ok(out)
}

/// The users of one storage target.
pub async fn usage(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Vec<StorageUsage>> {
    Ok(usages(pool, tenant).await?.remove(&id).unwrap_or_default())
}

/// A storage target as shown in the UI (without secrets).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StorageTarget {
    pub id: Uuid,
    pub name: String,
    pub config: TargetConfig,
    /// Base folder. S3: `bucket/prefix`, SMB: `share/folder`, local:
    /// absolute path.
    pub path: String,
    pub allow_insecure: bool,
    /// Upload limit in KiB/s; `None` = no limit.
    pub bwlimit_kbps: Option<i32>,
    /// A password or key is stored.
    pub has_secret: bool,
    /// Last connection test of the saved target.
    pub last_test_at: Option<DateTime<Utc>>,
    pub last_test_ok: Option<bool>,
    pub last_test_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct StorageTargetInput {
    pub name: String,
    pub config: TargetConfig,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub allow_insecure: bool,
    #[serde(default)]
    pub bwlimit_kbps: Option<i32>,
    /// Changed secrets: a missing field keeps the stored value, `""` removes it.
    #[serde(default)]
    pub secret: Option<TargetSecret>,
}

#[derive(FromRow)]
struct Row {
    id: Uuid,
    name: String,
    settings: Json<TargetConfig>,
    path: String,
    allow_insecure: bool,
    bwlimit_kbps: Option<i32>,
    has_secret: bool,
    last_test_at: Option<DateTime<Utc>>,
    last_test_ok: Option<bool>,
    last_test_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<Row> for StorageTarget {
    fn from(r: Row) -> Self {
        Self {
            id: r.id,
            name: r.name,
            config: r.settings.0,
            path: r.path,
            allow_insecure: r.allow_insecure,
            bwlimit_kbps: r.bwlimit_kbps,
            has_secret: r.has_secret,
            last_test_at: r.last_test_at,
            last_test_ok: r.last_test_ok,
            last_test_error: r.last_test_error,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

const COLUMNS: &str = "id, name, settings, path, allow_insecure, bwlimit_kbps,
    secret_enc IS NOT NULL AS has_secret, last_test_at, last_test_ok, last_test_error,
    created_at, updated_at";

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<StorageTarget>> {
    let sql = format!("SELECT {COLUMNS} FROM storage_targets WHERE tenant_id = $1 ORDER BY name");
    let rows: Vec<Row> = sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<StorageTarget> {
    let sql = format!("SELECT {COLUMNS} FROM storage_targets WHERE tenant_id = $1 AND id = $2");
    let row: Row = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(row.into())
}

/// The decrypted secrets of a storage target.
pub async fn secret(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<TargetSecret> {
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT secret_enc FROM storage_targets WHERE tenant_id = $1 AND id = $2",
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

/// The saved connection of a storage target, with decrypted secrets.
pub async fn connection(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<(StorageTarget, TargetConnection)> {
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

pub(crate) fn check_name(name: &str) -> CoreResult<()> {
    let name = name.trim();
    if name.is_empty() || !valid_text(name, 100) {
        return Err(CoreError::Validation(
            "name must have 1 to 100 characters".into(),
        ));
    }
    Ok(())
}

/// Checks an input against the stored secret; returns the checked
/// connection (with the merged secret).
pub fn check(input: &StorageTargetInput, stored: TargetSecret) -> CoreResult<TargetConnection> {
    check_name(&input.name)?;
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
    input: &StorageTargetInput,
) -> CoreResult<StorageTarget> {
    let conn = check(input, TargetSecret::default())?;
    write(pool, tenant, secrets, None, input, &conn).await
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
    input: &StorageTargetInput,
) -> CoreResult<StorageTarget> {
    let (before, stored) = connection(pool, tenant, secrets, id).await?;
    let conn = check(input, stored.secret.clone())?;
    // A changed connection invalidates the last test result.
    let same = before.config == conn.config
        && before.path == conn.path
        && before.allow_insecure == conn.allow_insecure
        && stored.secret == conn.secret;
    write(pool, tenant, secrets, Some((id, same)), input, &conn).await
}

async fn write(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Option<(Uuid, bool)>,
    input: &StorageTargetInput,
    conn: &TargetConnection,
) -> CoreResult<StorageTarget> {
    let secret_enc = encrypt_secret(secrets, &conn.secret)?;
    let keep_test = id.is_some_and(|(_, same)| same);
    let sql = match id {
        None => format!(
            "INSERT INTO storage_targets (tenant_id, id, name, kind, settings, path,
                 allow_insecure, bwlimit_kbps, secret_enc)
             VALUES ($1, COALESCE($2, gen_random_uuid()), $3, $4, $5, $6, $7, $8, $9)
             RETURNING {COLUMNS}"
        ),
        Some(_) => format!(
            "UPDATE storage_targets SET name = $3, kind = $4, settings = $5, path = $6,
                 allow_insecure = $7, bwlimit_kbps = $8, secret_enc = $9,
                 last_test_at = CASE WHEN $10 THEN last_test_at END,
                 last_test_ok = CASE WHEN $10 THEN last_test_ok END,
                 last_test_error = CASE WHEN $10 THEN last_test_error END,
                 updated_at = now()
             WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
        ),
    };
    let mut query = sqlx::query_as::<_, Row>(&sql)
        .bind(tenant)
        .bind(id.map(|(id, _)| id))
        .bind(input.name.trim())
        .bind(conn.config.kind())
        .bind(Json(&conn.config))
        .bind(&conn.path)
        .bind(conn.allow_insecure)
        .bind(input.bwlimit_kbps)
        .bind(secret_enc);
    if id.is_some() {
        query = query.bind(keep_test);
    }
    Ok(query.fetch_one(pool).await?.into())
}

/// Deletes a storage target. A target in use cannot be deleted.
pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let users = usage(pool, tenant, id).await?;
    if !users.is_empty() {
        let names: Vec<String> = users
            .iter()
            .map(|u| format!("{} \"{}\"", u.purpose.key(), u.name))
            .collect();
        return Err(CoreError::Conflict(format!(
            "the storage target is in use: {}",
            names.join(", ")
        )));
    }
    let done = sqlx::query("DELETE FROM storage_targets WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|err| match &err {
            // A user was added at the same time.
            sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                CoreError::Conflict("the storage target is in use".into())
            }
            _ => err.into(),
        })?;
    if done.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Records the result of a connection test of a saved target.
pub async fn record_test(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    result: Result<(), &str>,
) -> CoreResult<()> {
    let error = result
        .err()
        .map(|e| e.chars().take(1000).collect::<String>());
    sqlx::query(
        "UPDATE storage_targets SET last_test_at = now(), last_test_ok = $3::text IS NULL,
             last_test_error = $3
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn join_paths() {
        assert_eq!(
            join_path("/var/lib/talkops/targets/usb", ""),
            "/var/lib/talkops/targets/usb"
        );
        assert_eq!(
            join_path("/var/lib/talkops/targets/usb", "daily"),
            "/var/lib/talkops/targets/usb/daily"
        );
        assert_eq!(join_path("bucket/talkops", "audit"), "bucket/talkops/audit");
        assert_eq!(join_path("", "audit"), "audit");
        assert_eq!(join_path("", ""), "");
        assert_eq!(join_path("/", "audit"), "/audit");
        assert_eq!(join_path("/", ""), "/");
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

    fn input(name: &str, path: &str) -> StorageTargetInput {
        StorageTargetInput {
            name: name.into(),
            config: TargetConfig::Smb {
                host: "nas.local".into(),
                port: None,
                user: "backup".into(),
                domain: String::new(),
            },
            path: path.into(),
            allow_insecure: false,
            bwlimit_kbps: Some(1000),
            secret: Some(TargetSecret {
                password: Some("smb-password".into()),
                ..TargetSecret::default()
            }),
        }
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn store_targets_with_encrypted_secrets(pool: PgPool) {
        let secrets = SecretBox::from_hex(&"ab".repeat(32)).unwrap();
        let tenant = TenantId::DEFAULT;
        let created = create(&pool, tenant, &secrets, &input("NAS", "talkops"))
            .await
            .unwrap();
        assert!(created.has_secret);
        assert_eq!(created.bwlimit_kbps, Some(1000));
        assert!(created.last_test_at.is_none());
        let raw: String = sqlx::query_scalar("SELECT secret_enc FROM storage_targets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!raw.contains("smb-password"));
        assert!(
            !serde_json::to_string(&created)
                .unwrap()
                .contains("smb-password")
        );

        // A test result stays until the connection changes.
        record_test(&pool, tenant, created.id, Err("connection refused"))
            .await
            .unwrap();
        let failed = get(&pool, tenant, created.id).await.unwrap();
        assert_eq!(failed.last_test_ok, Some(false));
        assert_eq!(
            failed.last_test_error.as_deref(),
            Some("connection refused")
        );
        record_test(&pool, tenant, created.id, Ok(()))
            .await
            .unwrap();
        let mut change = input("NAS 2", "talkops");
        change.secret = None;
        let renamed = update(&pool, tenant, &secrets, created.id, &change)
            .await
            .unwrap();
        assert_eq!(renamed.name, "NAS 2");
        assert_eq!(renamed.last_test_ok, Some(true), "same connection");
        let (_, conn) = connection(&pool, tenant, &secrets, created.id)
            .await
            .unwrap();
        assert_eq!(conn.secret.password.as_deref(), Some("smb-password"));
        change.path = "other".into();
        let moved = update(&pool, tenant, &secrets, created.id, &change)
            .await
            .unwrap();
        assert!(moved.last_test_ok.is_none(), "connection changed");

        // The name is unique per tenant.
        assert!(matches!(
            create(&pool, tenant, &secrets, &change).await,
            Err(CoreError::Conflict(_))
        ));

        // A target in use cannot be deleted.
        let job: Uuid = sqlx::query_scalar(
            "INSERT INTO backup_targets (tenant_id, name, storage_target_id)
             VALUES ($1, 'Nightly', $2) RETURNING id",
        )
        .bind(tenant)
        .bind(created.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let users = usage(&pool, tenant, created.id).await.unwrap();
        assert_eq!(
            users,
            [StorageUsage {
                purpose: StoragePurpose::Backup,
                id: job,
                name: "Nightly".into()
            }]
        );
        let err = delete(&pool, tenant, created.id).await.unwrap_err();
        assert!(
            matches!(&err, CoreError::Conflict(m) if m.contains("Nightly")),
            "{err}"
        );
        // The database rejects it too.
        assert!(
            sqlx::query("DELETE FROM storage_targets WHERE id = $1")
                .bind(created.id)
                .execute(&pool)
                .await
                .is_err()
        );
        sqlx::query("DELETE FROM backup_targets")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(list(&pool, tenant).await.unwrap().len(), 1);
        delete(&pool, tenant, created.id).await.unwrap();
        assert!(matches!(
            delete(&pool, tenant, created.id).await,
            Err(CoreError::NotFound)
        ));
    }
}
