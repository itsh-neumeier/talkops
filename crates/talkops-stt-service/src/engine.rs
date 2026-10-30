//! The interface between the shared HTTP/realtime code and the recognizer
//! of a service.

use std::future::Future;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::diagnostics::Diagnostics;

/// A recognised word with its time in seconds from the start of the audio.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Word {
    pub word: String,
    pub start: f32,
    pub end: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Realtime sessions.
    Live,
    /// File transcriptions.
    File,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    #[error("all accelerators are busy; try again later")]
    Overloaded,
    #[error("no accelerator is ready")]
    Unavailable,
    #[error("model not available: {0}")]
    UnknownModel(String),
    #[error("inference failed: {0}")]
    Failed(String),
}

impl EngineError {
    /// The error code of the realtime `error` event and the HTTP error body.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Overloaded => "overloaded",
            Self::Unavailable => "unavailable",
            Self::UnknownModel(_) => "model_not_found",
            Self::Failed(_) => "inference_failed",
        }
    }
}

/// How the language of a turn is chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    Fixed(String),
    /// Languages in order of preference; the first is the tenant language.
    Auto(Vec<String>),
}

impl Choice {
    /// The fixed language, or the first preferred one.
    pub fn preferred(&self) -> Option<&str> {
        match self {
            Self::Fixed(l) => Some(l),
            Self::Auto(order) => order.first().map(String::as_str),
        }
    }
}

/// The text of a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub text: String,
    /// Word times from the start of the turn.
    pub words: Vec<Word>,
    /// Model confidence (`avg_logprob` of the segment); 0 if unknown.
    pub confidence: f32,
    pub language: Option<String>,
}

/// One turn of one speaker: audio arrives in parts, the text can be read
/// while it grows.
pub trait SpeechTurn: Send + 'static {
    /// Adds audio (16 kHz).
    fn push(&mut self, samples: &[f32]) -> impl Future<Output = Result<(), EngineError>> + Send;
    /// The text so far, if the turn has a language yet.
    fn partial(&self) -> Option<String>;
    /// Only the complete words that cannot change any more.
    fn stable_partial(&self) -> Option<String>;
    /// Decodes the rest of the turn.
    fn finish(self) -> impl Future<Output = Result<Decoded, EngineError>> + Send;
}

/// The recognizer side of a speech service.
pub trait SpeechEngine: Send + Sync + 'static {
    type Turn: SpeechTurn;
    /// Holds one realtime session slot until it is dropped.
    type Session: Send + 'static;

    /// `talkops-coral` or `talkops-cuda` (`owned_by` of `/v1/models`).
    fn owner(&self) -> &'static str;
    /// The model name that selects the language per turn (`coral-auto`).
    fn auto_model(&self) -> &'static str;
    /// The language of a fixed model name (`coral-ctc-de`, `cuda-de`).
    fn fixed_language(&self, model: &str) -> Option<String>;
    /// Languages with a model, in the default order.
    fn languages(&self) -> Vec<String>;
    /// All model names of `/v1/models`.
    fn model_ids(&self) -> Vec<String>;
    fn diagnostics(&self) -> &Diagnostics;
    fn turn(
        self: &Arc<Self>,
        choice: Choice,
        priority: Priority,
    ) -> Result<Self::Turn, EngineError>;
    /// A realtime session slot; `Unavailable` or `Overloaded` if none is free.
    fn try_session(self: &Arc<Self>) -> Result<Self::Session, EngineError>;
    /// Queue and session numbers.
    fn queue_status(&self) -> Value;
    /// The `/health` body and whether the service can transcribe.
    fn health(&self) -> (bool, Value);
    /// The models as the diagnostics report shows them.
    fn diagnostic_models(&self) -> Value;
}
