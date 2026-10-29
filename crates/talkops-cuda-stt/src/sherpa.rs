//! The sherpa-onnx backend: ONNX Runtime with the CUDA (or CPU) execution
//! provider through the safe `sherpa-onnx` crate (ADR 0052).
//!
//! Each language slot gets its own recognizer. A language-agnostic model
//! (NeMo CTC or transducer) that fills both language slots is loaded once.
//! Canary and Whisper take the language as a parameter of the recognizer,
//! so they get one instance per slot.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineRecognizerResult,
    SpokenLanguageIdentification, SpokenLanguageIdentificationConfig,
    SpokenLanguageIdentificationWhisperConfig,
};
use talkops_stt_service::Word;
use talkops_stt_service::audio::MODEL_RATE;

use crate::engine::{Backend, Hypothesis, LoadedModel, Output, Port, Task, TaskKind};
use crate::gpu::Gpu;
use crate::models::{Catalog, Entry, Family, Manifest};

pub struct SherpaBackend {
    /// `cuda` or `cpu`.
    pub provider: &'static str,
    /// CPU threads of ONNX Runtime (CPU provider and CPU parts of a model).
    pub threads: i32,
}

fn path(dir: &Path, manifest: &Manifest, role: &str) -> Option<String> {
    manifest
        .file(dir, role)
        .map(|p| p.to_string_lossy().into_owned())
}

/// The recognizer configuration of a model for one language.
pub fn recognizer_config(
    dir: &Path,
    manifest: &Manifest,
    language: &str,
    provider: &str,
    threads: i32,
) -> OfflineRecognizerConfig {
    let mut config = OfflineRecognizerConfig::default();
    let model = &mut config.model_config;
    let file = |role| path(dir, manifest, role);
    match manifest.family {
        Family::NemoCtc => model.nemo_ctc.model = file("model"),
        Family::NemoTransducer => {
            model.transducer.encoder = file("encoder");
            model.transducer.decoder = file("decoder");
            model.transducer.joiner = file("joiner");
            model.model_type = Some("nemo_transducer".into());
        }
        Family::Canary => {
            model.canary.encoder = file("encoder");
            model.canary.decoder = file("decoder");
            model.canary.src_lang = Some(language.into());
            model.canary.tgt_lang = Some(language.into());
            model.canary.use_pnc = true;
        }
        Family::Whisper => {
            model.whisper.encoder = file("encoder");
            model.whisper.decoder = file("decoder");
            model.whisper.language = Some(language.into());
            model.whisper.task = Some("transcribe".into());
            // The sherpa-onnx default (the C API treats -1 as "not set").
            model.whisper.tail_paddings = -1;
        }
    }
    model.tokens = file("tokens");
    model.num_threads = threads.max(1);
    model.provider = Some(provider.into());
    config.decoding_method = Some("greedy_search".into());
    config
}

fn langid_config(
    dir: &Path,
    manifest: &Manifest,
    provider: &str,
    threads: i32,
) -> SpokenLanguageIdentificationConfig {
    SpokenLanguageIdentificationConfig {
        whisper: SpokenLanguageIdentificationWhisperConfig {
            encoder: path(dir, manifest, "encoder"),
            decoder: path(dir, manifest, "decoder"),
            tail_paddings: -1,
        },
        num_threads: threads.max(1),
        debug: false,
        provider: Some(provider.into()),
    }
}

/// Word times from the tokens: a token that starts with a space or the
/// SentencePiece marker `▁` starts a new word.
pub fn words(result: &OfflineRecognizerResult) -> Vec<Word> {
    let Some(times) = &result.timestamps else {
        return Vec::new();
    };
    let mut words: Vec<Word> = Vec::new();
    for (i, token) in result.tokens.iter().enumerate() {
        let Some(&start) = times.get(i) else { break };
        let end = result
            .durations
            .as_ref()
            .and_then(|d| d.get(i))
            .map(|d| start + d)
            .or_else(|| times.get(i + 1).copied())
            .unwrap_or(start + 0.1);
        let piece = token.trim_start_matches(['▁', ' ']);
        // Punctuation belongs to the word before it.
        let punctuation = !piece.is_empty() && piece.chars().all(|c| c.is_ascii_punctuation());
        let new_word = (token.starts_with(['▁', ' ']) && !punctuation) || words.is_empty();
        if new_word {
            if piece.is_empty() {
                continue;
            }
            words.push(Word {
                word: piece.to_owned(),
                start,
                end,
            });
        } else if let Some(last) = words.last_mut() {
            last.word.push_str(piece);
            last.end = end;
        }
    }
    words
}

fn hypothesis(result: Option<OfflineRecognizerResult>) -> Result<Output, String> {
    let result = result.ok_or("the recognizer returned no result")?;
    Ok(Output::Text(Hypothesis {
        words: words(&result),
        text: result.text.trim().to_owned(),
    }))
}

/// Loads a model and decodes one second of silence (`check-model`). A
/// Whisper model that can recognize speech can also identify the language.
pub fn check(dir: &Path, manifest: &Manifest, provider: &str) -> Result<(), String> {
    let language = manifest.languages.first().map_or("en", String::as_str);
    let silence = vec![0.0f32; MODEL_RATE as usize];
    let recognizer =
        OfflineRecognizer::create(&recognizer_config(dir, manifest, language, provider, 1))
            .ok_or("sherpa-onnx cannot load the model")?;
    let stream = recognizer.create_stream();
    stream.accept_waveform(MODEL_RATE as i32, &silence);
    recognizer.decode(&stream);
    stream.get_result().ok_or("the model gave no result")?;
    Ok(())
}

impl SherpaBackend {
    fn load(
        &self,
        slot: &str,
        entry: &Entry,
        gpu: Option<&Gpu>,
        f: impl FnOnce() -> Option<Loaded>,
    ) -> (LoadedModel, Option<Loaded>) {
        let before = gpu.and_then(Gpu::used_mib);
        let started = Instant::now();
        let loaded = f();
        let after = gpu.and_then(Gpu::used_mib);
        let error = loaded.is_none().then(|| {
            format!(
                "sherpa-onnx cannot load the model (provider {}); see the service log",
                self.provider
            )
        });
        if let Some(err) = &error {
            tracing::warn!(slot, model = %entry.id, error = %err, "model not loaded");
        }
        let model = LoadedModel {
            slot: slot.to_owned(),
            id: entry.id.clone(),
            name: entry.manifest.name.clone(),
            family: entry.manifest.family,
            state: if loaded.is_some() { "ready" } else { "error" },
            vram_mib: before.zip(after).map(|(b, a)| a.saturating_sub(b)),
            load_secs: started.elapsed().as_secs_f32(),
            error,
        };
        (model, loaded)
    }
}

enum Loaded {
    Asr(Arc<OfflineRecognizer>),
    LangId(SpokenLanguageIdentification),
}

impl Backend for SherpaBackend {
    fn name(&self) -> &'static str {
        self.provider
    }

    fn serve(&self, catalog: &Catalog, gpu: Option<&Gpu>, port: &mut Port) -> Result<(), String> {
        let mut recognizers: BTreeMap<String, Arc<OfflineRecognizer>> = BTreeMap::new();
        let mut langid = None;
        let mut models = Vec::new();
        for language in crate::models::LANGUAGES {
            let Some(entry) = catalog.get(language) else {
                continue;
            };
            // One instance for both languages of a language-agnostic model.
            let shared = entry.manifest.family.language_agnostic().then(|| {
                catalog.slots.iter().find_map(|(slot, other)| {
                    (other.id == entry.id)
                        .then(|| recognizers.get(slot).cloned())
                        .flatten()
                })
            });
            if let Some(Some(recognizer)) = shared {
                recognizers.insert(language.to_owned(), recognizer);
                models.push(LoadedModel {
                    slot: language.to_owned(),
                    id: entry.id.clone(),
                    name: entry.manifest.name.clone(),
                    family: entry.manifest.family,
                    state: "ready",
                    vram_mib: Some(0),
                    load_secs: 0.0,
                    error: None,
                });
                continue;
            }
            let config = recognizer_config(
                &entry.dir,
                &entry.manifest,
                language,
                self.provider,
                self.threads,
            );
            let (model, loaded) = self.load(language, entry, gpu, || {
                OfflineRecognizer::create(&config).map(|r| Loaded::Asr(Arc::new(r)))
            });
            models.push(model);
            if let Some(Loaded::Asr(r)) = loaded {
                recognizers.insert(language.to_owned(), r);
            }
        }
        if let Some(entry) = catalog.get("langid") {
            let config = langid_config(&entry.dir, &entry.manifest, self.provider, self.threads);
            let (model, loaded) = self.load("langid", entry, gpu, || {
                SpokenLanguageIdentification::create(&config).map(Loaded::LangId)
            });
            models.push(model);
            if let Some(Loaded::LangId(l)) = loaded {
                langid = Some(l);
            }
        }
        port.ready(models);
        while let Some(batch) = port.next_batch() {
            let results = match &batch[0].kind {
                TaskKind::Decode { language } => match recognizers.get(language) {
                    Some(recognizer) => decode(recognizer, &batch),
                    None => vec![Err(format!("no model for `{language}`")); batch.len()],
                },
                TaskKind::LangId => match &langid {
                    Some(langid) => batch
                        .iter()
                        .map(|task| {
                            let stream = langid.create_stream();
                            stream.accept_waveform(MODEL_RATE as i32, &task.samples);
                            langid
                                .compute(&stream)
                                .map(|r| Output::Language(r.lang))
                                .ok_or_else(|| "language ID gave no result".to_owned())
                        })
                        .collect(),
                    None => vec![Err("no language ID model".into()); batch.len()],
                },
            };
            port.complete(results);
        }
        Ok(())
    }
}

fn decode(recognizer: &OfflineRecognizer, batch: &[Task]) -> Vec<Result<Output, String>> {
    let streams: Vec<_> = batch
        .iter()
        .map(|task| {
            let stream = recognizer.create_stream();
            stream.accept_waveform(MODEL_RATE as i32, &task.samples);
            stream
        })
        .collect();
    let refs: Vec<_> = streams.iter().collect();
    recognizer.decode_multiple_streams(&refs);
    streams.iter().map(|s| hypothesis(s.get_result())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(tokens: &[&str], times: Option<Vec<f32>>) -> OfflineRecognizerResult {
        OfflineRecognizerResult {
            text: tokens.concat(),
            tokens: tokens.iter().map(|t| (*t).to_owned()).collect(),
            timestamps: times,
            durations: None,
        }
    }

    #[test]
    fn tokens_become_words() {
        let r = result(
            &["▁Hal", "lo", "▁Welt", "▁."],
            Some(vec![0.0, 0.2, 0.5, 0.9]),
        );
        let w = words(&r);
        assert_eq!(
            w.iter().map(|w| w.word.as_str()).collect::<Vec<_>>(),
            ["Hallo", "Welt."]
        );
        assert_eq!((w[0].start, w[0].end), (0.0, 0.5));
        assert_eq!((w[1].start, w[1].end), (0.5, 1.0));
        assert!(words(&result(&["▁a"], None)).is_empty());
    }

    #[test]
    fn whisper_tokens_start_words_with_a_space() {
        let r = result(&[" Guten", " Tag", "!"], Some(vec![0.0, 0.4, 0.7]));
        let w = words(&r);
        assert_eq!(
            w.iter().map(|w| w.word.as_str()).collect::<Vec<_>>(),
            ["Guten", "Tag!"]
        );
    }

    #[test]
    fn configs_select_the_family_fields() {
        let manifest = Manifest {
            name: "x".into(),
            family: Family::Canary,
            languages: vec!["de".into(), "en".into()],
            files: [
                ("encoder", "e.onnx"),
                ("decoder", "d.onnx"),
                ("tokens", "tokens.txt"),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .into(),
            license: "CC-BY-4.0".into(),
            source: "x".into(),
            installed_at_unix: 0,
            archive_sha256: None,
            sha256: BTreeMap::new(),
        };
        let c = recognizer_config(Path::new("/m"), &manifest, "de", "cuda", 2);
        assert_eq!(c.model_config.canary.src_lang.as_deref(), Some("de"));
        assert_eq!(c.model_config.canary.encoder.as_deref(), Some("/m/e.onnx"));
        assert_eq!(c.model_config.provider.as_deref(), Some("cuda"));
        assert_eq!(c.model_config.tokens.as_deref(), Some("/m/tokens.txt"));
    }
}
