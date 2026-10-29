//! Model library and installation by an administrator (TalkOps UI, ADR 0051).
//!
//! TalkOps does not ship model weights whose license does not permit
//! redistribution. An administrator uploads compiled models that they are
//! allowed to use into a library and activates one model per slot:
//!
//! ```text
//! <dir>/library/<id>/      uploaded model with install.json (id: digest prefix)
//! <dir>/de/                copy of the active German model (loaded at start)
//! <dir>/en/, <dir>/langid/ the same for English and language detection
//! ```
//!
//! The service checks every upload and activation with the same code that
//! loads the slots at startup. The loaded catalog does not change until the
//! service restarts.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use talkops_stt_service::library::{
    ID_LEN, dir_bytes, invalid, model_id, now, read_json, remove_dir, sha256, staged, statement,
};
pub use talkops_stt_service::library::{InstallError, Usage};

use crate::models::{Catalog, check_slot};

/// Model slots that the library can fill.
pub const SLOTS: [&str; 3] = ["de", "en", "langid"];
/// Largest accepted model file. Compiled CTC models are about 10–40 MB.
pub const MAX_MODEL_BYTES: usize = 64 * 1024 * 1024;
/// Largest accepted `model.json`.
pub const MAX_META_BYTES: usize = 256 * 1024;
/// Library limits, so uploads cannot fill the volume.
pub const MAX_LIBRARY_MODELS: usize = 10;
pub const MAX_LIBRARY_BYTES: u64 = 512 * 1024 * 1024;
/// File names of a model directory.
pub const TPU_FILE: &str = "model_edgetpu.tflite";
pub const CPU_FILE: &str = "model.tflite";
const META_FILE: &str = "model.json";
const INFO_FILE: &str = "install.json";
const LIBRARY_DIR: &str = "library";
/// The custom operation that `edgetpu_compiler` writes into a model.
const EDGETPU_OP: &[u8] = b"edgetpu-custom-op";

/// The files and statements of one upload.
#[derive(Debug, Default)]
pub struct Upload {
    pub meta: Vec<u8>,
    pub tpu: Vec<u8>,
    pub cpu: Option<Vec<u8>>,
    /// Slot the model is for: `de`, `en` or `langid`.
    pub slot: String,
    /// Display name in the library.
    pub name: String,
    /// License of the weights, as stated by the administrator.
    pub license: String,
    /// Origin of the weights (URL or description).
    pub source: String,
}

/// Written next to each library model and copied into an active slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallInfo {
    pub name: String,
    pub slot: String,
    /// Empty for a model that was copied into a slot by hand.
    pub license: String,
    pub source: String,
    pub installed_at_unix: u64,
    pub tpu_sha256: String,
    #[serde(default)]
    pub cpu_sha256: Option<String>,
}

impl InstallInfo {
    pub fn id(&self) -> String {
        self.tpu_sha256.chars().take(ID_LEN).collect()
    }
}

/// One library model.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryModel {
    pub id: String,
    pub info: InstallInfo,
    /// `candidate` and `license` from `model.json`.
    pub candidate: Option<String>,
    pub meta_license: Option<String>,
    pub bytes: u64,
    /// The slot that has this model, if any.
    pub selected: bool,
}

/// One slot as the admin interface shows it.
#[derive(Debug, Clone, Serialize)]
pub struct SlotStatus {
    pub slot: &'static str,
    pub installed: bool,
    /// Library id of the selected model.
    pub model_id: Option<String>,
    pub name: Option<String>,
    pub tpu_sha256: Option<String>,
    /// The running service uses these files.
    pub active: bool,
    /// The running service has a model for this slot (maybe another one).
    pub loaded: bool,
}

/// Slots and library for the admin interface.
#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub slots: Vec<SlotStatus>,
    pub library: Vec<LibraryModel>,
    pub usage: Usage,
}

fn sha256_file(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| sha256(&b))
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

fn library(dir: &Path) -> PathBuf {
    dir.join(LIBRARY_DIR)
}

/// Library ids on disk, sorted. Staging directories start with a dot.
fn library_ids(dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(library(dir))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| model_id(name).is_ok())
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

/// Copies the files of a model directory. `cpu` is optional.
fn copy_model(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir(to)?;
    for file in [META_FILE, TPU_FILE, CPU_FILE, INFO_FILE] {
        let source = from.join(file);
        if source.is_file() {
            std::fs::copy(&source, to.join(file))?;
        }
    }
    Ok(())
}

/// Adds an upload to the library. Invalid uploads change nothing.
pub fn add(dir: &Path, upload: Upload) -> Result<LibraryModel, InstallError> {
    let slot = slot(&upload.slot)?;
    let name = statement(&upload.name, "name")?;
    let license = statement(&upload.license, "license")?;
    let source = statement(&upload.source, "source")?;
    check_files(&upload)?;
    let info = InstallInfo {
        name,
        slot: slot.to_owned(),
        license,
        source,
        installed_at_unix: now(),
        tpu_sha256: sha256(&upload.tpu),
        cpu_sha256: upload.cpu.as_deref().map(sha256),
    };
    let id = info.id();
    let target = library(dir).join(&id);
    if target.exists() {
        return Err(invalid("this model is already in the library"));
    }
    let ids = library_ids(dir);
    let size =
        (upload.meta.len() + upload.tpu.len() + upload.cpu.as_ref().map_or(0, Vec::len)) as u64;
    let used: u64 = ids.iter().map(|i| dir_bytes(&library(dir).join(i))).sum();
    if ids.len() >= MAX_LIBRARY_MODELS || used + size > MAX_LIBRARY_BYTES {
        return Err(invalid(
            "the model library is full; delete a model that no slot uses",
        ));
    }
    std::fs::create_dir_all(library(dir))?;
    staged(
        &library(dir).join(format!(".incoming-{id}")),
        &target,
        |staging| {
            std::fs::create_dir(staging)?;
            std::fs::write(staging.join(META_FILE), &upload.meta)?;
            std::fs::write(staging.join(TPU_FILE), &upload.tpu)?;
            if let Some(cpu) = &upload.cpu {
                std::fs::write(staging.join(CPU_FILE), cpu)?;
            }
            // Use the startup code: a model that passes here cannot stop the
            // service from starting.
            check_slot(staging, slot, TPU_FILE).map_err(|e| invalid(e.to_string()))?;
            if upload.cpu.is_some() {
                check_slot(staging, slot, CPU_FILE).map_err(|e| invalid(e.to_string()))?;
            }
            let json = serde_json::to_vec_pretty(&info).map_err(|e| invalid(e.to_string()))?;
            std::fs::write(staging.join(INFO_FILE), json)?;
            Ok(())
        },
    )?;
    library_model(dir, &id).ok_or_else(|| invalid("model not found after upload"))
}

/// The library id of the model in `slot`. A model that was copied into the
/// slot by hand gets an entry with an unknown license first, so selecting
/// another model does not lose it.
fn adopt(dir: &Path, slot: &'static str) -> Result<Option<String>, InstallError> {
    let path = dir.join(slot);
    let Some(tpu_sha256) = sha256_file(&path.join(TPU_FILE)) else {
        return Ok(None);
    };
    let info: InstallInfo = match read_json(&path.join(INFO_FILE)) {
        Some(info) => info,
        None => InstallInfo {
            name: format!("{slot} (copied by hand)"),
            slot: slot.to_owned(),
            license: String::new(),
            source: String::new(),
            installed_at_unix: now(),
            cpu_sha256: sha256_file(&path.join(CPU_FILE)),
            tpu_sha256,
        },
    };
    let id = info.id();
    let target = library(dir).join(&id);
    if !target.exists() {
        std::fs::create_dir_all(library(dir))?;
        staged(
            &library(dir).join(format!(".adopt-{id}")),
            &target,
            |staging| {
                copy_model(&path, staging)?;
                let json = serde_json::to_vec_pretty(&info).map_err(|e| invalid(e.to_string()))?;
                std::fs::write(staging.join(INFO_FILE), json)?;
                Ok(())
            },
        )?;
    }
    Ok(Some(id))
}

/// Selects a library model for its slot. The previous model stays in the
/// library.
pub fn select(dir: &Path, slot_name: &str, id: &str) -> Result<SlotStatus, InstallError> {
    let slot = slot(slot_name)?;
    let id = model_id(id)?;
    let source = library(dir).join(id);
    let info: InstallInfo = read_json(&source.join(INFO_FILE))
        .ok_or_else(|| invalid("model not found in the library"))?;
    if info.slot != slot {
        return Err(invalid(format!(
            "this model is for the slot {}, not {slot}",
            info.slot
        )));
    }
    adopt(dir, slot)?;
    staged(
        &dir.join(format!(".incoming-{slot}")),
        &dir.join(slot),
        |staging| {
            copy_model(&source, staging)?;
            check_slot(staging, slot, TPU_FILE).map_err(|e| invalid(e.to_string()))?;
            Ok(())
        },
    )?;
    Ok(slot_status(dir, slot, None))
}

/// Empties a slot. Its model stays in the library.
pub fn clear(dir: &Path, slot_name: &str) -> Result<SlotStatus, InstallError> {
    let slot = slot(slot_name)?;
    if !dir.join(slot).is_dir() {
        return Err(invalid("no model is selected for this slot"));
    }
    adopt(dir, slot)?;
    remove_dir(&dir.join(slot))?;
    Ok(slot_status(dir, slot, None))
}

/// Deletes a library model that no slot uses.
pub fn delete(dir: &Path, id: &str) -> Result<(), InstallError> {
    let id = model_id(id)?;
    let path = library(dir).join(id);
    if !path.is_dir() {
        return Err(invalid("model not found in the library"));
    }
    if SLOTS
        .into_iter()
        .any(|s| slot_model_id(dir, s).as_deref() == Some(id))
    {
        return Err(invalid(
            "a slot uses this model; select another model or empty the slot first",
        ));
    }
    remove_dir(&path)?;
    Ok(())
}

fn slot_model_id(dir: &Path, slot: &str) -> Option<String> {
    let path = dir.join(slot);
    read_json::<InstallInfo>(&path.join(INFO_FILE))
        .map(|i| i.id())
        .or_else(|| sha256_file(&path.join(TPU_FILE)).map(|s| s.chars().take(ID_LEN).collect()))
}

fn library_model(dir: &Path, id: &str) -> Option<LibraryModel> {
    let path = library(dir).join(id);
    let info: InstallInfo = read_json(&path.join(INFO_FILE))?;
    let meta: Option<serde_json::Value> = read_json(&path.join(META_FILE));
    let text = |key: &str| {
        meta.as_ref()
            .and_then(|m| m[key].as_str())
            .map(str::to_owned)
    };
    Some(LibraryModel {
        id: id.to_owned(),
        selected: slot_model_id(dir, &info.slot).as_deref() == Some(id),
        candidate: text("candidate"),
        meta_license: text("license"),
        bytes: dir_bytes(&path),
        info,
    })
}

/// The state of one slot on disk, compared with the loaded catalog.
pub fn slot_status(dir: &Path, slot: &'static str, catalog: Option<&Catalog>) -> SlotStatus {
    let path = dir.join(slot);
    let tpu_sha256 = sha256_file(&path.join(TPU_FILE));
    let cpu_sha256 = sha256_file(&path.join(CPU_FILE));
    let info: Option<InstallInfo> = read_json(&path.join(INFO_FILE));
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
        model_id: slot_model_id(dir, slot),
        name: info.map(|i| i.name),
        tpu_sha256,
        active,
        loaded: loaded.is_some(),
    }
}

/// Slots in [`SLOTS`] order and all library models.
pub fn overview(dir: &Path, catalog: &Catalog) -> Overview {
    let library: Vec<LibraryModel> = library_ids(dir)
        .iter()
        .filter_map(|id| library_model(dir, id))
        .collect();
    Overview {
        slots: SLOTS
            .into_iter()
            .map(|s| slot_status(dir, s, Some(catalog)))
            .collect(),
        usage: Usage {
            models: library.len(),
            bytes: library.iter().map(|m| m.bytes).sum(),
            max_models: MAX_LIBRARY_MODELS,
            max_bytes: MAX_LIBRARY_BYTES,
        },
        library,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tflite(edgetpu: bool, tag: &[u8]) -> Vec<u8> {
        let mut bytes = b"\0\0\0\0TFL3model".to_vec();
        bytes.extend_from_slice(tag);
        if edgetpu {
            bytes.extend_from_slice(EDGETPU_OP);
        }
        bytes
    }

    fn upload(slot: &str, tag: &[u8], labels: serde_json::Value) -> Upload {
        Upload {
            meta:
                serde_json::json!({"labels": labels, "window_frames": 256, "license": "CC-BY-4.0"})
                    .to_string()
                    .into_bytes(),
            tpu: tflite(true, tag),
            cpu: Some(tflite(false, tag)),
            slot: slot.into(),
            name: "Citrinet DE".into(),
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
    fn library_selects_switches_and_deletes() {
        let dir = temp("library");
        let a = add(&dir, upload("de", b"a", serde_json::json!([" ", "a"]))).unwrap();
        let b = add(&dir, upload("de", b"b", serde_json::json!([" ", "b"]))).unwrap();
        assert_ne!(a.id, b.id);
        assert!(!a.selected);
        assert!(
            add(&dir, upload("de", b"a", serde_json::json!([" "]))).is_err(),
            "duplicate"
        );

        select(&dir, "de", &a.id).unwrap();
        let catalog = Catalog::load(&dir, TPU_FILE).unwrap();
        assert_eq!(catalog.ids(), ["coral-ctc-de", "coral-auto"]);
        let view = overview(&dir, &catalog);
        assert!(view.slots[0].active);
        assert_eq!(view.slots[0].model_id.as_deref(), Some(a.id.as_str()));
        assert_eq!(view.usage.models, 2);

        select(&dir, "de", &b.id).unwrap();
        let view = overview(&dir, &catalog);
        assert!(!view.slots[0].active, "loaded after a restart");
        assert!(view.library.iter().any(|m| m.id == b.id && m.selected));
        assert!(delete(&dir, &b.id).is_err(), "selected models stay");
        assert!(select(&dir, "en", &a.id).is_err(), "wrong slot");
        delete(&dir, &a.id).unwrap();

        clear(&dir, "de").unwrap();
        assert!(Catalog::load(&dir, TPU_FILE).unwrap().ids().is_empty());
        delete(&dir, &b.id).unwrap();
        assert!(overview(&dir, &catalog).library.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn adopts_models_copied_by_hand() {
        let dir = temp("adopt");
        let hand = dir.join("en");
        std::fs::create_dir_all(&hand).unwrap();
        std::fs::write(
            hand.join(META_FILE),
            serde_json::json!({"labels": [" ", "a"], "window_frames": 256}).to_string(),
        )
        .unwrap();
        std::fs::write(hand.join(TPU_FILE), tflite(true, b"hand")).unwrap();
        let new = add(&dir, upload("en", b"new", serde_json::json!([" ", "a"]))).unwrap();
        select(&dir, "en", &new.id).unwrap();
        let view = overview(&dir, &Catalog::default());
        let kept = view.library.iter().find(|m| m.id != new.id).unwrap();
        assert!(kept.info.license.is_empty() && !kept.selected);
        select(&dir, "en", &kept.id).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_invalid_uploads_without_changes() {
        let dir = temp("invalid");
        let empty_labels = upload("en", b"x", serde_json::json!([]));
        assert!(add(&dir, empty_labels).is_err());
        let mut uncompiled = upload("en", b"x", serde_json::json!([" ", "a"]));
        uncompiled.tpu = tflite(false, b"x");
        assert!(add(&dir, uncompiled).is_err());
        let mut swapped = upload("en", b"x", serde_json::json!([" ", "a"]));
        swapped.cpu = Some(tflite(true, b"x"));
        assert!(add(&dir, swapped).is_err());
        let mut unlicensed = upload("en", b"x", serde_json::json!([" ", "a"]));
        unlicensed.license = " ".into();
        assert!(add(&dir, unlicensed).is_err());
        assert!(add(&dir, upload("../x", b"x", serde_json::json!([" "]))).is_err());
        assert!(select(&dir, "de", "../../etc").is_err());
        assert!(delete(&dir, "ABCDEF123456").is_err());
        assert!(overview(&dir, &Catalog::default()).library.is_empty());
        assert!(library_ids(&dir).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
