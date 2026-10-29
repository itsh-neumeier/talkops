//! Speech recognition of one turn on top of the [Engine] (ADR 0052).
//!
//! The models are not streaming models. While a live turn grows, the turn
//! decodes all its audio again after each partial interval and publishes the
//! new text. A turn longer than the turn limit is cut at its quietest
//! 100 ms in the last five seconds; the first part is decoded once and
//! stays fixed. The language is fixed, comes from the language ID model on
//! the first two seconds, or else is the first preferred language.

use std::sync::Arc;

use talkops_stt_service::audio::MODEL_RATE;
use talkops_stt_service::{Choice, Decoded, EngineError, Priority, SpeechTurn, Word};

use crate::engine::{Engine, Hypothesis, Output, Task, TaskKind};
use crate::models::AUTO;

/// Audio for language identification.
const LANGID_SAMPLES: usize = 2 * MODEL_RATE as usize;
/// The part of a long turn that is searched for a pause.
const CUT_SEARCH_SAMPLES: usize = 5 * MODEL_RATE as usize;
const CUT_FRAME: usize = MODEL_RATE as usize / 10;

pub struct Turn {
    engine: Arc<Engine>,
    choice: Choice,
    priority: Priority,
    language: Option<String>,
    /// Audio of the open part.
    samples: Vec<f32>,
    /// Samples before the open part.
    offset: usize,
    /// Fixed parts of a long turn, word times from the start of the turn.
    done: Vec<Hypothesis>,
    /// Samples since the last partial decode.
    since: usize,
    last: Option<Hypothesis>,
    previous: Option<Hypothesis>,
    /// Words of the open part that two decodes agreed on.
    stable: Vec<String>,
}

fn words_of(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

/// The start of the quietest frame in the last part of `samples`.
fn quiet_cut(samples: &[f32]) -> usize {
    let from = samples.len().saturating_sub(CUT_SEARCH_SAMPLES);
    (from..samples.len().saturating_sub(CUT_FRAME))
        .step_by(CUT_FRAME)
        .min_by(|&a, &b| {
            let energy = |i: usize| samples[i..i + CUT_FRAME].iter().map(|s| s * s).sum::<f32>();
            energy(a).total_cmp(&energy(b))
        })
        .map_or(samples.len(), |i| i + CUT_FRAME / 2)
}

impl Turn {
    pub fn new(
        engine: Arc<Engine>,
        choice: Choice,
        priority: Priority,
    ) -> Result<Self, EngineError> {
        let available = engine.loaded_languages();
        let (choice, language) = match choice {
            Choice::Fixed(lang) => {
                if !available.contains(&lang) {
                    return Err(EngineError::UnknownModel(format!("cuda-{lang}")));
                }
                (Choice::Fixed(lang.clone()), Some(lang))
            }
            Choice::Auto(order) => {
                let order: Vec<String> = order
                    .into_iter()
                    .filter(|l| available.contains(l))
                    .collect();
                let language = match order.as_slice() {
                    [] => return Err(EngineError::UnknownModel(AUTO.into())),
                    [only] => Some(only.clone()),
                    _ if engine.has_langid() => None,
                    [first, ..] => Some(first.clone()),
                };
                (Choice::Auto(order), language)
            }
        };
        Ok(Self {
            engine,
            choice,
            priority,
            language,
            samples: Vec::new(),
            offset: 0,
            done: Vec::new(),
            since: 0,
            last: None,
            previous: None,
            stable: Vec::new(),
        })
    }

    async fn identify(&mut self) -> Result<(), EngineError> {
        let allowed = match &self.choice {
            Choice::Auto(order) => order.clone(),
            Choice::Fixed(l) => vec![l.clone()],
        };
        let task = Task {
            kind: TaskKind::LangId,
            samples: self.samples[..self.samples.len().min(LANGID_SAMPLES)].to_vec(),
        };
        let detected = match self.engine.run(task, self.priority).await? {
            Output::Language(l) => Some(l),
            Output::Text(_) => None,
        };
        self.language = detected
            .filter(|l| allowed.contains(l))
            .or_else(|| self.choice.preferred().map(str::to_owned));
        Ok(())
    }

    async fn decode(&self, samples: &[f32]) -> Result<Hypothesis, EngineError> {
        let language = self.language.clone().unwrap_or_default();
        let task = Task {
            kind: TaskKind::Decode { language },
            samples: samples.to_vec(),
        };
        match self.engine.run(task, self.priority).await? {
            Output::Text(h) => Ok(h),
            Output::Language(_) => Err(EngineError::Failed("unexpected output".into())),
        }
    }

    /// Words with times from the start of the turn.
    fn shifted(mut h: Hypothesis, offset: usize) -> Hypothesis {
        let secs = offset as f32 / MODEL_RATE as f32;
        for w in &mut h.words {
            w.start += secs;
            w.end += secs;
        }
        h
    }

    /// Decodes the first part of a long turn once and keeps the rest open.
    async fn cut(&mut self) -> Result<(), EngineError> {
        let at = quiet_cut(&self.samples);
        let head = self.decode(&self.samples[..at]).await?;
        self.done.push(Self::shifted(head, self.offset));
        self.samples.drain(..at);
        self.offset += at;
        self.since = self.samples.len();
        self.last = None;
        self.previous = None;
        self.stable.clear();
        Ok(())
    }

    fn update_stable(&mut self) {
        let (Some(last), Some(previous)) = (&self.last, &self.previous) else {
            return;
        };
        let (now, before) = (words_of(&last.text), words_of(&previous.text));
        let agreed = now.iter().zip(&before).take_while(|(a, b)| a == b).count();
        // The last word of the newest decode can still grow.
        let stable = &now[..agreed.min(now.len().saturating_sub(1))];
        if stable.len() > self.stable.len() && stable.starts_with(&self.stable) {
            self.stable = stable.to_vec();
        }
    }

    fn joined<'a>(&'a self, open: impl IntoIterator<Item = &'a str>) -> String {
        self.done
            .iter()
            .map(|h| h.text.as_str())
            .chain(open)
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Adds audio (16 kHz).
    pub async fn push(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        self.samples.extend_from_slice(samples);
        self.since += samples.len();
        if self.language.is_none() {
            if self.samples.len() < LANGID_SAMPLES {
                return Ok(());
            }
            self.identify().await?;
        }
        if self.samples.len() >= self.engine.max_turn_samples {
            self.cut().await?;
        }
        if self.priority == Priority::Live && self.since >= self.engine.partial_samples {
            self.since = 0;
            match self.decode(&self.samples).await {
                Ok(h) => {
                    self.previous = self.last.replace(h);
                    self.update_stable();
                }
                // A partial result can wait for the next interval.
                Err(EngineError::Overloaded) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub fn partial(&self) -> Option<String> {
        self.language.as_ref()?;
        Some(self.joined(self.last.as_ref().map(|h| h.text.as_str())))
    }

    pub fn stable_partial(&self) -> Option<String> {
        self.language.as_ref()?;
        Some(self.joined(self.stable.iter().map(String::as_str)))
    }

    pub async fn finish(mut self) -> Result<Decoded, EngineError> {
        if self.samples.is_empty() && self.done.is_empty() {
            return Ok(Decoded {
                text: String::new(),
                words: Vec::new(),
                confidence: 0.0,
                language: None,
            });
        }
        if self.language.is_none() {
            self.identify().await?;
        }
        if !self.samples.is_empty() {
            let rest = self.decode(&self.samples).await?;
            self.done.push(Self::shifted(rest, self.offset));
        }
        let text = self.joined(None);
        let words: Vec<Word> = self.done.into_iter().flat_map(|h| h.words).collect();
        Ok(Decoded {
            text,
            words,
            confidence: 0.0,
            language: self.language,
        })
    }
}

impl SpeechTurn for Turn {
    fn push(
        &mut self,
        samples: &[f32],
    ) -> impl std::future::Future<Output = Result<(), EngineError>> + Send {
        Turn::push(self, samples)
    }

    fn partial(&self) -> Option<String> {
        Turn::partial(self)
    }

    fn stable_partial(&self) -> Option<String> {
        Turn::stable_partial(self)
    }

    fn finish(self) -> impl std::future::Future<Output = Result<Decoded, EngineError>> + Send {
        Turn::finish(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_is_at_the_quietest_frame() {
        let rate = MODEL_RATE as usize;
        let mut samples = vec![0.5f32; 20 * rate];
        // A pause from 17.0 to 17.1 s.
        samples[17 * rate..17 * rate + CUT_FRAME].fill(0.0);
        let at = quiet_cut(&samples);
        assert_eq!(at, 17 * rate + CUT_FRAME / 2);
        assert_eq!(quiet_cut(&[]), 0);
    }
}
