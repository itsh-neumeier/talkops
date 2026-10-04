//! Core domain types, database access and the Postgres-backed job queue shared
//! by all TalkOps services.

pub mod audit;
pub mod cdr;
pub mod crypto;
pub mod db;
pub mod dialing;
pub mod error;
pub mod extensions;
pub mod jobs;
pub mod presets;
pub mod settings;
pub mod telemetry;
pub mod tenant;
pub mod trunks;
pub mod users;

/// Version of the TalkOps build, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
