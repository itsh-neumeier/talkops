//! Shared parts of the TalkOps speech services `coral-stt` (ADR 0046) and
//! `cuda-stt` (ADR 0054).
//!
//! Both services offer the same subset of the OpenAI transcription interface
//! and the same realtime events, so the media worker uses them like any
//! OpenAI-compatible connection. A service implements [engine::SpeechEngine]
//! for its accelerator; this crate holds the code that does not depend on
//! it: audio input, turn split, diagnostics capture, the HTTP routes, the
//! realtime protocol and the license statements of the model library.

pub mod audio;
pub mod diagnostics;
pub mod engine;
pub mod http;
pub mod library;
pub mod realtime;
pub mod turns;

pub use engine::{Choice, Decoded, EngineError, Priority, SpeechEngine, SpeechTurn, Word};
