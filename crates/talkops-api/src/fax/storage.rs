//! Fax storage checks. Never fall back to temporary or unshared document storage.
use std::{io::Write, path::Path};

use serde::Serialize;

const MIN_FREE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageStatus {
    Ready,
    Unavailable,
    Full,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum StorageError {
    #[error(
        "Fax storage is not writable. Ask an administrator to check the shared fax volume and its permissions."
    )]
    Unavailable,
    #[error("Fax storage has insufficient free space. At least 100 MiB is required.")]
    Full,
}

impl StorageError {
    pub fn status(self) -> StorageStatus {
        match self {
            Self::Unavailable => StorageStatus::Unavailable,
            Self::Full => StorageStatus::Full,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "fax_storage_unavailable",
            Self::Full => "fax_storage_full",
        }
    }
}

/// Keep paths and operating-system details in server logs, not the API response.
pub fn io_error(path: &Path, operation: &str, error: &std::io::Error) -> StorageError {
    tracing::warn!(path=%path.display(), operation, error=%error, "fax storage access failed");
    if error.kind() == std::io::ErrorKind::StorageFull {
        StorageError::Full
    } else {
        StorageError::Unavailable
    }
}

fn check_directory(root: &Path, minimum: u64) -> Result<(), StorageError> {
    std::fs::create_dir_all(root).map_err(|e| io_error(root, "create directory", &e))?;
    // Check actual access as the service user, including file creation and removal.
    // The unique temporary file cannot overwrite a document or another probe.
    let mut probe = tempfile::Builder::new()
        .prefix(".fax-storage-")
        .tempfile_in(root)
        .map_err(|e| io_error(root, "create probe", &e))?;
    probe
        .write_all(b"TalkOps fax storage check\n")
        .and_then(|()| probe.as_file().sync_data())
        .map_err(|e| io_error(root, "write probe", &e))?;
    probe
        .close()
        .map_err(|e| io_error(root, "remove probe", &e))?;
    let available = fs2::available_space(root).map_err(|e| io_error(root, "free space", &e))?;
    if available < minimum {
        tracing::warn!(path=%root.display(), available, minimum, "insufficient fax storage");
        return Err(StorageError::Full);
    }
    Ok(())
}

pub async fn check(root: &Path) -> Result<(), StorageError> {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || check_directory(&root, MIN_FREE_BYTES))
        .await
        .map_err(|error| {
            tracing::warn!(%error, "fax storage check failed");
            StorageError::Unavailable
        })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_preserves_documents_and_cleans_up_when_space_is_low() {
        use axum::response::IntoResponse;
        let root = tempfile::tempdir().unwrap();
        let document = root.path().join("document.tif");
        std::fs::write(&document, b"existing fax").unwrap();
        assert!(check_directory(root.path(), 0).is_ok());
        let error = check_directory(root.path(), u64::MAX).unwrap_err();
        assert!(matches!(error, StorageError::Full));
        assert_eq!(
            crate::error::ApiError::FaxStorage(error)
                .into_response()
                .status(),
            axum::http::StatusCode::INSUFFICIENT_STORAGE
        );
        assert_eq!(std::fs::read(&document).unwrap(), b"existing fax");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_storage_path_has_a_safe_error() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("private-path");
        std::fs::write(&file, b"preserve").unwrap();
        let error = check_directory(&file, 0).unwrap_err();
        assert_eq!(error.code(), "fax_storage_unavailable");
        assert!(!error.to_string().contains("private-path"));
        assert_eq!(std::fs::read(file).unwrap(), b"preserve");
    }
}
