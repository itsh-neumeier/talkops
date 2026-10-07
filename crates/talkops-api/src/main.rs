use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use talkops_api::backup::{self, BackupConfig};
use talkops_api::config::{Config, Storage};
use talkops_api::fsxml::sofia::ProfileSettings;
use talkops_api::voicemail::VmContext;
use talkops_api::{AppState, MediaPaths, app};
use talkops_core::crypto::SecretBox;
use talkops_core::presets::PresetCatalog;
use talkops_provisioning::PhoneCatalog;
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
    Serve(Box<Config>),
    /// Apply pending database migrations and exit.
    Migrate {
        #[arg(long, env = "TALKOPS_DATABASE_URL", hide_env_values = true)]
        database_url: String,
    },
    /// Write a backup archive (database and data volumes) and exit.
    Backup {
        #[command(flatten)]
        storage: Storage,
        /// Leave call recordings out.
        #[arg(long)]
        without_recordings: bool,
    },
    /// Restore a backup archive. Replaces the database and the data
    /// volumes: stop the server and the media worker first.
    Restore {
        #[command(flatten)]
        storage: Storage,
        /// The archive (`talkops-YYYYMMDD-HHMMSS.tar.gz`).
        archive: std::path::PathBuf,
        /// Confirm that all current data is replaced.
        #[arg(long)]
        yes: bool,
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
        Command::Serve(config) => serve(*config).await,
        Command::Migrate { database_url } => {
            let pool = talkops_core::db::connect_lazy(&database_url, 1)?;
            talkops_core::db::migrate(&pool).await?;
            println!("migrations applied");
            Ok(())
        }
        Command::Backup {
            storage,
            without_recordings,
        } => {
            let cfg = BackupConfig::from_storage(&storage);
            let pool = talkops_core::db::connect_lazy(&storage.database_url, 2)?;
            let name = backup::create(&cfg, &pool, !without_recordings).await?;
            println!("{}", cfg.dir.join(name).display());
            Ok(())
        }
        Command::Restore {
            storage,
            archive,
            yes,
        } => {
            if !yes {
                anyhow::bail!(
                    "this replaces the database and all data volumes with {}; \
                     stop TalkOps first and run again with --yes",
                    archive.display()
                );
            }
            let cfg = BackupConfig::from_storage(&storage);
            let pool = talkops_core::db::connect_lazy(&storage.database_url, 1)?;
            let manifest = backup::restore(&cfg, &pool, &archive).await?;
            println!(
                "restored backup of {} (TalkOps {}), volumes: {}",
                manifest.created_at,
                manifest.talkops_version,
                manifest.volumes.join(", ")
            );
            Ok(())
        }
        Command::Healthcheck { port } => healthcheck(port).await,
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    talkops_core::telemetry::init(config.log_format);
    tracing::info!(version = talkops_core::VERSION, listen = %config.listen, "TalkOps starting");

    let db =
        talkops_core::db::connect_lazy(&config.storage.database_url, config.db_max_connections)?;
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

    let secrets = SecretBox::from_hex(&config.secret_key).context("invalid TALKOPS_SECRET_KEY")?;
    let catalog = PresetCatalog::load_dir(&config.presets_dir).with_context(|| {
        format!(
            "cannot load trunk presets from {}",
            config.presets_dir.display()
        )
    })?;
    tracing::info!(presets = catalog.len(), "trunk presets loaded");
    let phone_catalog = PhoneCatalog::load_dir(&config.phones_dir).with_context(|| {
        format!(
            "cannot load phone models from {}",
            config.phones_dir.display()
        )
    })?;

    let profile = ProfileSettings {
        sip_ip: config.sip_ip.clone(),
        internal_port: config.sip_port,
        external_port: config.sip_trunk_port,
        external_ip: String::new(),
    };
    let state = AppState::new(
        db.clone(),
        secrets,
        catalog,
        phone_catalog,
        config.storage.provisioning_dir.clone(),
        profile,
        &config.xmlcurl_password,
    )
    .with_media(MediaPaths {
        voicemail: config.storage.voicemail_dir.clone(),
        sounds: config.storage.sounds_dir.clone(),
        recordings: config.storage.recordings_dir.clone(),
        snapshots: config.storage.snapshots_dir.clone(),
    })
    .with_outbound_socket(&config.esl_outbound_listen)
    .with_sip_ws(&config.sip_ws_url)
    .with_metrics_token(config.metrics_token.as_deref())
    .with_backup(BackupConfig::from_storage(&config.storage));
    let queue_sync = talkops_api::callcenter::spawn(db.clone(), state.telephony.esl.clone());
    let state = state.with_queue_sync(queue_sync);
    state
        .telephony
        .esl
        .spawn_supervisor(config.esl_addr.clone(), config.esl_password.clone());
    state
        .telephony
        .spawn_poller(Duration::from_secs(10), db.clone());
    let outbound = tokio::net::TcpListener::bind(&config.esl_outbound_listen)
        .await
        .with_context(|| format!("cannot bind {}", config.esl_outbound_listen))?;
    let vm_ctx = VmContext::from(&state);
    tokio::spawn(talkops_esl::outbound::serve(outbound, move |session| {
        talkops_api::voicemail::handle(session, vm_ctx.clone())
    }));
    talkops_api::mailer::spawn(state.db.clone(), state.secrets.clone(), state.media.clone());
    talkops_api::retention::spawn(db.clone(), state.media.clone());
    if let Some(cfg) = &state.backup {
        backup::spawn(db.clone(), cfg.clone());
    }
    talkops_api::ldap::spawn_sync(state.db.clone(), state.secrets.clone());
    talkops_api::doors::spawn_listeners(talkops_api::doors::DoorCtx::from(&state));
    spawn_session_cleanup(db);
    let router = app(state, Some(&config.web_dir));

    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("cannot bind {}", config.listen))?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("TalkOps stopped");
    Ok(())
}

/// Deletes expired sessions once an hour.
fn spawn_session_cleanup(db: sqlx::PgPool) {
    tokio::spawn(async move {
        loop {
            match talkops_core::users::purge_expired_sessions(&db).await {
                Ok(n) if n > 0 => tracing::debug!(purged = n, "expired sessions removed"),
                Ok(_) => {}
                Err(err) => tracing::warn!(error = %err, "session cleanup failed"),
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
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
