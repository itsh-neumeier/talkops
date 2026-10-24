//! A deterministic backend without hardware, for CI and tests.
//!
//! Each window with sound gives one word in the middle of the window:
//! `hallo` (German model) or `hello` (English model). The language ID
//! model says English for audio with many zero crossings (high pitch) and
//! German otherwise. An optional delay makes the device slow, and a flag
//! makes it fail.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::ctc::Matrix;
use crate::devices::DeviceSpec;
use crate::engine::{Backend, Output, Port, TaskKind};
use crate::features::{Frontend, LogMel};
use crate::models::{Catalog, Meta, ModelFile};

pub struct MockBackend {
    pub devices: usize,
    pub delay: Duration,
    /// When set, every inference fails (the device goes to `error`).
    pub fail: Arc<AtomicBool>,
}

impl MockBackend {
    pub fn new(devices: usize, delay: Duration) -> Self {
        Self {
            devices,
            delay,
            fail: Arc::new(AtomicBool::new(false)),
        }
    }
}

const LABELS: &str = " abcdefghijklmnopqrstuvwxyzäöüß'";

fn meta(classes: Vec<String>) -> Meta {
    Meta {
        labels: if classes.is_empty() {
            LABELS.chars().map(String::from).collect()
        } else {
            Vec::new()
        },
        classes,
        window_frames: 256,
        overlap_frames: Some(64),
        subsampling: 2,
        input_layout: "nct".into(),
        log_probs: true,
        frontend: Frontend::default(),
        candidate: Some("mock".into()),
        license: None,
    }
}

/// German and English models and (optionally) the language ID model.
pub fn catalog(langid: bool) -> Catalog {
    let mel = Arc::new(LogMel::new(Frontend::default()));
    let file = |name: &str, language: Option<&str>, m: Meta| ModelFile {
        name: name.into(),
        language: language.map(str::to_owned),
        path: PathBuf::from("mock"),
        meta: m,
        mel: mel.clone(),
    };
    Catalog {
        ctc: vec![
            file("coral-ctc-de", Some("de"), meta(Vec::new())),
            file("coral-ctc-en", Some("en"), meta(Vec::new())),
        ],
        langid: langid.then(|| file("langid", None, meta(vec!["de".into(), "en".into()]))),
    }
}

fn word_frames(word: &str, labels: &[String]) -> Vec<usize> {
    let blank = labels.len();
    let mut ids = vec![0]; // a space before the word
    let mut prev = None;
    for c in word.chars() {
        let k = labels
            .iter()
            .position(|l| l == &c.to_string())
            .unwrap_or(blank);
        if prev == Some(k) {
            ids.push(blank);
        }
        ids.push(k);
        prev = Some(k);
    }
    ids
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

impl Backend for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn discover(&self) -> Vec<DeviceSpec> {
        (0..self.devices)
            .map(|i| DeviceSpec {
                id: format!("mock:{i}"),
                kind: "mock".into(),
                node: None,
                note: None,
            })
            .collect()
    }

    fn serve(&self, _spec: &DeviceSpec, catalog: &Catalog, port: &mut Port) -> Result<(), String> {
        port.ready(catalog.files().iter().map(|m| m.name.clone()).collect());
        while let Some(task) = port.next_task() {
            if !self.delay.is_zero() {
                std::thread::sleep(self.delay);
            }
            if self.fail.load(Ordering::Relaxed) {
                port.complete(Err("mock device failure".into()));
                return Err("mock device failure".into());
            }
            let loud = rms(&task.samples) >= 0.01;
            let output = match task.kind {
                TaskKind::LangId => {
                    let en = if !loud {
                        0.5
                    } else if zero_crossings(&task.samples) > 0.1 {
                        0.9
                    } else {
                        0.1
                    };
                    Output::Probs(vec![1.0 - en, en])
                }
                TaskKind::Ctc { language } => {
                    let model = catalog.ctc(&language).ok_or("unknown model")?;
                    let (rows, cols) = (
                        model.meta.window_frames / model.meta.subsampling,
                        model.meta.classes_with_blank(),
                    );
                    let blank = cols - 1;
                    let mut ids = vec![blank; rows];
                    if loud {
                        let word = if language == "en" { "hello" } else { "hallo" };
                        for (i, k) in word_frames(word, &model.meta.labels)
                            .into_iter()
                            .enumerate()
                        {
                            ids[rows / 2 - 10 + i] = k;
                        }
                    }
                    let mut data = vec![-10.0f32; rows * cols];
                    for (r, k) in ids.into_iter().enumerate() {
                        data[r * cols + k] = 0.0;
                    }
                    Output::Matrix(Matrix::new(rows, cols, data))
                }
            };
            port.complete(Ok(output));
        }
        Ok(())
    }
}
