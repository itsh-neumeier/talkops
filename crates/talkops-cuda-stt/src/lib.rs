//! `cuda-stt`: German and English speech recognition on an NVIDIA GPU
//! (ADR 0053), with the same subset of the OpenAI transcription interface
//! as `coral-stt` (ADR 0046), so the TalkOps media worker uses both the same
//! way. sherpa-onnx runs NeMo (CTC, transducer/TDT, Canary) and Whisper
//! models with the ONNX Runtime CUDA execution provider.
//!
//! The service is separate from the media worker, so the CUDA libraries and
//! models are not in the worker image. It listens on the loopback address
//! only by default: audio does not leave the server.

pub mod engine;
pub mod gpu;
pub mod http;
pub mod library;
pub mod mock;
pub mod models;
pub mod recognizer;
pub mod sherpa;
