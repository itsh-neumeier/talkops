//! Media worker: claims TTS and transcription jobs from the Postgres job queue.
//!
//! On start it renders missing system prompts with Piper; afterwards it runs
//! TTS greeting jobs (transcription follows in phase 5). Unknown job kinds are
//! never claimed.

mod tts;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use talkops_core::jobs::{self, Job};
use talkops_core::telemetry::{self, LogFormat};
use talkops_core::{ivr, voicemail};

use crate::tts::Piper;

/// Job kinds this worker can execute.
const SUPPORTED_KINDS: &[&str] = &[voicemail::JOB_TTS_GREETING, ivr::JOB_TTS_IVR];

/// Running jobs without a heartbeat for this long are handed to another worker.
const LEASE_TIMEOUT_SECS: i64 = 30 * 60;

#[derive(Debug, Parser)]
#[command(name = "talkops-media-worker", version, about)]
struct Args {
    #[arg(long, env = "TALKOPS_DATABASE_URL", hide_env_values = true)]
    database_url: String,

    /// Unique name of this worker instance, recorded on claimed jobs.
    #[arg(long, env = "TALKOPS_WORKER_ID", default_value_t = default_worker_id())]
    worker_id: String,

    /// Fallback polling interval in seconds (workers are also woken via NOTIFY).
    #[arg(long, env = "TALKOPS_WORKER_POLL_SECS", default_value_t = 15)]
    poll_secs: u64,

    /// File touched after every successful loop iteration; used as liveness probe.
    #[arg(
        long,
        env = "TALKOPS_WORKER_HEARTBEAT_FILE",
        default_value = "/tmp/talkops-worker.alive"
    )]
    heartbeat_file: PathBuf,

    /// Piper binary.
    #[arg(long, env = "TALKOPS_PIPER_BIN", default_value = "/opt/piper/piper")]
    piper_bin: PathBuf,

    /// Directory with Piper voices (`<voice>.onnx` + `.onnx.json`).
    #[arg(
        long,
        env = "TALKOPS_VOICES_DIR",
        default_value = "/usr/share/talkops/voices"
    )]
    voices_dir: PathBuf,

    /// Shared sounds volume (system prompts).
    #[arg(
        long,
        env = "TALKOPS_SOUNDS_DIR",
        default_value = "/var/lib/talkops/sounds"
    )]
    sounds_dir: PathBuf,

    /// Shared voicemail volume (greetings).
    #[arg(
        long,
        env = "TALKOPS_VOICEMAIL_DIR",
        default_value = "/var/lib/talkops/voicemail"
    )]
    voicemail_dir: PathBuf,

    #[arg(long, env = "TALKOPS_LOG_FORMAT", default_value = "pretty")]
    log_format: LogFormat,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Exit 0 if the heartbeat file was touched recently (container healthcheck).
    Healthcheck {
        #[arg(long, default_value_t = 120)]
        max_age_secs: u64,
    },
}

fn default_worker_id() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| format!("worker-{}", std::process::id()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(Command::Healthcheck { max_age_secs }) = args.command {
        return healthcheck(&args.heartbeat_file, Duration::from_secs(max_age_secs));
    }
    telemetry::init(args.log_format);
    tracing::info!(version = talkops_core::VERSION, worker_id = %args.worker_id, "media worker starting");

    let pool = talkops_core::db::connect_lazy(&args.database_url, 4)?;
    let mut listener = loop {
        match PgListener::connect_with(&pool).await {
            Ok(mut l) => {
                l.listen(jobs::NOTIFY_CHANNEL).await?;
                break l;
            }
            Err(err) => {
                tracing::warn!(error = %err, "database not reachable, retrying in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    };

    let ctx = Ctx {
        pool: pool.clone(),
        piper: Piper {
            bin: args.piper_bin.clone(),
            voices_dir: args.voices_dir.clone(),
        },
        voicemail_dir: args.voicemail_dir.clone(),
        sounds_dir: args.sounds_dir.clone(),
    };
    match tts::ensure_prompts(&ctx.piper, &args.sounds_dir).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(count = n, "system prompts rendered"),
        Err(err) => tracing::error!(error = %err, "rendering system prompts failed"),
    }

    let poll = Duration::from_secs(args.poll_secs);
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        if let Err(err) = drain_queue(&ctx, &args.worker_id).await {
            tracing::error!(error = %err, "job loop error");
        } else {
            touch(&args.heartbeat_file);
        }
        tokio::select! {
            _ = &mut shutdown => break,
            notification = tokio::time::timeout(poll, listener.recv()) => {
                if let Ok(Err(err)) = notification {
                    tracing::warn!(error = %err, "LISTEN connection error");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
    tracing::info!("media worker stopped");
    Ok(())
}

/// What job handlers need.
struct Ctx {
    pool: PgPool,
    piper: Piper,
    voicemail_dir: PathBuf,
    sounds_dir: PathBuf,
}

async fn drain_queue(ctx: &Ctx, worker_id: &str) -> anyhow::Result<()> {
    let pool = &ctx.pool;
    let recovered = jobs::requeue_stale(pool, LEASE_TIMEOUT_SECS).await?;
    if recovered > 0 {
        tracing::warn!(recovered, "re-queued jobs with expired lease");
    }
    if SUPPORTED_KINDS.is_empty() {
        return Ok(());
    }
    while let Some(job) = jobs::claim(pool, worker_id, SUPPORTED_KINDS).await? {
        let id = job.id;
        match run(ctx, &job).await {
            Ok(()) => jobs::complete(pool, id).await?,
            Err(err) => {
                let status = jobs::fail(pool, id, &format!("{err:#}")).await?;
                tracing::warn!(job = %id, kind = %job.kind, ?status, error = %err, "job failed");
            }
        }
    }
    Ok(())
}

async fn run(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    match job.kind.as_str() {
        voicemail::JOB_TTS_GREETING => tts_greeting(ctx, job).await,
        ivr::JOB_TTS_IVR => tts_ivr(ctx, job).await,
        other => anyhow::bail!("no handler for job kind `{other}`"),
    }
}

/// Renders a voicemail greeting; the box status tells the UI the outcome.
async fn tts_greeting(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    let p = TtsPayload::parse(job)?;
    let path = ctx.voicemail_dir.join(&p.file);
    let result = ctx
        .piper
        .render(&p.language, &[(p.text.clone(), path)])
        .await;
    voicemail::set_greeting_status(&ctx.pool, p.owner, &p.text, result.is_ok()).await?;
    result
}

fn touch(path: &Path) {
    if let Err(err) = std::fs::write(path, b"ok") {
        tracing::warn!(error = %err, path = %path.display(), "cannot write heartbeat file");
    }
}

fn healthcheck(path: &Path, max_age: Duration) -> anyhow::Result<()> {
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .with_context(|| format!("no heartbeat at {}", path.display()))?;
    let age = modified.elapsed().unwrap_or_default();
    anyhow::ensure!(age <= max_age, "heartbeat is {}s old", age.as_secs());
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

/// Payload of the TTS jobs: text to render into a file below a media volume.
#[derive(serde::Deserialize)]
struct TtsPayload {
    #[serde(alias = "extension_id", alias = "menu_id")]
    owner: uuid::Uuid,
    text: String,
    language: String,
    file: String,
}

impl TtsPayload {
    fn parse(job: &Job) -> anyhow::Result<Self> {
        let p: Self = serde_json::from_value(job.payload.clone()).context("invalid payload")?;
        anyhow::ensure!(
            !p.file
                .split('/')
                .any(|part| part == ".." || part.is_empty()),
            "invalid output path"
        );
        Ok(p)
    }
}

/// Renders an IVR greeting into the sounds volume.
async fn tts_ivr(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    let p = TtsPayload::parse(job)?;
    let result = ctx
        .piper
        .render(
            &p.language,
            &[(p.text.clone(), ctx.sounds_dir.join(&p.file))],
        )
        .await;
    ivr::set_greeting_status(&ctx.pool, p.owner, &p.text, result.is_ok()).await?;
    result
}
