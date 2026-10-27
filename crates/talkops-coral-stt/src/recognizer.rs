//! Speech recognition of one turn on top of the [Engine].
//!
//! The Edge TPU needs fixed tensor shapes, so audio runs in fixed windows
//! with overlap. A [Stream] runs each window as soon as its audio is
//! complete; at the end of a turn only the last window or two remain. A
//! [Turn] selects the language: fixed, by the language ID model on the first
//! window, or (without that model) by the CTC confidence of all models.

use std::sync::Arc;

use crate::ctc::{self, Matrix, Word};
use crate::engine::{Engine, EngineError, Output, Priority, Task, TaskKind};
use crate::models::ModelFile;

/// A classifier result below this probability uses the preferred language.
const LANGID_MIN: f32 = 0.6;
/// A confidence this close to the best one still selects the preferred
/// language.
const CONFIDENCE_MARGIN: f32 = 0.05;

/// How the language of a turn is chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    Fixed(String),
    /// Languages in order of preference; the first is the tenant language.
    Auto(Vec<String>),
}

/// The text of a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub text: String,
    /// Word times from the start of the turn.
    pub words: Vec<Word>,
    pub confidence: f32,
    pub language: Option<String>,
}

fn window_task(model: &ModelFile, samples: &[f32], kind: TaskKind) -> Task {
    let frames = model.meta.window_frames;
    let mut window = samples[..samples.len().min(model.window_samples())].to_vec();
    window.resize(model.window_samples(), 0.0);
    Task {
        kind,
        features: model.mel.compute(&window, frames),
        samples: window,
    }
}

/// Incremental CTC recognition of one turn with one model.
pub struct Stream {
    model: ModelFile,
    samples: Vec<f32>,
    outputs: Vec<Matrix>,
    priority: Priority,
}

impl Stream {
    pub fn new(model: ModelFile, priority: Priority) -> Self {
        Self {
            model,
            samples: Vec::new(),
            outputs: Vec::new(),
            priority,
        }
    }

    fn step(&self) -> usize {
        (self.model.meta.window_frames - self.model.meta.overlap()) * self.model.mel.fe.hop()
    }

    async fn run_ready(&mut self, engine: &Engine, last: bool) -> Result<(), EngineError> {
        loop {
            let start = self.outputs.len() * self.step();
            let end = start + self.model.window_samples();
            if end > self.samples.len() && !(last && start < self.samples.len()) {
                return Ok(());
            }
            let task = window_task(
                &self.model,
                &self.samples[start..self.samples.len().min(end)],
                TaskKind::Ctc {
                    language: self.model.language.clone().unwrap_or_default(),
                },
            );
            match engine.run(task, self.priority).await? {
                Output::Matrix(m) => self.outputs.push(m),
                Output::Probs(_) => return Err(EngineError::Failed("unexpected output".into())),
            }
            if end >= self.samples.len() {
                return Ok(());
            }
        }
    }

    /// Adds audio (16 kHz) and runs all complete windows.
    pub async fn push(&mut self, engine: &Engine, samples: &[f32]) -> Result<(), EngineError> {
        self.samples.extend_from_slice(samples);
        self.run_ready(engine, false).await
    }

    fn decode(&self, total: usize) -> Decoded {
        let meta = &self.model.meta;
        let rows = ctc::stitch(
            &self.outputs,
            meta.window_frames,
            meta.overlap(),
            total,
            meta.subsampling,
        );
        let ids: Vec<usize> = rows.iter().map(|r| ctc::argmax(r)).collect();
        let blank = meta.classes_with_blank() - 1;
        let frame_secs = self.model.mel.fe.window_stride * meta.subsampling as f32;
        let words = ctc::collapse(&ids, &meta.labels, blank, frame_secs);
        Decoded {
            text: words
                .iter()
                .map(|w| w.word.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            words,
            confidence: ctc::confidence(&rows, blank, meta.log_probs),
            language: self.model.language.clone(),
        }
    }

    /// The text of the windows done so far, without the end of the last
    /// window (the next window can still change it).
    pub fn partial(&self) -> String {
        let Some(last) = self.outputs.len().checked_sub(1) else {
            return String::new();
        };
        let meta = &self.model.meta;
        let step = meta.window_frames - meta.overlap();
        // Treat the next window as present: the last window keeps only its
        // stable part.
        let total = last * step + meta.window_frames - meta.overlap() / 2;
        let mut outputs = self.outputs.clone();
        outputs.push(Matrix::new(0, meta.classes_with_blank(), Vec::new()));
        let stream = Stream {
            model: self.model.clone(),
            samples: Vec::new(),
            outputs,
            priority: self.priority,
        };
        let mut d = stream.decode(total + step);
        // Words that start after the stable part are not final yet.
        let limit = total as f32 * self.model.mel.fe.window_stride;
        d.words.retain(|w| w.end <= limit);
        d.words
            .iter()
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Complete words from immutable centre frames. Keep the open last
    /// word until a space closes it or the client commits the turn.
    pub fn stable_partial(&self) -> String {
        let Some(last) = self.outputs.len().checked_sub(1) else {
            return String::new();
        };
        let meta = &self.model.meta;
        let end = last * (meta.window_frames - meta.overlap()) + meta.window_frames;
        let keep = end - (meta.overlap() - meta.overlap() / 2);
        let rows = ctc::stitch(
            &self.outputs,
            meta.window_frames,
            meta.overlap(),
            end,
            meta.subsampling,
        );
        let ids: Vec<usize> = rows
            .iter()
            .take(keep.div_ceil(meta.subsampling))
            .map(|row| ctc::argmax(row))
            .collect();
        let blank = meta.classes_with_blank() - 1;
        let mut words = ctc::collapse(
            &ids,
            &meta.labels,
            blank,
            self.model.mel.fe.window_stride * meta.subsampling as f32,
        );
        let closed = ids
            .iter()
            .rev()
            .find(|&&id| id != blank)
            .and_then(|&id| meta.labels.get(id))
            .is_some_and(|label| label == " ");
        if !closed {
            words.pop();
        }
        words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Runs the rest of the audio and decodes the turn.
    pub async fn finish(&mut self, engine: &Engine) -> Result<Decoded, EngineError> {
        if self.samples.is_empty() {
            return Ok(Decoded {
                text: String::new(),
                words: Vec::new(),
                confidence: f32::NEG_INFINITY,
                language: self.model.language.clone(),
            });
        }
        self.run_ready(engine, true).await?;
        Ok(self.decode(self.model.mel.fe.frames(self.samples.len())))
    }
}

/// One turn of one speaker.
pub struct Turn {
    engine: Arc<Engine>,
    choice: Choice,
    priority: Priority,
    /// Audio before the language is known (classifier mode).
    buffer: Vec<f32>,
    pub language: Option<String>,
    streams: Vec<Stream>,
}

impl Turn {
    pub fn new(
        engine: Arc<Engine>,
        choice: Choice,
        priority: Priority,
    ) -> Result<Self, EngineError> {
        let mut turn = Self {
            engine,
            choice,
            priority,
            buffer: Vec::new(),
            language: None,
            streams: Vec::new(),
        };
        match turn.choice.clone() {
            Choice::Fixed(lang) => turn.select(&lang)?,
            Choice::Auto(langs) => {
                let available: Vec<String> = langs
                    .into_iter()
                    .filter(|l| turn.engine.catalog.ctc(l).is_some())
                    .collect();
                match available.as_slice() {
                    [] => return Err(EngineError::UnknownModel("coral-auto".into())),
                    [only] => turn.select(&only.clone())?,
                    _ if turn.engine.catalog.langid.is_some() => {}
                    _ => {
                        for lang in &available {
                            let model = turn.engine.catalog.ctc(lang).cloned();
                            turn.streams.extend(model.map(|m| Stream::new(m, priority)));
                        }
                    }
                }
                turn.choice = Choice::Auto(available);
            }
        }
        Ok(turn)
    }

    fn select(&mut self, language: &str) -> Result<(), EngineError> {
        let model = self
            .engine
            .catalog
            .ctc(language)
            .cloned()
            .ok_or_else(|| EngineError::UnknownModel(format!("coral-ctc-{language}")))?;
        self.language = Some(language.to_owned());
        self.streams = vec![Stream::new(model, self.priority)];
        Ok(())
    }

    fn preferred(&self) -> String {
        match &self.choice {
            Choice::Fixed(l) => l.clone(),
            Choice::Auto(langs) => langs.first().cloned().unwrap_or_default(),
        }
    }

    async fn classify(&mut self) -> Result<(), EngineError> {
        let Some(langid) = self.engine.catalog.langid.clone() else {
            return Ok(());
        };
        let task = window_task(&langid, &self.buffer, TaskKind::LangId);
        let probs = match self.engine.run(task, self.priority).await? {
            Output::Probs(p) => p,
            Output::Matrix(_) => return Err(EngineError::Failed("unexpected output".into())),
        };
        let allowed = match &self.choice {
            Choice::Auto(langs) => langs.clone(),
            Choice::Fixed(l) => vec![l.clone()],
        };
        let best = langid
            .meta
            .classes
            .iter()
            .zip(&probs)
            .filter(|(c, _)| allowed.contains(c))
            .max_by(|a, b| a.1.total_cmp(b.1));
        let language = match best {
            Some((class, p)) if *p >= LANGID_MIN => class.clone(),
            _ => self.preferred(),
        };
        self.select(&language)?;
        let buffer = std::mem::take(&mut self.buffer);
        let engine = self.engine.clone();
        self.streams[0].push(&engine, &buffer).await
    }

    /// Adds audio (16 kHz).
    pub async fn push(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        let engine = self.engine.clone();
        if self.streams.is_empty() {
            self.buffer.extend_from_slice(samples);
            let need = engine
                .catalog
                .langid
                .as_ref()
                .map_or(0, ModelFile::window_samples);
            if self.buffer.len() >= need {
                self.classify().await?;
            }
            return Ok(());
        }
        for stream in &mut self.streams {
            stream.push(&engine, samples).await?;
        }
        Ok(())
    }

    /// Stable text so far (only once the language is known).
    pub fn partial(&self) -> Option<String> {
        match self.streams.as_slice() {
            [one] => Some(one.partial()),
            _ => None,
        }
    }

    /// Conservative preview, after the language of the turn is known.
    pub fn stable_partial(&self) -> Option<String> {
        match self.streams.as_slice() {
            [one] => Some(one.stable_partial()),
            _ => None,
        }
    }

    pub async fn finish(mut self) -> Result<Decoded, EngineError> {
        if self.streams.is_empty() {
            if self.buffer.is_empty() {
                return Ok(Decoded {
                    text: String::new(),
                    words: Vec::new(),
                    confidence: f32::NEG_INFINITY,
                    language: None,
                });
            }
            self.classify().await?;
        }
        let engine = self.engine.clone();
        let mut results = Vec::new();
        for stream in &mut self.streams {
            results.push(stream.finish(&engine).await?);
        }
        let preferred = self.preferred();
        let best = results
            .iter()
            .map(|d| d.confidence)
            .fold(f32::NEG_INFINITY, f32::max);
        let pick = results
            .iter()
            .position(|d| {
                d.language.as_deref() == Some(preferred.as_str())
                    && d.confidence >= best - CONFIDENCE_MARGIN
            })
            .or_else(|| results.iter().position(|d| d.confidence == best))
            .unwrap_or(0);
        Ok(results.swap_remove(pick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(ids: &[usize]) -> Stream {
        let mut model = crate::mock::catalog(false).ctc.remove(0);
        model.meta.labels = [" ", "a", "b"].map(str::to_owned).to_vec();
        model.meta.window_frames = 8;
        model.meta.overlap_frames = Some(4);
        model.meta.subsampling = 1;
        let mut stream = Stream::new(model, Priority::Live);
        stream.outputs.push(matrix(ids));
        stream
    }

    fn matrix(ids: &[usize]) -> Matrix {
        let data = ids
            .iter()
            .flat_map(|&id| (0..4).map(move |k| if k == id { 0.0 } else { -10.0 }))
            .collect();
        Matrix::new(ids.len(), 4, data)
    }

    #[test]
    fn stable_preview_holds_an_open_word_and_ignores_unstable_space() {
        let mut s = stream(&[1, 2, 3, 3, 3, 3, 0, 3]);
        assert_eq!(s.stable_partial(), "");
        // The next window confirms the closing space at global frame 6.
        s.outputs.push(matrix(&[3, 3, 0, 3, 3, 3, 3, 3]));
        assert_eq!(s.stable_partial(), "ab");
        assert_eq!(s.decode(12).text, "ab");
    }

    #[test]
    fn stable_preview_publishes_a_closed_word_but_holds_the_next_word() {
        let s = stream(&[1, 0, 2, 3, 3, 3, 1, 0]);
        assert_eq!(s.stable_partial(), "a");
    }

    #[test]
    fn stable_previews_remain_prefixes_across_windows_and_final_decode() {
        let ids: Vec<usize> = [1, 2, 0, 3].into_iter().cycle().take(40).collect();
        for length in 8..=40 {
            let mut s = stream(&ids[..8]);
            let mut previous = String::new();
            for (start, end) in ctc::windows(length, 8, 4) {
                if end <= length {
                    if start > 0 {
                        s.outputs.push(matrix(&ids[start..end]));
                    }
                    let partial = s.stable_partial();
                    assert!(partial.starts_with(&previous));
                    previous = partial;
                } else {
                    let mut tail = ids[start..length].to_vec();
                    tail.resize(8, 3);
                    s.outputs.push(matrix(&tail));
                }
            }
            assert!(s.decode(length).text.starts_with(&previous));
        }
    }
}
