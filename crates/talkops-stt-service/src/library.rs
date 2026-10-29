//! Format-independent parts of the model library (ADR 0051, ADR 0052):
//! errors, model ids, license and source statements, staged writes and
//! usage numbers. Each service owns its model format and checks.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::http::ApiError;

/// Hex digits of the digest that form a model id.
pub const ID_LEN: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{0}")]
    Invalid(String),
    #[error("model directory: {0}")]
    Io(#[from] std::io::Error),
}

impl From<InstallError> for ApiError {
    fn from(err: InstallError) -> Self {
        match err {
            InstallError::Invalid(message) => Self::bad_request(message),
            InstallError::Io(_) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_directory",
                // Usually a read-only models volume.
                format!("{err}; check that the models volume is writable"),
            ),
        }
    }
}

pub fn invalid(message: impl Into<String>) -> InstallError {
    InstallError::Invalid(message.into())
}

/// Lower-case hex SHA-256.
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(bytes))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 of a file, read in parts (model files can have gigabytes).
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Checks a library id: [ID_LEN] lower-case hex digits.
pub fn model_id(id: &str) -> Result<&str, InstallError> {
    if id.len() == ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        Ok(id)
    } else {
        Err(invalid("invalid model id"))
    }
}

/// A license, source or name statement of the administrator: 1 to 200
/// characters, no control characters.
pub fn statement(value: &str, field: &str) -> Result<String, InstallError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 200 || value.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{field}: 1 to 200 characters are required"
        )));
    }
    Ok(value.to_owned())
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Removes a directory; a missing directory is not an error.
pub fn remove_dir(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Bytes of the files in a directory and its subdirectories.
pub fn dir_bytes(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let meta = e.metadata().ok()?;
                    Some(if meta.is_dir() {
                        dir_bytes(&e.path())
                    } else if meta.is_file() {
                        meta.len()
                    } else {
                        0
                    })
                })
                .sum()
        })
        .unwrap_or(0)
}

/// Writes into `staging`, runs `fill`, then renames it to `target`.
/// `staging` is removed when something fails.
pub fn staged(
    staging: &Path,
    target: &Path,
    fill: impl FnOnce(&Path) -> Result<(), InstallError>,
) -> Result<(), InstallError> {
    remove_dir(staging)?;
    let result = fill(staging).and_then(|()| {
        // Replace the target only after a complete, checked copy.
        let old = staging.with_extension("old");
        remove_dir(&old)?;
        if target.exists() {
            std::fs::rename(target, &old)?;
        }
        std::fs::rename(staging, target)?;
        remove_dir(&old)?;
        Ok(())
    });
    if result.is_err() {
        let _ = remove_dir(staging);
    }
    result
}

/// Library size and limits.
#[derive(Debug, Clone, Serialize)]
pub struct Usage {
    pub models: usize,
    pub bytes: u64,
    pub max_models: usize,
    pub max_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_and_ids_are_checked() {
        assert_eq!(statement("  CC-BY-4.0 ", "license").unwrap(), "CC-BY-4.0");
        assert!(statement("", "license").is_err());
        assert!(statement("a\nb", "license").is_err());
        assert!(statement(&"x".repeat(201), "license").is_err());
        assert!(model_id("0123456789ab").is_ok());
        assert!(model_id("0123456789AB").is_err());
        assert!(model_id("../etc/pass").is_err());
    }

    #[test]
    fn file_digest_matches_the_memory_digest() {
        let dir = std::env::temp_dir().join(format!("stt-library-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a"), b"abc").unwrap();
        std::fs::write(dir.join("sub/b"), b"defg").unwrap();
        assert_eq!(sha256_file(&dir.join("a")).unwrap(), sha256(b"abc"));
        assert_eq!(dir_bytes(&dir), 7);
        remove_dir(&dir).unwrap();
    }
}
