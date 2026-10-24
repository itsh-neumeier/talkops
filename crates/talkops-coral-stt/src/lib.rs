//! `coral-stt`: German and English speech recognition on Google Coral Edge
//! TPUs (ADR 0046), with a subset of the OpenAI transcription interface, so
//! the TalkOps media worker can use it like any OpenAI-compatible
//! connection. Small CTC models with fixed windows run on the TPU; the host
//! computes the log-mel features and decodes the output.
//!
//! The service is separate from the media worker, so the TPU libraries and
//! models are not in the worker image. It listens on the loopback address
//! only by default: audio does not leave the server.

pub mod audio;
pub mod ctc;
pub mod devices;
pub mod engine;
pub mod features;
mod fft;
pub mod http;
pub mod mock;
pub mod models;
pub mod realtime;
pub mod recognizer;
pub mod tflite;
pub mod turns;
