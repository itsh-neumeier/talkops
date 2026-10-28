//! A deterministic backend without GPU, libraries or models, for CI and
//! UI tests.
//!
//! Each half second with sound gives one word: `hallo` (German slot) or
//! `hello` (English slot). Language ID says English for audio with many
//! zero crossings (high pitch) and German otherwise. An optional delay makes
//! the device slow, and a flag makes it fail. A library model whose name
//! starts with `broken` does not load.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use talkops_stt_service::Word;
use talkops_stt_service::audio::MODEL_RATE;

use crate::engine::{Backend, Hypothesis, LoadedModel, Output, Port, TaskKind};
use crate::gpu::Gpu;
use crate::models::{Catalog, Entry, Family, Manifest};

pub struct MockBackend {
    pub delay: Duration,
    /// When set, every inference fails (the device goes to `error`).
    pub fail: Arc<AtomicBool>,
}

impl MockBackend {
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            fail: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// German, English and language ID models that exist only in memory.
pub fn catalog() -> Catalog {
    let entry = |slot: &str, family| Entry {
        id: format!("{:0>12}", slot.len()),
        dir: PathBuf::from("/nonexistent"),
        manifest: Manifest {
            name: format!("mock-{slot}"),
            family,
            languages: vec![slot.to_owned()],
            files: BTreeMap::new(),
            license: "test".into(),
            source: "mock".into(),
            installed_at_unix: 0,
            archive_sha256: None,
            sha256: BTreeMap::new(),
        },
    };
    Catalog {
        slots: [
            ("de", entry("de", Family::NemoTransducer)),
            ("en", entry("en", Family::NemoTransducer)),
            ("langid", entry("langid", Family::Whisper)),
        ]
        .map(|(slot, e)| (slot.to_owned(), e))
        .into(),
        errors: BTreeMap::new(),
    }
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

fn zero_crossings(samples: &[f32]) -> f32 {
    let n = samples
        .windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count();
    n as f32 / samples.len().max(1) as f32
}

/// One word per half second with sound.
pub fn transcribe(samples: &[f32], language: &str) -> Hypothesis {
    let word = if language == "en" { "hello" } else { "hallo" };
    let chunk = MODEL_RATE as usize / 2;
    let words: Vec<Word> = samples
        .chunks(chunk)
        .enumerate()
        .filter(|(_, c)| c.len() >= chunk / 2 && rms(c) >= 0.01)
        .map(|(i, _)| Word {
            word: word.into(),
            start: i as f32 * 0.5,
            end: i as f32 * 0.5 + 0.4,
        })
        .collect();
    Hypothesis {
        text: words
            .iter()
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        words,
    }
}

impl Backend for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn serve(&self, catalog: &Catalog, _gpu: Option<&Gpu>, port: &mut Port) -> Result<(), String> {
        let models = catalog
            .slots
            .iter()
            .map(|(slot, e)| {
                let broken = e.manifest.name.starts_with("broken");
                LoadedModel {
                    slot: slot.clone(),
                    id: e.id.clone(),
                    name: e.manifest.name.clone(),
                    family: e.manifest.family,
                    state: if broken { "error" } else { "ready" },
                    vram_mib: Some(100),
                    load_secs: 0.0,
                    error: broken.then(|| "mock load failure".to_owned()),
                }
            })
            .collect();
        port.ready(models);
        while let Some(batch) = port.next_batch() {
            if !self.delay.is_zero() {
                std::thread::sleep(self.delay);
            }
            if self.fail.load(Ordering::Relaxed) {
                port.complete(vec![Err("mock device failure".into()); batch.len()]);
                return Err("mock device failure".into());
            }
            let results = batch
                .iter()
                .map(|task| {
                    Ok(match &task.kind {
                        TaskKind::LangId => Output::Language(
                            if zero_crossings(&task.samples) > 0.1 {
                                "en"
                            } else {
                                "de"
                            }
                            .into(),
                        ),
                        TaskKind::Decode { language } => {
                            Output::Text(transcribe(&task.samples, language))
                        }
                    })
                })
                .collect();
            port.complete(results);
        }
        Ok(())
    }
}
