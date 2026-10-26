//! Core domain types, database access and the Postgres-backed job queue shared
//! by all TalkOps services.

pub mod attendant;
pub mod audio;
pub mod audit;
pub mod audit_export;
pub mod backup_targets;
pub mod backups;
pub mod blocking;
pub mod cdr;
pub mod contact_import;
pub mod crypto;
pub mod db;
pub mod dial_permissions;
pub mod dialing;
pub mod doors;
pub mod error;
pub mod extensions;
pub mod fax;
pub mod fax_adapter;
pub mod geo;
pub mod holidays;
pub mod identity;

pub mod jobs;
pub mod live_transcripts;
pub mod mail;
pub mod mail_render;
pub mod mail_templates;
pub mod mfa;
pub mod notifications;
pub mod numbering;
pub mod phone_provisioning;
pub mod phones;
pub mod portainer;
pub mod presets;
pub mod prompts;
pub mod queues;
pub mod recordings;
pub mod ring_groups;
pub mod roles;
pub mod settings;
pub mod sip_guard;
pub mod speech;
pub mod stats;
pub mod storage_targets;
pub mod system_load;
pub mod telemetry;
pub mod tenant;
pub mod time_conditions;
pub mod transcription_api;
pub mod transcription_jobs;
pub mod transcription_profiles;
pub mod trunks;
pub mod turn;
pub mod unifi;
pub mod unifi_changes;
pub mod users;
pub mod voicemail;
pub mod voicemail_config;
pub mod wifi_survey;

/// Version of the TalkOps build, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
