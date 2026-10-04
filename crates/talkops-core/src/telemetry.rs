//! Logging setup shared by all binaries.

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable output for development.
    Pretty,
    /// One JSON object per line for log shippers.
    Json,
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "pretty" | "text" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unknown log format `{other}` (expected `pretty` or `json`)"
            )),
        }
    }
}

/// Installs the global tracing subscriber. The filter is read from `RUST_LOG`
/// and defaults to `info`.
pub fn init(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let registry = tracing_subscriber::registry().with(filter);
    match format {
        LogFormat::Pretty => registry.with(fmt::layer()).init(),
        LogFormat::Json => registry
            .with(fmt::layer().json().flatten_event(true))
            .init(),
    }
}
