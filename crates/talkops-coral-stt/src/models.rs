//! Model files and the public model names.
//!
//! The models directory holds one subdirectory per model, as written by
//! `tools/coral/export_nemo.py`, `train_langid.py` and `compile_model.py`:
//!
//! ```text
//! <dir>/de/model_edgetpu.tflite   compiled for the Edge TPU
//! <dir>/de/model.tflite           the same model for the CPU backend
//! <dir>/de/model.json             labels, window, front end, license
//! <dir>/en/...
//! <dir>/langid/...                optional German/English classifier
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Read;

use crate::features::{Frontend, LogMel};

/// Languages with a CTC model, in this order.
pub const LANGUAGES: [&str; 2] = ["de", "en"];
/// The model that selects the language of each turn.
pub const AUTO: &str = "coral-auto";

/// Metadata written by the tools next to each model.
#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    /// CTC labels without the blank (the blank is the class after them).
    #[serde(default)]
    pub labels: Vec<String>,
    /// Language ID classes, e.g. `["de", "en"]`.
    #[serde(default)]
    pub classes: Vec<String>,
    pub window_frames: usize,
    #[serde(default)]
    pub overlap_frames: Option<usize>,
    #[serde(default = "default_subsampling")]
    pub subsampling: usize,
    /// `nct`, `ntc` or `nhwc`.
    #[serde(default = "default_layout")]
    pub input_layout: String,
    /// The output holds log probabilities (NeMo) instead of logits.
    #[serde(default = "default_true")]
    pub log_probs: bool,
    #[serde(default)]
    pub frontend: Frontend,
    #[serde(default)]
    pub candidate: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
}

fn default_subsampling() -> usize {
    2
}
fn default_layout() -> String {
    "nct".into()
}
fn default_true() -> bool {
    true
}

impl Meta {
    pub fn overlap(&self) -> usize {
        self.overlap_frames.unwrap_or(self.window_frames / 4)
    }

    /// CTC classes including the blank.
    pub fn classes_with_blank(&self) -> usize {
        self.labels.len() + 1
    }
}

/// One loaded model description.
#[derive(Clone)]
pub struct ModelFile {
    /// `coral-ctc-de`, `coral-ctc-en` or `langid`.
    pub name: String,
    /// The language of a CTC model.
    pub language: Option<String>,
    /// The file the backend loads.
    pub path: PathBuf,
    /// Digest of the loaded model bytes; mock models have no file.
    pub sha256: Option<String>,
    pub meta: Meta,
    pub mel: Arc<LogMel>,
}

impl ModelFile {
    /// Samples of one input window at the model rate.
    pub fn window_samples(&self) -> usize {
        (self.meta.window_frames - 1) * self.mel.fe.hop()
    }
}

impl std::fmt::Debug for ModelFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelFile")
            .field("name", &self.name)
            .field("path", &self.path)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub ctc: Vec<ModelFile>,
    pub langid: Option<ModelFile>,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
}

fn load_one(dir: &Path, name: &str, file: &str) -> Result<Option<ModelFile>, CatalogError> {
    let json = dir.join("model.json");
    let path = dir.join(file);
    if !json.is_file() || !path.is_file() {
        return Ok(None);
    }
    let invalid = |message: String| CatalogError::Invalid {
        path: json.display().to_string(),
        message,
    };
    let text = std::fs::read_to_string(&json).map_err(|e| invalid(e.to_string()))?;
    let meta: Meta = serde_json::from_str(&text).map_err(|e| invalid(e.to_string()))?;
    if meta.window_frames < 8 || meta.overlap() >= meta.window_frames || meta.subsampling == 0 {
        return Err(invalid("invalid window".into()));
    }
    let mut source = std::fs::File::open(&path).map_err(|e| invalid(e.to_string()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|e| invalid(e.to_string()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let sha256 = Some(
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    );
    let mel = Arc::new(LogMel::new(meta.frontend.clone()));
    Ok(Some(ModelFile {
        name: name.to_owned(),
        language: None,
        path,
        sha256,
        meta,
        mel,
    }))
}

impl Catalog {
    /// Loads the models of `dir`; `file` is `model_edgetpu.tflite` (TPU) or
    /// `model.tflite` (CPU). Missing models are skipped.
    pub fn load(dir: &Path, file: &str) -> Result<Self, CatalogError> {
        let mut catalog = Self::default();
        for lang in LANGUAGES {
            if let Some(mut m) = load_one(&dir.join(lang), &format!("coral-ctc-{lang}"), file)? {
                if m.meta.labels.is_empty() {
                    return Err(CatalogError::Invalid {
                        path: m.path.display().to_string(),
                        message: "model.json has no labels".into(),
                    });
                }
                m.language = Some(lang.to_owned());
                catalog.ctc.push(m);
            }
        }
        catalog.langid = load_one(&dir.join("langid"), "langid", file)?;
        Ok(catalog)
    }

    pub fn ctc(&self, language: &str) -> Option<&ModelFile> {
        self.ctc
            .iter()
            .find(|m| m.language.as_deref() == Some(language))
    }

    pub fn languages(&self) -> Vec<String> {
        self.ctc.iter().filter_map(|m| m.language.clone()).collect()
    }

    /// Public model names: one per language and `coral-auto`.
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.ctc.iter().map(|m| m.name.clone()).collect();
        if !ids.is_empty() {
            ids.push(AUTO.into());
        }
        ids
    }

    /// All files a device loads.
    pub fn files(&self) -> Vec<&ModelFile> {
        self.ctc.iter().chain(self.langid.as_ref()).collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes a model directory with metadata and an empty model file.
    pub fn write_model(dir: &Path, sub: &str, meta: serde_json::Value) {
        let d = dir.join(sub);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("model.json"), meta.to_string()).unwrap();
        std::fs::write(d.join("model_edgetpu.tflite"), b"tflite").unwrap();
    }

    #[test]
    fn loads_models_and_names() {
        let dir = std::env::temp_dir().join(format!("coral-models-{}", std::process::id()));
        let ctc = serde_json::json!({"labels": [" ", "a"], "window_frames": 256});
        write_model(&dir, "de", ctc.clone());
        write_model(&dir, "en", ctc);
        write_model(
            &dir,
            "langid",
            serde_json::json!({"classes": ["de", "en"], "window_frames": 256, "input_layout": "nhwc"}),
        );
        let c = Catalog::load(&dir, "model_edgetpu.tflite").unwrap();
        assert_eq!(c.ids(), ["coral-ctc-de", "coral-ctc-en", "coral-auto"]);
        assert_eq!(c.languages(), ["de", "en"]);
        assert!(c.langid.is_some());
        assert_eq!(c.ctc("en").unwrap().meta.overlap(), 64);
        assert_eq!(c.ctc("de").unwrap().window_samples(), 255 * 160);
        // The CPU file is missing: nothing is loaded.
        assert!(
            Catalog::load(&dir, "model.tflite")
                .unwrap()
                .ids()
                .is_empty()
        );
        write_model(
            &dir,
            "en",
            serde_json::json!({"labels": [], "window_frames": 256}),
        );
        assert!(Catalog::load(&dir, "model_edgetpu.tflite").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
