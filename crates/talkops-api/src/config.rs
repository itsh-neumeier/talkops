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

    /// Directory with the built web UI. Not served if it does not exist.
    #[arg(
        long,
        env = "TALKOPS_WEB_DIR",
        default_value = "/usr/share/talkops/web"
    )]
    pub web_dir: PathBuf,

    /// FreeSWITCH Event Socket address.
    #[arg(long, env = "TALKOPS_ESL_ADDR", default_value = "127.0.0.1:8021")]
    pub esl_addr: String,

    #[arg(long, env = "TALKOPS_ESL_PASSWORD", hide_env_values = true)]
    pub esl_password: String,

    /// Password FreeSWITCH's mod_xml_curl uses (HTTP basic auth, user `talkops`).
    #[arg(long, env = "TALKOPS_XMLCURL_PASSWORD", hide_env_values = true)]
    pub xmlcurl_password: String,

    #[arg(long, env = "TALKOPS_LOG_FORMAT", default_value = "pretty")]
    pub log_format: LogFormat,
}
