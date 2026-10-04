//! Core domain types, database access and the Postgres-backed job queue shared
//! by all TalkOps services.

pub mod db;
pub mod dialing;
pub mod jobs;
pub mod presets;
pub mod telemetry;
pub mod tenant;

/// Version of the TalkOps build, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
