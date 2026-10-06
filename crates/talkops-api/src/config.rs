//! Runtime configuration, read from command-line flags or `TALKOPS_*` env vars.

use std::net::SocketAddr;
use std::path::PathBuf;

use talkops_core::telemetry::LogFormat;

#[derive(Debug, Clone, clap::Args)]
pub struct Config {
    /// Address the HTTP server binds to.
    #[arg(long, env = "TALKOPS_LISTEN", default_value = "0.0.0.0:8080")]
    pub listen: SocketAddr,

    /// Postgres connection string.
    #[arg(long, env = "TALKOPS_DATABASE_URL", hide_env_values = true)]
    pub database_url: String,

    #[arg(long, env = "TALKOPS_DB_MAX_CONNECTIONS", default_value_t = 10)]
    pub db_max_connections: u32,

    /// Apply pending database migrations on startup.
    #[arg(long, env = "TALKOPS_AUTO_MIGRATE", default_value_t = true, action = clap::ArgAction::Set)]
    pub auto_migrate: bool,

    /// 256-bit key (64 hex characters) encrypting SIP and trunk credentials.
    #[arg(long, env = "TALKOPS_SECRET_KEY", hide_env_values = true)]
    pub secret_key: String,

    /// Directory with the built web UI. Not served if it does not exist.
    #[arg(
        long,
        env = "TALKOPS_WEB_DIR",
        default_value = "/usr/share/talkops/web"
    )]
    pub web_dir: PathBuf,

    /// Directory with trunk presets (`*.yaml`).
    #[arg(
        long,
        env = "TALKOPS_PRESETS_DIR",
        default_value = "/usr/share/talkops/presets/trunks"
    )]
    pub presets_dir: PathBuf,

    /// Directory with the phone model catalog (`*.yaml`).
    #[arg(
        long,
        env = "TALKOPS_PHONES_DIR",
        default_value = "/usr/share/talkops/presets/phones"
    )]
    pub phones_dir: PathBuf,

    /// Writable directory for provisioning data (firmware images).
    #[arg(
        long,
        env = "TALKOPS_PROVISIONING_DIR",
        default_value = "/var/lib/talkops/provisioning"
    )]
    pub provisioning_dir: PathBuf,

    /// Shared voicemail volume (messages, greetings).
    #[arg(
        long,
        env = "TALKOPS_VOICEMAIL_DIR",
        default_value = "/var/lib/talkops/voicemail"
    )]
    pub voicemail_dir: PathBuf,

    /// Shared recordings volume (call recordings).
    #[arg(
        long,
        env = "TALKOPS_RECORDINGS_DIR",
        default_value = "/var/lib/talkops/recordings"
    )]
    pub recordings_dir: PathBuf,

    /// Door station snapshots.
    #[arg(
        long,
        env = "TALKOPS_SNAPSHOTS_DIR",
        default_value = "/var/lib/talkops/snapshots"
    )]
    pub snapshots_dir: PathBuf,

    /// Shared sounds volume (system prompts rendered by the media worker).
    #[arg(
        long,
        env = "TALKOPS_SOUNDS_DIR",
        default_value = "/var/lib/talkops/sounds"
    )]
    pub sounds_dir: PathBuf,

    /// Address of the outbound Event Socket server FreeSWITCH hands
    /// interactive calls (voicemail) to.
    #[arg(
        long,
        env = "TALKOPS_ESL_OUTBOUND_LISTEN",
        default_value = "127.0.0.1:8084"
    )]
    pub esl_outbound_listen: String,

    /// FreeSWITCH Event Socket address.
    #[arg(long, env = "TALKOPS_ESL_ADDR", default_value = "127.0.0.1:8021")]
    pub esl_addr: String,

    #[arg(long, env = "TALKOPS_ESL_PASSWORD", hide_env_values = true)]
    pub esl_password: String,

    /// Password FreeSWITCH's mod_xml_curl and mod_xml_cdr use (HTTP basic auth, user `talkops`).
    #[arg(long, env = "TALKOPS_XMLCURL_PASSWORD", hide_env_values = true)]
    pub xmlcurl_password: String,

    /// IP the SIP profiles bind to; empty = FreeSWITCH's detected local IPv4.
    #[arg(long, env = "TALKOPS_SIP_IP", default_value = "")]
    pub sip_ip: String,

    /// SIP port for phones (internal profile).
    #[arg(long, env = "TALKOPS_SIP_PORT", default_value_t = 5060)]
    pub sip_port: u16,

    /// SIP port for trunks (external profile).
    #[arg(long, env = "TALKOPS_SIP_TRUNK_PORT", default_value_t = 5080)]
    pub sip_trunk_port: u16,

    #[arg(long, env = "TALKOPS_LOG_FORMAT", default_value = "pretty")]
    pub log_format: LogFormat,
}
