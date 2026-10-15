//! rclone as the transfer tool for backup targets (ADR 0029). Each call
//! starts one rclone process. The connection goes into environment
//! variables of that process only (`RCLONE_CONFIG_T_*`): passwords never
//! appear in arguments, in a process list or in a file on disk.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use talkops_core::backup_targets::{FtpTls, TargetConfig, TargetConnection};
use tokio::io::AsyncWriteExt;

/// Name of the remote in the environment variables.
const REMOTE: &str = "T";
/// Most entries one folder listing returns.
pub const MAX_ENTRIES: usize = 1000;
/// Time limit for listings and connection tests.
pub const SHORT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, thiserror::Error)]
pub enum RcloneError {
    #[error("not found on the target")]
    NotFound,
    #[error("the target did not answer in time")]
    Timeout,
    #[error("rclone is not available: {0}")]
    Missing(String),
    #[error("{0}")]
    Failed(String),
}

pub type Result<T, E = RcloneError> = std::result::Result<T, E>;

/// One entry of a folder listing.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Entry {
    #[serde(rename(deserialize = "Name"))]
    pub name: String,
    #[serde(rename(deserialize = "IsDir"))]
    pub is_dir: bool,
    #[serde(rename(deserialize = "Size"), default)]
    pub size: i64,
    #[serde(rename(deserialize = "ModTime"), default)]
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
}

/// Result of a connection test.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct TestReport {
    /// SFTP: host keys pinned on this first connection. Save them with the target.
    pub host_keys: Option<String>,
    /// SFTP: SHA-256 fingerprint of the pinned host key, for comparison.
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Rclone {
    bin: PathBuf,
}

impl Default for Rclone {
    fn default() -> Self {
        Self::new(PathBuf::from("rclone"))
    }
}

/// Removes the time stamp and level of an rclone log line.
fn clean_line(line: &str) -> &str {
    for level in ["CRITICAL: ", "ERROR : ", "ERROR: ", "NOTICE: ", "INFO  : "] {
        if let Some((_, rest)) = line.split_once(level) {
            return rest.trim();
        }
    }
    line.trim()
}

fn error_message(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .map(clean_line)
        .filter(|l| !l.is_empty())
        .collect();
    let msg = lines
        .iter()
        .rev()
        .find(|l| !l.starts_with("Attempt ") && !l.contains("There was nothing to transfer"))
        .or(lines.last())
        .copied()
        .unwrap_or("rclone failed");
    msg.chars().take(500).collect()
}

/// The remote path below the target folder, e.g. `T:bucket/talkops/name`.
pub fn remote_path(conn: &TargetConnection, sub: &str) -> String {
    let base = conn.path.trim_end_matches('/');
    let sub = sub.trim_matches('/');
    let path = match (base.is_empty(), sub.is_empty()) {
        (true, _) => sub.to_owned(),
        (false, true) => base.to_owned(),
        (false, false) => format!("{base}/{sub}"),
    };
    let path = if matches!(conn.config, TargetConfig::Local) && !path.starts_with('/') {
        format!("/{path}")
    } else {
        path
    };
    format!("{REMOTE}:{path}")
}

/// Environment variables that describe the connection. `obscured` is the
/// password in rclone's obscured form.
pub fn connection_env(conn: &TargetConnection, obscured: Option<&str>) -> Vec<(String, String)> {
    let mut env: Vec<(&str, String)> = Vec::new();
    let port = |p: &Option<u16>, env: &mut Vec<(&str, String)>| {
        if let Some(p) = p {
            env.push(("PORT", p.to_string()));
        }
    };
    let pass = |env: &mut Vec<(&str, String)>| {
        if let Some(p) = obscured {
            env.push(("PASS", p.to_owned()));
        }
    };
    match &conn.config {
        TargetConfig::Local => {
            env.push(("TYPE", "local".into()));
            // Never follow links out of the target folder.
            env.push(("SKIP_LINKS", "true".into()));
        }
        TargetConfig::Smb {
            host,
            port: p,
            user,
            domain,
        } => {
            env.push(("TYPE", "smb".into()));
            env.push(("HOST", host.clone()));
            port(p, &mut env);
            env.push(("USER", user.clone()));
            if !domain.is_empty() {
                env.push(("DOMAIN", domain.clone()));
            }
            pass(&mut env);
        }
        TargetConfig::S3 {
            provider,
            endpoint,
            region,
            access_key_id,
            path_style,
        } => {
            env.push(("TYPE", "s3".into()));
            env.push(("PROVIDER", provider.clone()));
            env.push(("ENV_AUTH", "false".into()));
            env.push(("ACCESS_KEY_ID", access_key_id.clone()));
            if let Some(key) = &conn.secret.secret_access_key {
                env.push(("SECRET_ACCESS_KEY", key.clone()));
            }
            if !region.is_empty() {
                env.push(("REGION", region.clone()));
            }
            if !endpoint.is_empty() {
                env.push(("ENDPOINT", endpoint.clone()));
            }
            env.push(("FORCE_PATH_STYLE", path_style.to_string()));
            // The bucket exists; do not need the permission to create it.
            env.push(("NO_CHECK_BUCKET", "true".into()));
        }
        TargetConfig::Ftp {
            host,
            port: p,
            user,
            tls,
        } => {
            env.push(("TYPE", "ftp".into()));
            env.push(("HOST", host.clone()));
            port(p, &mut env);
            env.push((
                "USER",
                if user.is_empty() {
                    "anonymous".into()
                } else {
                    user.clone()
                },
            ));
            match tls {
                FtpTls::Explicit => env.push(("EXPLICIT_TLS", "true".into())),
                FtpTls::Implicit => env.push(("TLS", "true".into())),
                FtpTls::None => {}
            }
            pass(&mut env);
        }
        TargetConfig::Webdav { url, vendor, user } => {
            env.push(("TYPE", "webdav".into()));
            env.push(("URL", url.clone()));
            env.push(("VENDOR", vendor.clone()));
            if !user.is_empty() {
                env.push(("USER", user.clone()));
            }
            pass(&mut env);
        }
        TargetConfig::Sftp {
            host,
            port: p,
            user,
            host_keys,
        } => {
            env.push(("TYPE", "sftp".into()));
            env.push(("HOST", host.clone()));
            port(p, &mut env);
            env.push(("USER", user.clone()));
            pass(&mut env);
            if let Some(key) = &conn.secret.private_key {
                // rclone expects the PEM on one line with literal \n.
                env.push((
                    "KEY_PEM",
                    key.trim().replace("\r\n", "\n").replace('\n', "\\n"),
                ));
            }
            if !host_keys.is_empty() {
                env.push(("HOST_KEYS", host_keys.clone()));
            }
            // Do not run shell commands on the server.
            env.push(("SHELL_TYPE", "none".into()));
        }
    }
    env.into_iter()
        .map(|(k, v)| (format!("RCLONE_CONFIG_{REMOTE}_{k}"), v))
        .collect()
}

/// Inherited variables: the search path, time zone, proxies and CA certificates.
const INHERITED: &[&str] = &[
    "PATH",
    "TZ",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

struct Call<'a> {
    conn: &'a TargetConnection,
    args: Vec<OsString>,
    stdin: Option<Vec<u8>>,
    timeout: Option<Duration>,
    config_file: Option<&'a Path>,
}

impl Rclone {
    pub fn new(bin: PathBuf) -> Self {
        Self { bin }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var_os("TALKOPS_RCLONE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("rclone")),
        )
    }

    fn command(&self, home: &Path) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.bin);
        cmd.env_clear();
        for name in INHERITED {
            if let Some(v) = std::env::var_os(name) {
                cmd.env(name, v);
            }
        }
        cmd.env("HOME", home)
            .env("XDG_CACHE_HOME", home)
            .env("XDG_CONFIG_HOME", home)
            .env("RCLONE_CACHE_DIR", home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        cmd
    }

    /// The password in rclone's obscured form (`rclone obscure -`).
    async fn obscure(&self, home: &Path, password: &str) -> Result<String> {
        let mut cmd = self.command(home);
        cmd.args(["obscure", "-"]).stdin(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| RcloneError::Missing(e.to_string()))?;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin
            .write_all(password.as_bytes())
            .await
            .map_err(|e| RcloneError::Failed(e.to_string()))?;
        drop(stdin);
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| RcloneError::Failed(e.to_string()))?;
        if !out.status.success() {
            return Err(RcloneError::Failed(error_message(&out.stderr)));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    async fn run(&self, call: Call<'_>) -> Result<Vec<u8>> {
        let home = tempfile::tempdir().map_err(|e| RcloneError::Failed(e.to_string()))?;
        let obscured = match &call.conn.secret.password {
            Some(p) if !matches!(call.conn.config, TargetConfig::S3 { .. }) => {
                Some(self.obscure(home.path(), p).await?)
            }
            _ => None,
        };
        let mut cmd = self.command(home.path());
        cmd.envs(connection_env(call.conn, obscured.as_deref()));
        cmd.arg("--config");
        match call.config_file {
            Some(file) => cmd.arg(file),
            None => cmd.arg(""),
        };
        cmd.args([
            "--contimeout",
            "15s",
            "--timeout",
            "5m",
            "--retries",
            "3",
            "--low-level-retries",
            "10",
            "--stats",
            "0",
        ]);
        cmd.args(&call.args);
        if call.stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| RcloneError::Missing(e.to_string()))?;
        if let Some(data) = call.stdin {
            let mut stdin = child.stdin.take().expect("stdin");
            stdin
                .write_all(&data)
                .await
                .map_err(|e| RcloneError::Failed(e.to_string()))?;
        }
        let out = match call.timeout {
            Some(limit) => tokio::time::timeout(limit, child.wait_with_output())
                .await
                .map_err(|_| RcloneError::Timeout)?,
            None => child.wait_with_output().await,
        }
        .map_err(|e| RcloneError::Failed(e.to_string()))?;
        drop(home);
        if out.status.success() {
            return Ok(out.stdout);
        }
        // Exit codes 3 and 4: directory or file not found.
        match out.status.code() {
            Some(3 | 4) => Err(RcloneError::NotFound),
            _ => Err(RcloneError::Failed(error_message(&out.stderr))),
        }
    }

    fn call<'a>(conn: &'a TargetConnection, args: &[&str]) -> Call<'a> {
        Call {
            conn,
            args: args.iter().map(OsString::from).collect(),
            stdin: None,
            timeout: Some(SHORT_TIMEOUT),
            config_file: None,
        }
    }

    /// Entries of a folder below the target path (one level, at most
    /// [`MAX_ENTRIES`]). Returns whether the list was cut.
    pub async fn list(&self, conn: &TargetConnection, sub: &str) -> Result<(Vec<Entry>, bool)> {
        let remote = remote_path(conn, sub);
        let out = self
            .run(Self::call(conn, &["lsjson", "--no-mimetype", &remote]))
            .await?;
        let mut entries: Vec<Entry> = serde_json::from_slice(&out)
            .map_err(|e| RcloneError::Failed(format!("unexpected listing: {e}")))?;
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        let cut = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);
        Ok((entries, cut))
    }

    /// Metadata of one file below the target path.
    pub async fn stat(&self, conn: &TargetConnection, name: &str) -> Result<Entry> {
        let remote = remote_path(conn, name);
        let out = self
            .run(Self::call(
                conn,
                &["lsjson", "--stat", "--no-mimetype", &remote],
            ))
            .await?;
        serde_json::from_slice(&out)
            .map_err(|e| RcloneError::Failed(format!("unexpected listing: {e}")))
    }

    pub async fn mkdir(&self, conn: &TargetConnection, sub: &str) -> Result<()> {
        let remote = remote_path(conn, sub);
        self.run(Self::call(conn, &["mkdir", &remote])).await?;
        Ok(())
    }

    pub async fn delete(&self, conn: &TargetConnection, name: &str) -> Result<()> {
        let remote = remote_path(conn, name);
        self.run(Self::call(conn, &["deletefile", &remote])).await?;
        Ok(())
    }

    /// Uploads a file as `name` into the target folder. Other protocols than
    /// S3 receive `name.partial` first, so an interrupted upload never has
    /// a valid archive name. Checks the size afterwards.
    pub async fn upload(
        &self,
        conn: &TargetConnection,
        local: &Path,
        name: &str,
        bwlimit_kbps: Option<i32>,
    ) -> Result<()> {
        let size = std::fs::metadata(local)
            .map_err(|e| RcloneError::Failed(e.to_string()))?
            .len();
        let atomic = matches!(conn.config, TargetConfig::S3 { .. });
        let first = if atomic {
            name.to_owned()
        } else {
            format!("{name}.partial")
        };
        let mut args: Vec<OsString> = vec!["copyto".into(), local.into()];
        args.push(remote_path(conn, &first).into());
        if let Some(limit) = bwlimit_kbps {
            args.push("--bwlimit".into());
            args.push(format!("{limit}K").into());
        }
        self.run(Call {
            conn,
            args,
            stdin: None,
            timeout: None,
            config_file: None,
        })
        .await?;
        if !atomic {
            let from = remote_path(conn, &first);
            let to = remote_path(conn, name);
            self.run(Call {
                timeout: Some(Duration::from_secs(1800)),
                ..Self::call(conn, &["moveto", &from, &to])
            })
            .await?;
        }
        let stat = self.stat(conn, name).await?;
        if stat.size >= 0 && stat.size as u64 != size {
            return Err(RcloneError::Failed(format!(
                "size check failed after upload ({} of {size} bytes)",
                stat.size
            )));
        }
        Ok(())
    }

    /// Downloads `name` from the target folder into `local` (a new file).
    pub async fn download(&self, conn: &TargetConnection, name: &str, local: &Path) -> Result<()> {
        let args: Vec<OsString> = vec![
            "copyto".into(),
            remote_path(conn, name).into(),
            local.into(),
        ];
        self.run(Call {
            conn,
            args,
            stdin: None,
            timeout: None,
            config_file: None,
        })
        .await?;
        Ok(())
    }

    /// SFTP: connects once, accepts the host key of the server (trust on
    /// first use) and returns it with its fingerprint.
    pub async fn pin_host_key(&self, conn: &TargetConnection) -> Result<(String, String)> {
        let dir = tempfile::tempdir().map_err(|e| RcloneError::Failed(e.to_string()))?;
        let file = dir.path().join("rclone.conf");
        std::fs::write(&file, "").map_err(|e| RcloneError::Failed(e.to_string()))?;
        let remote = remote_path(conn, "");
        let mut call = Self::call(conn, &["lsjson", "--sftp-pin-host-key", &remote]);
        call.config_file = Some(&file);
        // A missing folder is fine here; the key is pinned on connect.
        match self.run(call).await {
            Ok(_) | Err(RcloneError::NotFound) => {}
            Err(err) => return Err(err),
        }
        let pinned = std::fs::read_to_string(&file).unwrap_or_default();
        let keys = pinned
            .lines()
            .find_map(|l| l.strip_prefix("host_keys = "))
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| RcloneError::Failed("the server sent no host key".into()))?
            .to_owned();
        let print = fingerprint(&keys);
        Ok((keys, print))
    }

    /// Creates the target folder, writes, reads and deletes a test file.
    /// An SFTP target without a pinned host key pins it on this connection
    /// (trust on first use) and returns it.
    pub async fn test(&self, conn: &TargetConnection) -> Result<TestReport> {
        let mut report = TestReport::default();
        let mut conn = conn.clone();
        if matches!(&conn.config, TargetConfig::Sftp { host_keys, .. } if host_keys.is_empty()) {
            let (keys, print) = self.pin_host_key(&conn).await?;
            if let TargetConfig::Sftp { host_keys, .. } = &mut conn.config {
                host_keys.clone_from(&keys);
            }
            report.host_keys = Some(keys);
            report.fingerprint = Some(print);
        }
        if !matches!(conn.config, TargetConfig::S3 { .. }) && !conn.path.is_empty() {
            self.mkdir(&conn, "").await?;
        }
        let name = format!(".talkops-test-{}", uuid::Uuid::new_v4().simple());
        let data = format!("TalkOps write test {}\n", chrono::Utc::now().to_rfc3339());
        let remote = remote_path(&conn, &name);
        self.run(Call {
            stdin: Some(data.clone().into_bytes()),
            ..Self::call(&conn, &["rcat", &remote])
        })
        .await?;
        let read = self.run(Self::call(&conn, &["cat", &remote])).await;
        let deleted = self.delete(&conn, &name).await;
        if read? != data.as_bytes() {
            return Err(RcloneError::Failed(
                "the test file read back differs from the written file".into(),
            ));
        }
        deleted?;
        Ok(report)
    }
}

/// OpenSSH-style SHA-256 fingerprints (`SHA256:...`) of `algo base64` keys.
pub fn fingerprint(host_keys: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    host_keys
        .split(',')
        .filter_map(|k| {
            let (algo, key) = k.trim().split_once(' ')?;
            let raw = base64::engine::general_purpose::STANDARD
                .decode(key.trim())
                .ok()?;
            let hash = sha2::Sha256::digest(raw);
            Some(format!(
                "{algo} SHA256:{}",
                base64::engine::general_purpose::STANDARD_NO_PAD.encode(hash)
            ))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkops_core::backup_targets::TargetSecret;

    fn conn(config: TargetConfig, path: &str) -> TargetConnection {
        TargetConnection {
            config,
            path: path.into(),
            allow_insecure: false,
            secret: TargetSecret {
                password: Some("pw".into()),
                secret_access_key: Some("sk".into()),
                private_key: Some("-----BEGIN KEY-----\r\nabc\r\n-----END KEY-----\n".into()),
            },
        }
    }

    #[test]
    fn remote_paths() {
        assert_eq!(
            remote_path(&conn(TargetConfig::Local, "/mnt/usb"), "a.tar.gz"),
            "T:/mnt/usb/a.tar.gz"
        );
        let s3 = TargetConfig::S3 {
            provider: "AWS".into(),
            endpoint: String::new(),
            region: "eu-central-1".into(),
            access_key_id: "AK".into(),
            path_style: false,
        };
        assert_eq!(remote_path(&conn(s3.clone(), ""), ""), "T:");
        assert_eq!(remote_path(&conn(s3.clone(), ""), "bucket"), "T:bucket");
        assert_eq!(remote_path(&conn(s3, "bucket/x"), "/f/"), "T:bucket/x/f");
    }

    #[test]
    fn secrets_only_in_environment() {
        let s3 = conn(
            TargetConfig::S3 {
                provider: "Minio".into(),
                endpoint: "https://minio.local".into(),
                region: String::new(),
                access_key_id: "AK".into(),
                path_style: true,
            },
            "b",
        );
        let env = connection_env(&s3, None);
        let get = |env: &[(String, String)], k: &str| {
            env.iter()
                .find(|(n, _)| n == &format!("RCLONE_CONFIG_T_{k}"))
                .map(|(_, v)| v.clone())
        };
        assert_eq!(get(&env, "TYPE").as_deref(), Some("s3"));
        assert_eq!(get(&env, "SECRET_ACCESS_KEY").as_deref(), Some("sk"));
        assert_eq!(get(&env, "FORCE_PATH_STYLE").as_deref(), Some("true"));
        assert_eq!(get(&env, "PASS"), None);

        let ftp = conn(
            TargetConfig::Ftp {
                host: "ftp.local".into(),
                port: Some(2121),
                user: String::new(),
                tls: FtpTls::Explicit,
            },
            "",
        );
        let env = connection_env(&ftp, Some("obscured"));
        assert_eq!(get(&env, "PASS").as_deref(), Some("obscured"));
        assert_eq!(get(&env, "USER").as_deref(), Some("anonymous"));
        assert_eq!(get(&env, "EXPLICIT_TLS").as_deref(), Some("true"));
        assert_eq!(get(&env, "PORT").as_deref(), Some("2121"));

        let sftp = conn(
            TargetConfig::Sftp {
                host: "h".into(),
                port: None,
                user: "u".into(),
                host_keys: "ssh-ed25519 AAAA".into(),
            },
            "",
        );
        let env = connection_env(&sftp, Some("o"));
        assert_eq!(
            get(&env, "KEY_PEM").as_deref(),
            Some("-----BEGIN KEY-----\\nabc\\n-----END KEY-----")
        );
        assert_eq!(get(&env, "SHELL_TYPE").as_deref(), Some("none"));
        assert_eq!(get(&env, "HOST_KEYS").as_deref(), Some("ssh-ed25519 AAAA"));
    }

    #[test]
    fn error_lines() {
        let stderr = b"2026/10/15 19:51:00 ERROR : Attempt 1/3 failed with 1 errors\n\
            2026/10/15 19:51:00 CRITICAL: Failed to create file system: login denied\n";
        assert_eq!(
            error_message(stderr),
            "Failed to create file system: login denied"
        );
        assert_eq!(error_message(b""), "rclone failed");
        assert_eq!(
            fingerprint("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA=="),
            format!("ssh-ed25519 SHA256:{}", {
                use base64::Engine;
                use sha2::Digest;
                let raw = base64::engine::general_purpose::STANDARD
                    .decode("AAAAC3NzaC1lZDI1NTE5AAAAIA==")
                    .unwrap();
                base64::engine::general_purpose::STANDARD_NO_PAD.encode(sha2::Sha256::digest(raw))
            })
        );
    }
}
