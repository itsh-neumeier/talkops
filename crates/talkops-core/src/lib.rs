//! Core domain types, database access and the Postgres-backed job queue shared
//! by all TalkOps services.

pub mod attendant;
pub mod audio;
pub mod audit;
pub mod backups;
pub mod blocking;
pub mod cdr;
pub mod contact_import;
pub mod crypto;
pub mod db;
pub mod dialing;
pub mod doors;
pub mod error;
pub mod extensions;
pub mod holidays;
pub mod identity;

pub mod jobs;
pub mod mail;
pub mod mfa;
pub mod numbering;
pub mod phones;
pub mod presets;
pub mod prompts;
pub mod queues;
pub mod recordings;
pub mod ring_groups;
pub mod settings;
pub mod sip_guard;
pub mod stats;
pub mod telemetry;
pub mod tenant;
pub mod time_conditions;
pub mod transcription_api;
pub mod trunks;
pub mod turn;
pub mod users;
pub mod voicemail;
pub mod voicemail_config;

/// Version of the TalkOps build, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
