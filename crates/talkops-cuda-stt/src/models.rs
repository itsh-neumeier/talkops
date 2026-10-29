//! Model manifests, the slot selection and the catalog that the service
//! loads at startup (ADR 0053).
//!
//! ```text
//! <dir>/library/<id>/model.json   manifest with license and source
//! <dir>/library/<id>/*.onnx       model files, tokens.txt
//! <dir>/selection.json            library id per slot
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Slots: one model per language, and the language ID model.
pub const SLOTS: [&str; 3] = ["de", "en", "langid"];
/// Slots with a speech recognition model.
pub const LANGUAGES: [&str; 2] = ["de", "en"];
pub const AUTO: &str = "cuda-auto";
pub const MANIFEST: &str = "model.json";
pub const SELECTION: &str = "selection.json";
pub const LIBRARY: &str = "library";

/// A model family of sherpa-onnx.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// NeMo CTC (`model.onnx`), e.g. the CTC head of FastConformer hybrid.
    NemoCtc,
    /// NeMo transducer and TDT (`encoder`, `decoder`, `joiner`), e.g. Parakeet.
    NemoTransducer,
    /// NeMo Canary (`encoder`, `decoder`).
    Canary,
    /// OpenAI Whisper (`encoder`, `decoder`); also the language ID model.
    Whisper,
}

impl Family {
    pub fn parse(value: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(value.trim().into())).ok()
    }

    /// The roles of the ONNX files.
    pub fn roles(self) -> &'static [&'static str] {
        match self {
            Self::NemoCtc => &["model"],
            Self::NemoTransducer => &["encoder", "decoder", "joiner"],
            Self::Canary | Self::Whisper => &["encoder", "decoder"],
        }
    }

    /// The model decodes all its languages without a language parameter,
    /// so one loaded instance can serve several slots.
    pub fn language_agnostic(self) -> bool {
        matches!(self, Self::NemoCtc | Self::NemoTransducer)
    }
}

/// `model.json` of a library model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub family: Family,
    /// ISO 639-1 codes that the administrator wants to use the model for.
    pub languages: Vec<String>,
    /// File name per role (`encoder`, `decoder`, `joiner`, `model`, `tokens`).
    pub files: BTreeMap<String, String>,
    /// License of the weights, as stated by the administrator or a preset.
    pub license: String,
    /// Origin of the weights (URL or description).
    pub source: String,
    pub installed_at_unix: u64,
    /// SHA-256 of the installed archive.
    #[serde(default)]
    pub archive_sha256: Option<String>,
    /// SHA-256 of each file, by file name.
    #[serde(default)]
    pub sha256: BTreeMap<String, String>,
}

impl Manifest {
    pub fn file(&self, dir: &Path, role: &str) -> Option<PathBuf> {
        self.files.get(role).map(|name| dir.join(name))
    }
}

/// Library id per slot.
pub type Selection = BTreeMap<String, String>;

pub fn read_selection(dir: &Path) -> Selection {
    talkops_stt_service::library::read_json(&dir.join(SELECTION)).unwrap_or_default()
}

/// One selected model.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    pub dir: PathBuf,
    pub manifest: Manifest,
}

/// The selected models at startup. A missing or broken model leaves its
/// slot empty and is reported, so the service still starts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    pub slots: BTreeMap<String, Entry>,
    /// Slot -> reason, for selected models that cannot be read.
    pub errors: BTreeMap<String, String>,
}

impl Catalog {
    pub fn load(dir: &Path) -> Self {
        let mut catalog = Self::default();
        for (slot, id) in read_selection(dir) {
            if !SLOTS.contains(&slot.as_str()) {
                continue;
            }
            let model_dir = dir.join(LIBRARY).join(&id);
            let manifest: Option<Manifest> =
                talkops_stt_service::library::read_json(&model_dir.join(MANIFEST));
            match manifest {
                Some(manifest) => {
                    let missing: Vec<&String> = manifest
                        .files
                        .values()
                        .filter(|f| !model_dir.join(f).is_file())
                        .collect();
                    if missing.is_empty() {
                        catalog.slots.insert(
                            slot,
                            Entry {
                                id,
                                dir: model_dir,
                                manifest,
                            },
                        );
                    } else {
                        catalog
                            .errors
                            .insert(slot, format!("model {id}: missing files {missing:?}"));
                    }
                }
                None => {
                    catalog.errors.insert(
                        slot,
                        format!("model {id}: model.json is missing or invalid"),
                    );
                }
            }
        }
        catalog
    }

    pub fn get(&self, slot: &str) -> Option<&Entry> {
        self.slots.get(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_parse_and_have_roles() {
        assert_eq!(
            Family::parse("nemo_transducer"),
            Some(Family::NemoTransducer)
        );
        assert_eq!(Family::parse(" whisper "), Some(Family::Whisper));
        assert_eq!(Family::parse("tflite"), None);
        assert_eq!(Family::NemoCtc.roles(), ["model"]);
        assert!(Family::NemoTransducer.language_agnostic());
        assert!(!Family::Canary.language_agnostic());
    }

    #[test]
    fn catalog_reports_broken_selections() {
        let dir = std::env::temp_dir().join(format!("cuda-models-{}", std::process::id()));
        let model = dir.join(LIBRARY).join("0123456789ab");
        std::fs::create_dir_all(&model).unwrap();
        let manifest = Manifest {
            name: "test".into(),
            family: Family::NemoCtc,
            languages: vec!["de".into()],
            files: [("model", "model.onnx"), ("tokens", "tokens.txt")]
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .into(),
            license: "CC-BY-4.0".into(),
            source: "test".into(),
            installed_at_unix: 0,
            archive_sha256: None,
            sha256: BTreeMap::new(),
        };
        std::fs::write(model.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
        std::fs::write(model.join("model.onnx"), b"x").unwrap();
        std::fs::write(
            dir.join(SELECTION),
            br#"{"de": "0123456789ab", "en": "ffffffffffff", "x": "0123456789ab"}"#,
        )
        .unwrap();
        let catalog = Catalog::load(&dir);
        assert!(catalog.slots.is_empty(), "tokens.txt is missing");
        assert!(catalog.errors["de"].contains("tokens.txt"));
        assert!(catalog.errors["en"].contains("model.json"));
        std::fs::write(model.join("tokens.txt"), b"a 0\n").unwrap();
        let catalog = Catalog::load(&dir);
        assert_eq!(catalog.get("de").unwrap().manifest, manifest);
        assert!(!catalog.errors.contains_key("x"));
        talkops_stt_service::library::remove_dir(&dir).unwrap();
    }
}
