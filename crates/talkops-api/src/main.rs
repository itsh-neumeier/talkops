use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use talkops_api::config::Config;
use talkops_api::esl::EslHandle;
use talkops_api::{AppState, app};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Parser)]
#[command(name = "talkops", version, about = "TalkOps PBX control plane")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the HTTP server and FreeSWITCH control plane.
    Serve(Config),
    /// Apply pending database migrations and exit.
    Migrate {
        #[arg(long, env = "TALKOPS_DATABASE_URL", hide_env_values = true)]
        database_url: String,
    },
    /// Probe the local server's readiness endpoint (container healthcheck).
    Healthcheck {
        /// Port of the local server; defaults to the port of TALKOPS_LISTEN.
        #[arg(long)]
        port: Option<u16>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve(config) => serve(config).await,
        Command::Migrate { database_url } => {
            let pool = talkops_core::db::connect_lazy(&database_url, 1)?;
            talkops_core::db::migrate(&pool).await?;
            println!("migrations applied");
            Ok(())
        }
        Command::Healthcheck { port } => healthcheck(port).await,
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    talkops_core::telemetry::init(config.log_format);
    tracing::info!(version = talkops_core::VERSION, listen = %config.listen, "TalkOps starting");

    let db = talkops_core::db::connect_lazy(&config.database_url, config.db_max_connections)?;
    if config.auto_migrate {
        let mut delay = Duration::from_secs(1);
        loop {
            match talkops_core::db::migrate(&db).await {
                Ok(()) => break,
                Err(err) => {
                    tracing::warn!(error = %err, retry_in = ?delay, "migration failed, retrying");
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(Duration::from_secs(30));
                }
            }
        }
        tracing::info!("database schema up to date");
    }

    let esl = EslHandle::default();
    esl.spawn_supervisor(config.esl_addr.clone(), config.esl_password.clone());

    let state = AppState {
        db,
        esl,
        xmlcurl_password: Arc::from(config.xmlcurl_password.as_str()),
    };
    let router = app(state, Some(&config.web_dir));

    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("cannot bind {}", config.listen))?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("TalkOps stopped");
    Ok(())
}

/// Minimal HTTP/1.0 GET against 127.0.0.1 so the runtime image needs no curl.
async fn healthcheck(port: Option<u16>) -> anyhow::Result<()> {
    let port = match port {
        Some(p) => p,
        None => std::env::var("TALKOPS_LISTEN")
            .ok()
            .and_then(|l| l.parse::<std::net::SocketAddr>().ok())
            .map_or(8080, |a| a.port()),
    };
    let probe = async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
        stream
            .write_all(b"GET /readyz HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        anyhow::Ok(response)
    };
    let response = tokio::time::timeout(Duration::from_secs(5), probe)
        .await
        .context("healthcheck timed out")??;
    let status_line = response.lines().next().unwrap_or_default();
    anyhow::ensure!(status_line.contains(" 200 "), "not ready: {status_line}");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = term.recv() => {},
        }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
}
