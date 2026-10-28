//! Model installation by an administrator (upload in the TalkOps UI).
//!
//! TalkOps does not ship model weights whose license does not permit
//! redistribution. An administrator can install a compiled model that
//! they are allowed to use. The service checks the files with the same code
//! that loads them at startup, then replaces the slot directory:
//!
//! ```text
//! <dir>/.incoming-de/   new files while they are checked
//! <dir>/de/             the installed model (loaded at the next start)
//! <dir>/.backup/de/     the previous model (one level, for a restore)
//! ```
//!
//! The loaded catalog does not change until the service restarts.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::models::{Catalog, check_slot};

/// Model slots that an upload can replace.
pub const SLOTS: [&str; 3] = ["de", "en", "langid"];
/// Largest accepted model file. Compiled CTC models are about 10–40 MB.
pub const MAX_MODEL_BYTES: usize = 64 * 1024 * 1024;
/// Largest accepted `model.json`.
pub const MAX_META_BYTES: usize = 256 * 1024;
/// File names of a model directory.
pub const TPU_FILE: &str = "model_edgetpu.tflite";
pub const CPU_FILE: &str = "model.tflite";
const META_FILE: &str = "model.json";
const INFO_FILE: &str = "install.json";
const BACKUP_DIR: &str = ".backup";
/// The custom operation that `edgetpu_compiler` writes into a model.
const EDGETPU_OP: &[u8] = b"edgetpu-custom-op";

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{0}")]
    Invalid(String),
    #[error("model directory: {0}")]
    Io(#[from] std::io::Error),
}

fn invalid(message: impl Into<String>) -> InstallError {
    InstallError::Invalid(message.into())
}

/// The files and statements of one upload.
#[derive(Debug, Default)]
pub struct Upload {
    pub meta: Vec<u8>,
    pub tpu: Vec<u8>,
    pub cpu: Option<Vec<u8>>,
    /// License of the weights, as stated by the administrator.
    pub license: String,
    /// Origin of the weights (URL or description).
    pub source: String,
}

/// Written next to an uploaded model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallInfo {
    pub license: String,
    pub source: String,
    pub installed_at_unix: u64,
    pub tpu_sha256: String,
    #[serde(default)]
    pub cpu_sha256: Option<String>,
}

/// One slot as the admin interface shows it.
#[derive(Debug, Clone, Serialize)]
pub struct SlotStatus {
    pub slot: &'static str,
    pub installed: bool,
    /// `candidate` and `license` from `model.json`.
    pub candidate: Option<String>,
    pub meta_license: Option<String>,
    /// From `install.json`; missing for models that were copied by hand.
    pub info: Option<InstallInfo>,
    pub tpu_sha256: Option<String>,
    /// The running service uses these files.
    pub active: bool,
    pub backup: bool,
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn slot(name: &str) -> Result<&'static str, InstallError> {
    SLOTS
        .into_iter()
        .find(|s| *s == name)
        .ok_or_else(|| invalid("unknown model slot; use de, en or langid"))
}

/// A TensorFlow Lite flatbuffer has the identifier `TFL3` at offset 4.
fn is_tflite(bytes: &[u8]) -> bool {
    bytes.get(4..8) == Some(b"TFL3".as_slice())
}

fn has_edgetpu_op(bytes: &[u8]) -> bool {
    bytes.windows(EDGETPU_OP.len()).any(|w| w == EDGETPU_OP)
}

fn statement(value: &str, field: &str) -> Result<String, InstallError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 200 || value.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{field}: 1 to 200 characters are required"
        )));
    }
    Ok(value.to_owned())
}

fn check_files(upload: &Upload) -> Result<(), InstallError> {
    if upload.meta.is_empty() || upload.meta.len() > MAX_META_BYTES {
        return Err(invalid("model.json is missing or too large"));
    }
    if upload.tpu.len() > MAX_MODEL_BYTES
        || upload
            .cpu
            .as_ref()
            .is_some_and(|c| c.len() > MAX_MODEL_BYTES)
    {
        return Err(invalid("a model file is larger than 64 MiB"));
    }
    if !is_tflite(&upload.tpu) || !has_edgetpu_op(&upload.tpu) {
        return Err(invalid(
            "the Edge TPU file is not a model compiled by edgetpu_compiler",
        ));
    }
    if let Some(cpu) = &upload.cpu
        && (!is_tflite(cpu) || has_edgetpu_op(cpu))
    {
        return Err(invalid(
            "the CPU file must be the uncompiled TensorFlow Lite model",
        ));
    }
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn remove_dir(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn backup_path(dir: &Path, slot: &str) -> PathBuf {
    dir.join(BACKUP_DIR).join(slot)
}

/// Moves the installed slot (if any) to the backup, replacing the old backup.
fn move_to_backup(dir: &Path, slot: &str) -> std::io::Result<()> {
    let current = dir.join(slot);
    if !current.exists() {
        return Ok(());
    }
    let backup = backup_path(dir, slot);
    remove_dir(&backup)?;
    std::fs::create_dir_all(dir.join(BACKUP_DIR))?;
    std::fs::rename(current, backup)
}

/// Checks and installs an upload into `slot`. The previous model becomes
/// the backup. Invalid uploads change nothing.
pub fn install(dir: &Path, name: &str, upload: Upload) -> Result<SlotStatus, InstallError> {
    let slot = slot(name)?;
    let license = statement(&upload.license, "license")?;
    let source = statement(&upload.source, "source")?;
    check_files(&upload)?;
    std::fs::create_dir_all(dir)?;
    let staging = dir.join(format!(".incoming-{slot}"));
    remove_dir(&staging)?;
    std::fs::create_dir(&staging)?;
    let result = (|| -> Result<(), InstallError> {
        std::fs::write(staging.join(META_FILE), &upload.meta)?;
        std::fs::write(staging.join(TPU_FILE), &upload.tpu)?;
        if let Some(cpu) = &upload.cpu {
            std::fs::write(staging.join(CPU_FILE), cpu)?;
        }
        // Use the startup code: a model that passes here cannot stop the
        // service from starting.
        check_slot(&staging, slot, TPU_FILE).map_err(|e| invalid(e.to_string()))?;
        if upload.cpu.is_some() {
            check_slot(&staging, slot, CPU_FILE).map_err(|e| invalid(e.to_string()))?;
        }
        let info = InstallInfo {
            license,
            source,
            installed_at_unix: now(),
            tpu_sha256: sha256(&upload.tpu),
            cpu_sha256: upload.cpu.as_deref().map(sha256),
        };
        let json = serde_json::to_vec_pretty(&info).map_err(|e| invalid(e.to_string()))?;
        std::fs::write(staging.join(INFO_FILE), json)?;
        move_to_backup(dir, slot)?;
        std::fs::rename(&staging, dir.join(slot))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_dir(&staging);
    }
    result?;
    Ok(status(dir, slot, None))
}

/// Swaps the installed model and its backup.
pub fn restore(dir: &Path, name: &str) -> Result<SlotStatus, InstallError> {
    let slot = slot(name)?;
    let backup = backup_path(dir, slot);
    if !backup.is_dir() {
        return Err(invalid("no previous model for this slot"));
    }
    let swap = dir.join(format!(".restore-{slot}"));
    remove_dir(&swap)?;
    std::fs::rename(&backup, &swap)?;
    move_to_backup(dir, slot)?;
    std::fs::rename(&swap, dir.join(slot))?;
    Ok(status(dir, slot, None))
}

/// Removes the installed model of `slot`; it becomes the backup.
pub fn remove(dir: &Path, name: &str) -> Result<SlotStatus, InstallError> {
    let slot = slot(name)?;
    if !dir.join(slot).is_dir() {
        return Err(invalid("no model is installed in this slot"));
    }
    move_to_backup(dir, slot)?;
    Ok(status(dir, slot, None))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// The state of one slot on disk, compared with the loaded catalog.
pub fn status(dir: &Path, slot: &'static str, catalog: Option<&Catalog>) -> SlotStatus {
    let path = dir.join(slot);
    let meta: Option<serde_json::Value> = read_json(&path.join(META_FILE));
    let text = |key: &str| {
        meta.as_ref()
            .and_then(|m| m[key].as_str())
            .map(str::to_owned)
    };
    let tpu_sha256 = std::fs::read(path.join(TPU_FILE)).ok().map(|b| sha256(&b));
    let cpu_sha256 = std::fs::read(path.join(CPU_FILE)).ok().map(|b| sha256(&b));
    let loaded = catalog.and_then(|c| {
        if slot == "langid" {
            c.langid.as_ref()
        } else {
            c.ctc(slot)
        }
    });
    let active = loaded
        .is_some_and(|m| m.sha256.is_some() && (m.sha256 == tpu_sha256 || m.sha256 == cpu_sha256));
    SlotStatus {
        slot,
        installed: path.join(META_FILE).is_file() && tpu_sha256.is_some(),
        candidate: text("candidate"),
        meta_license: text("license"),
        info: read_json(&path.join(INFO_FILE)),
        tpu_sha256,
        active,
        backup: backup_path(dir, slot).is_dir(),
    }
}

/// All slots, in [`SLOTS`] order.
pub fn list(dir: &Path, catalog: &Catalog) -> Vec<SlotStatus> {
    SLOTS
        .into_iter()
        .map(|s| status(dir, s, Some(catalog)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tflite(edgetpu: bool) -> Vec<u8> {
        let mut bytes = b"\0\0\0\0TFL3model".to_vec();
        if edgetpu {
            bytes.extend_from_slice(EDGETPU_OP);
        }
        bytes
    }

    fn upload(labels: serde_json::Value) -> Upload {
        Upload {
            meta:
                serde_json::json!({"labels": labels, "window_frames": 256, "license": "CC-BY-4.0"})
                    .to_string()
                    .into_bytes(),
            tpu: tflite(true),
            cpu: Some(tflite(false)),
            license: "CC-BY-4.0".into(),
            source: "https://example.org/model".into(),
        }
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("coral-install-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn installs_backs_up_and_restores() {
        let dir = temp("cycle");
        let first = install(&dir, "de", upload(serde_json::json!([" ", "a"]))).unwrap();
        assert!(first.installed && !first.backup);
        assert_eq!(first.meta_license.as_deref(), Some("CC-BY-4.0"));
        let catalog = Catalog::load(&dir, TPU_FILE).unwrap();
        assert_eq!(catalog.ids(), ["coral-ctc-de", "coral-auto"]);
        assert!(list(&dir, &catalog)[0].active);

        let mut second = upload(serde_json::json!([" ", "a", "b"]));
        second.tpu.extend_from_slice(b"v2");
        let second = install(&dir, "de", second).unwrap();
        assert!(second.backup);
        assert!(
            !list(&dir, &catalog)[0].active,
            "not loaded before a restart"
        );

        let restored = restore(&dir, "de").unwrap();
        assert_eq!(restored.tpu_sha256, first.tpu_sha256);
        assert!(restored.backup);
        let removed = remove(&dir, "de").unwrap();
        assert!(!removed.installed && removed.backup);
        assert!(Catalog::load(&dir, TPU_FILE).unwrap().ids().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_invalid_uploads_without_changes() {
        let dir = temp("invalid");
        install(&dir, "en", upload(serde_json::json!([" ", "a"]))).unwrap();
        let before = status(&dir, "en", None).tpu_sha256;

        let empty_labels = upload(serde_json::json!([]));
        assert!(install(&dir, "en", empty_labels).is_err());
        let mut uncompiled = upload(serde_json::json!([" ", "a"]));
        uncompiled.tpu = tflite(false);
        assert!(install(&dir, "en", uncompiled).is_err());
        let mut swapped = upload(serde_json::json!([" ", "a"]));
        swapped.cpu = Some(tflite(true));
        assert!(install(&dir, "en", swapped).is_err());
        let mut unlicensed = upload(serde_json::json!([" ", "a"]));
        unlicensed.license = " ".into();
        assert!(install(&dir, "en", unlicensed).is_err());
        assert!(install(&dir, "../x", upload(serde_json::json!([" "]))).is_err());

        let after = status(&dir, "en", None);
        assert_eq!(after.tpu_sha256, before);
        assert!(!after.backup);
        assert!(!dir.join(".incoming-en").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
