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
    /// Inspect and validate a backup without a running database. Optionally export configuration.
    Inspect {
        archive: std::path::PathBuf,
        #[arg(long, env = "TALKOPS_BACKUP_PASSWORD", hide_env_values = true)]
        backup_password: Option<String>,
        #[arg(long, env = "TALKOPS_BACKUP_PASSWORD_FILE")]
        backup_password_file: Option<std::path::PathBuf>,
        /// New output directory; existing files are never overwritten.
        #[arg(long)]
        output: Option<std::path::PathBuf>,
        #[arg(long, default_value_t = 68_719_476_736)]
        max_bytes: u64,
    },
    /// List or download archives on a backup target, without a database.
    /// The target file is JSON: `{"config": {"kind": "s3", ...}, "path":
    /// "bucket/talkops", "allow_insecure": false, "secret": {...}}`.
    Fetch {
        #[arg(long)]
        target_file: std::path::PathBuf,
        /// List the archives on the target.
        #[arg(long, conflicts_with = "name")]
        list: bool,
        /// Archive to download.
        #[arg(long, requires = "output")]
        name: Option<String>,
        /// Directory for the downloaded archive.
        #[arg(long)]
        output: Option<std::path::PathBuf>,
        /// SFTP: accept the host key of the server on first use.
        #[arg(long)]
        trust_host_key: bool,
        #[arg(long, env = "TALKOPS_RCLONE", default_value = "rclone")]
        rclone: std::path::PathBuf,
        #[arg(long, default_value_t = 68_719_476_736)]
        max_bytes: u64,
    },
    /// Print the host recovery script. No database or secrets are required.
    RecoveryScript,
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
            let _writer_guard = talkops_core::db::operation_guard(&database_url, false).await?;
            let pool = talkops_core::db::connect_lazy(&database_url, 1)?;
            talkops_core::db::migrate(&pool).await?;
            println!("migrations applied");
            Ok(())
        }
        Command::Backup {
            storage,
            without_recordings,
        } => {
            let cfg = BackupConfig::from_storage(&storage)?;
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
            let cfg = BackupConfig::from_storage(&storage)?;
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
        Command::Inspect {
            archive,
            backup_password,
            backup_password_file,
            output,
            max_bytes,
        } => {
            let password = talkops_api::recovery::password(
                backup_password.as_deref(),
                backup_password_file.as_deref(),
            )?;
            let prepared = talkops_api::recovery::prepare(&archive, password.as_ref(), max_bytes)?;
            if let Some(output) = output {
                prepared.export(&output)?;
            }
            println!("{}", serde_json::to_string_pretty(&prepared.manifest)?);
            Ok(())
        }
        Command::Fetch {
            target_file,
            list,
            name,
            output,
            trust_host_key,
            rclone,
            max_bytes,
        } => {
            fetch(
                &target_file,
                list,
                name,
                output,
                trust_host_key,
                rclone,
                max_bytes,
            )
            .await
        }
        Command::RecoveryScript => {
            print!("{}", include_str!("../../../scripts/recover.sh"));
            Ok(())
        }
        Command::Healthcheck { port } => healthcheck(port).await,
    }
}

/// Loads the number geolocation data of the call map. Without the directory
/// the map has no locations; invalid data stops the start.
fn load_geo(dir: &std::path::Path) -> anyhow::Result<talkops_core::geo::GeoIndex> {
    if !dir.is_dir() {
        tracing::warn!(dir = %dir.display(), "no geo data, the call map shows no locations");
        return Ok(talkops_core::geo::GeoIndex::default());
    }
    let started = std::time::Instant::now();
    let geo = talkops_core::geo::GeoIndex::load_dir(dir)
        .with_context(|| format!("cannot load geo data from {}", dir.display()))?;
    tracing::info!(
        areas = geo.len(),
        prefixes = geo.prefix_count(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "geo data loaded"
    );
    Ok(geo)
}

async fn serve(config: Config) -> anyhow::Result<()> {
    talkops_core::telemetry::init(config.log_format);
    tracing::info!(version = talkops_core::VERSION, listen = %config.listen, "TalkOps starting");

    let db =
        talkops_core::db::connect_lazy(&config.storage.database_url, config.db_max_connections)?;
    let _writer_guard =
        talkops_core::db::operation_guard(&config.storage.database_url, false).await?;
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

    let geo = load_geo(&config.geo_dir)?;

    let profile = ProfileSettings {
        sip_ip: config.sip_ip.clone(),
        internal_port: config.sip_port,
        external_port: config.sip_trunk_port,
        external_ip: String::new(),
    };
    if config
        .setup_token
        .as_ref()
        .is_some_and(|t| !t.is_empty() && t.len() < 32)
    {
        anyhow::bail!("TALKOPS_SETUP_TOKEN must have at least 32 characters");
    }
    let mut backup_cfg = BackupConfig::from_storage(&config.storage)?;
    backup_cfg.deployment_key = Some(config.secret_key.clone().into());
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
        fax: config.storage.fax_dir.clone(),
        voicemail: config.storage.voicemail_dir.clone(),
        sounds: config.storage.sounds_dir.clone(),
        recordings: config.storage.recordings_dir.clone(),
        snapshots: config.storage.snapshots_dir.clone(),
    })
    .with_outbound_socket(&config.esl_outbound_listen)
    .with_sip_ws(&config.sip_ws_url)
    .with_metrics_token(config.metrics_token.as_deref())
    .with_backup(backup_cfg)
    .with_setup_token(config.setup_token.as_deref())
    .with_fs_peers(config.fs_peers.clone())
    .with_geo(geo)
    .with_turn(
        config.turn_urls.clone(),
        config.turn_secret.clone(),
        config.turn_relay_only,
    );
    if let Some(turn) = &state.turn {
        tracing::info!(urls = ?turn.urls, relay_only = turn.relay_only, "TURN enabled for softphones");
    }
    let queue_sync = talkops_api::callcenter::spawn(db.clone(), state.telephony.esl.clone());
    let state = state.with_queue_sync(queue_sync).with_diagnostics(
        talkops_api::diagnostics::Diagnostics::new(
            config.esl_addr.clone(),
            config.esl_password.clone(),
        ),
    );
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
    let fax_state = state.clone();
    tokio::spawn(talkops_esl::outbound::serve(outbound, move |session| {
        let state = fax_state.clone();
        let ctx = vm_ctx.clone();
        async move {
            if session
                .var("talkops_app")
                .is_some_and(|v| v.starts_with("fax_"))
            {
                talkops_api::fax::handle(session, state).await;
            } else if session.var("talkops_app") == Some(talkops_api::wifi_survey::APP) {
                talkops_api::wifi_survey::handle(session, state).await;
            } else {
                talkops_api::voicemail::handle(session, ctx).await;
            }
        }
    }));
    talkops_api::fax::spawn(state.clone());
    talkops_api::wifi_survey::spawn(state.clone());
    talkops_api::mailer::spawn(state.db.clone(), state.secrets.clone(), state.media.clone());
    talkops_api::notifier::spawn(state.db.clone(), state.secrets.clone(), state.media.clone());
    talkops_api::retention::spawn(db.clone(), state.media.clone());
    talkops_api::cdr_spool::spawn(state.clone(), config.storage.cdr_spool_dir.clone());
    match talkops_core::attendant::adopt_legacy_greetings(&db, &state.media.sounds).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(count = n, "moved voice menu greetings to audio clips"),
        Err(err) => tracing::warn!(error = %err, "cannot move voice menu greetings"),
    }
    talkops_api::sip_guard::spawn(db.clone(), state.telephony.esl.clone());
    talkops_api::audit_export::spawn(db.clone(), state.backup.clone(), state.secrets.clone());
    if let Some(cfg) = &state.backup {
        backup::spawn(db.clone(), cfg.clone(), state.secrets.clone());
    }
    talkops_api::ldap::spawn_sync(state.db.clone(), state.secrets.clone());
    talkops_api::doors::spawn_listeners(talkops_api::doors::DoorCtx::from(&state));
    talkops_api::door_dtmf::spawn(
        talkops_api::doors::DoorCtx::from(&state),
        state.telephony.esl.clone(),
    );
    spawn_session_cleanup(db);
    let state = match config.cisco_tftp_dir {
        Some(root) => {
            let state = state.with_cisco_tftp(root);
            talkops_api::cisco_tftp::spawn(state.clone()).await?;
            state
        }
        None => state,
    };
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

async fn fetch(
    target_file: &std::path::Path,
    list: bool,
    name: Option<String>,
    output: Option<std::path::PathBuf>,
    trust_host_key: bool,
    rclone: std::path::PathBuf,
    max_bytes: u64,
) -> anyhow::Result<()> {
    use talkops_core::backup_targets::{TargetConfig, TargetConnection};
    let text = std::fs::read_to_string(target_file)
        .with_context(|| format!("cannot read {}", target_file.display()))?;
    let mut conn: TargetConnection = serde_json::from_str(&text).context("invalid target file")?;
    conn.validate(true)?;
    let rclone = talkops_api::rclone::Rclone::new(rclone);
    if matches!(&conn.config, TargetConfig::Sftp { host_keys, .. } if host_keys.is_empty()) {
        let (keys, print) = rclone.pin_host_key(&conn).await?;
        eprintln!("SFTP host key: {print}");
        if !trust_host_key {
            anyhow::bail!(
                "compare the fingerprint, then add \"host_keys\": \"{keys}\" to the target file or run again with --trust-host-key"
            );
        }
        if let TargetConfig::Sftp { host_keys, .. } = &mut conn.config {
            *host_keys = keys;
        }
    }
    if list || name.is_none() {
        for entry in talkops_api::backup_targets::archives(&rclone, &conn).await? {
            println!("{}\t{}", entry.name, entry.size);
        }
        return Ok(());
    }
    let (Some(name), Some(output)) = (name, output) else {
        anyhow::bail!("--name needs --output");
    };
    let path =
        talkops_api::backup_targets::fetch(&rclone, &conn, &name, &output, max_bytes).await?;
    println!("{}", path.display());
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
