//! Media worker: claims TTS and transcription jobs from the Postgres job queue.
//!
//! On start it renders missing system prompts with Piper; afterwards it runs
//! TTS jobs and transcriptions (whisper.cpp) in two independent lanes, so a
//! long transcription never delays a greeting. Unknown job kinds are never
//! claimed.

mod api;
mod transcribe;
mod tts;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use talkops_core::crypto::SecretBox;
use talkops_core::jobs::{self, Job};
use talkops_core::recordings::{self, Source};
use talkops_core::settings::TranscriptionEngine;
use talkops_core::telemetry::{self, LogFormat};
use talkops_core::{audio, settings, transcription_api, voicemail};
use tokio::sync::Notify;

use crate::transcribe::Whisper;
use crate::tts::Piper;

/// Job kinds per lane; each lane works through its jobs one at a time.
const LANES: &[&[&str]] = &[
    &[audio::JOB_TTS_CLIP, voicemail::JOB_TTS_GREETING],
    &[recordings::JOB_TRANSCRIBE],
];

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

    /// Shared recordings volume (call recordings).
    #[arg(
        long,
        env = "TALKOPS_RECORDINGS_DIR",
        default_value = "/var/lib/talkops/recordings"
    )]
    recordings_dir: PathBuf,

    /// whisper.cpp command line binary.
    #[arg(
        long,
        env = "TALKOPS_WHISPER_BIN",
        default_value = "/opt/whisper/whisper-cli"
    )]
    whisper_bin: PathBuf,

    /// Whisper model (`ggml-<name>.bin`); `base`, `small`, `medium(-q5_0)` and
    /// `large-v3-turbo(-q5_0, -q8_0)` are downloaded automatically.
    #[arg(
        long,
        env = "TALKOPS_WHISPER_MODEL",
        default_value = "large-v3-turbo-q5_0"
    )]
    whisper_model: String,

    /// Transcribe only detected speech (Silero VAD); avoids invented text in
    /// pauses and hold music.
    #[arg(long, env = "TALKOPS_WHISPER_VAD", default_value_t = true, action = clap::ArgAction::Set)]
    whisper_vad: bool,

    /// CPU threads per transcription (default: up to 4).
    #[arg(long, env = "TALKOPS_WHISPER_THREADS")]
    whisper_threads: Option<usize>,

    /// Directory for downloaded models.
    #[arg(
        long,
        env = "TALKOPS_MODELS_DIR",
        default_value = "/var/lib/talkops/models"
    )]
    models_dir: PathBuf,

    /// Key that decrypts the transcription API key (same as talkops-api).
    #[arg(long, env = "TALKOPS_SECRET_KEY", hide_env_values = true)]
    secret_key: Option<String>,

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
    /// Transcribes a WAV file and prints the text, e.g. to compare quality
    /// levels: `talkops-media-worker transcribe --quality best call.wav`.
    Transcribe {
        file: PathBuf,
        /// `fast`, `accurate`, `best` or `german`.
        #[arg(long, default_value = "fast")]
        quality: String,
        #[arg(long, default_value = "de")]
        language: String,
        /// Names and terms, comma-separated.
        #[arg(long, default_value = "")]
        vocabulary: String,
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
    let threads = args.whisper_threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get().min(4))
            .unwrap_or(2)
    });
    let whisper = Whisper {
        bin: args.whisper_bin.clone(),
        models_dir: args.models_dir.clone(),
        model: args.whisper_model.clone(),
        threads: threads.max(1),
        vad: args.whisper_vad,
    };
    if let Some(Command::Transcribe {
        file,
        quality,
        language,
        vocabulary,
    }) = &args.command
    {
        let opts = transcribe::Options {
            quality: settings::TranscriptionQuality::parse(quality)
                .context("quality must be fast, accurate, best or german")?,
            vocabulary: vocabulary.clone(),
        };
        let started = std::time::Instant::now();
        for seg in whisper.transcribe(file, language, &opts).await? {
            println!(
                "[{:7.2}–{:7.2}] {:>6} {}",
                seg.start, seg.end, seg.speaker, seg.text
            );
        }
        eprintln!("{:.1} s", started.elapsed().as_secs_f32());
        return Ok(());
    }
    tracing::info!(version = talkops_core::VERSION, worker_id = %args.worker_id, "media worker starting");

    let pool = talkops_core::db::connect_lazy(&args.database_url, 4)?;
    let mut listener = loop {
        match PgListener::connect_with(&pool).await {
            Ok(mut l) => {
                l.listen(jobs::NOTIFY_CHANNEL).await?;
                // Restart requests from the web UI (talkops-api diagnostics).
                l.listen(CONTROL_CHANNEL).await?;
                break l;
            }
            Err(err) => {
                tracing::warn!(error = %err, "database not reachable, retrying in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    };

    let ctx = std::sync::Arc::new(Ctx {
        pool: pool.clone(),
        piper: Piper {
            bin: args.piper_bin.clone(),
            voices_dir: args.voices_dir.clone(),
        },
        whisper,
        secrets: match args.secret_key.as_deref().filter(|k| !k.is_empty()) {
            Some(k) => Some(SecretBox::from_hex(k).context("invalid TALKOPS_SECRET_KEY")?),
            None => None,
        },
        voicemail_dir: args.voicemail_dir.clone(),
        sounds_dir: args.sounds_dir.clone(),
        recordings_dir: args.recordings_dir.clone(),
    });
    match tts::ensure_prompts(&ctx.piper, &args.sounds_dir).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(count = n, "system prompts rendered"),
        Err(err) => tracing::error!(error = %err, "rendering system prompts failed"),
    }

    let wake = std::sync::Arc::new(Notify::new());
    let poll = Duration::from_secs(args.poll_secs);
    for kinds in LANES {
        let (ctx, wake, worker_id) = (ctx.clone(), wake.clone(), args.worker_id.clone());
        let heartbeat = args.heartbeat_file.clone();
        tokio::spawn(async move {
            loop {
                let woken = wake.notified();
                if let Err(err) = drain_queue(&ctx, &worker_id, kinds).await {
                    tracing::error!(error = %err, "job loop error");
                } else if kinds == &LANES[0] {
                    // The fast lane proves liveness; transcriptions may run long.
                    touch(&heartbeat);
                }
                tokio::select! {
                    _ = woken => {}
                    _ = tokio::time::sleep(poll) => {}
                }
            }
        });
    }

    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            notification = listener.recv() => match notification {
                Ok(n) if n.channel() == CONTROL_CHANNEL && n.payload() == "restart" => {
                    // Docker's restart policy starts the worker again.
                    tracing::warn!("restart requested");
                    break;
                }
                Ok(_) => wake.notify_waiters(),
                Err(err) => {
                    tracing::warn!(error = %err, "LISTEN connection error");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            },
        }
    }
    tracing::info!("media worker stopped");
    Ok(())
}

/// Postgres channel for control messages (same as talkops-api).
const CONTROL_CHANNEL: &str = "talkops_control";

/// What job handlers need.
struct Ctx {
    pool: PgPool,
    piper: Piper,
    whisper: Whisper,
    /// Decrypts the transcription API key; `None` without TALKOPS_SECRET_KEY.
    secrets: Option<SecretBox>,
    voicemail_dir: PathBuf,
    sounds_dir: PathBuf,
    recordings_dir: PathBuf,
}

async fn drain_queue(ctx: &Ctx, worker_id: &str, kinds: &[&str]) -> anyhow::Result<()> {
    let pool = &ctx.pool;
    let recovered = jobs::requeue_stale(pool, LEASE_TIMEOUT_SECS).await?;
    if recovered > 0 {
        tracing::warn!(recovered, "re-queued jobs with expired lease");
    }
    while let Some(job) = jobs::claim(pool, worker_id, kinds).await? {
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
        audio::JOB_TTS_CLIP => tts_clip(ctx, job).await,
        voicemail::JOB_TTS_GREETING => tts_greeting(ctx, job).await,
        recordings::JOB_TRANSCRIBE => transcribe(ctx, job).await,
        other => anyhow::bail!("no handler for job kind `{other}`"),
    }
}

/// Renders a generated audio clip; its status tells the UI the outcome.
async fn tts_clip(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    #[derive(serde::Deserialize)]
    struct Payload {
        clip_id: uuid::Uuid,
    }
    let p: Payload = serde_json::from_value(job.payload.clone()).context("invalid payload")?;
    let clip = audio::render_job(&ctx.pool, p.clip_id).await?;
    let path = ctx
        .sounds_dir
        .join(audio::clip_file(clip.tenant_id, p.clip_id));
    let voice = audio::voice(&clip.language, clip.voice);
    let result = ctx
        .piper
        .render_with(voice.model, &[(clip.text.clone(), path.clone())])
        .await;
    let duration = match &result {
        Ok(()) => Some(wav_duration_ms(&path).unwrap_or(0)),
        Err(_) => None,
    };
    audio::set_rendered(&ctx.pool, p.clip_id, duration).await?;
    result
}

/// Length of a WAV file in milliseconds.
fn wav_duration_ms(path: &Path) -> anyhow::Result<i32> {
    let reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let ms = u64::from(reader.duration()) * 1000 / u64::from(spec.sample_rate.max(1));
    Ok(i32::try_from(ms).unwrap_or(i32::MAX))
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
    #[serde(alias = "extension_id")]
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

/// Payload of `transcribe` jobs.
#[derive(serde::Deserialize, serde::Serialize)]
struct TranscribePayload {
    recording_id: Option<uuid::Uuid>,
    voicemail_id: Option<uuid::Uuid>,
    /// 1 = quick first pass (default), 2 = more accurate second pass.
    #[serde(default = "first_pass")]
    pass: u8,
}

fn first_pass() -> u8 {
    1
}

/// Transcribes a call recording or voicemail in up to two passes: a quick
/// one whose transcript is shown (and mailed) right away, then optionally a
/// more accurate one that replaces it. A voicemail's e-mail waits for the
/// first pass and is sent once it is there or has finally failed.
async fn transcribe(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    let p: TranscribePayload =
        serde_json::from_value(job.payload.clone()).context("invalid payload")?;
    let tenant = job.tenant_id;
    let tenant_settings = settings::get(&ctx.pool, tenant).await?;
    let refine = TranscriptionEngine::refine(&tenant_settings);
    let (source, file, language) = match (p.recording_id, p.voicemail_id) {
        (Some(id), None) => {
            let rec = recordings::get(&ctx.pool, tenant, id).await?;
            let lang = tenant_settings.default_language.clone();
            (
                Source::Recording(id),
                ctx.recordings_dir.join(rec.file),
                lang,
            )
        }
        (None, Some(id)) => {
            let msg = voicemail::get_message(&ctx.pool, tenant, id).await?;
            let lang = match voicemail::get_box(&ctx.pool, tenant, msg.extension_id)
                .await?
                .language
            {
                Some(lang) => lang,
                None => tenant_settings.default_language.clone(),
            };
            (
                Source::Voicemail(id),
                ctx.voicemail_dir.join(msg.file),
                lang,
            )
        }
        _ => anyhow::bail!("payload needs recording_id or voicemail_id"),
    };
    let engine = if p.pass >= 2 {
        match refine {
            Some(engine) => engine,
            // Switched off since the first pass.
            None => return Ok(recordings::finalize_transcript(&ctx.pool, source).await?),
        }
    } else {
        TranscriptionEngine::parse(&tenant_settings.transcription_quality).unwrap_or(
            TranscriptionEngine::Local(settings::TranscriptionQuality::Fast),
        )
    };
    let language = talkops_core::prompts::language(&language);
    let started = std::time::Instant::now();
    let result = run_engine(ctx, tenant, engine, &file, language, &tenant_settings).await;
    let last_attempt = job.attempts >= job.max_attempts;
    let finished = match &result {
        Ok((segments, label)) => {
            let is_final = p.pass >= 2 || refine.is_none();
            let meta = recordings::TranscriptMeta {
                engine: label,
                is_final,
            };
            recordings::save_transcript(&ctx.pool, tenant, source, language, segments, meta)
                .await?;
            tracing::info!(
                ?source,
                pass = p.pass,
                engine = %label,
                segments = segments.len(),
                secs = started.elapsed().as_secs(),
                "transcribed"
            );
            if !is_final {
                let payload = TranscribePayload { pass: 2, ..p };
                let mut next =
                    jobs::NewJob::new(recordings::JOB_TRANSCRIBE, serde_json::to_value(&payload)?);
                next.tenant_id = tenant;
                // After new first passes; one retry, the first transcript stays anyway.
                next.priority = -1;
                next.max_attempts = 2;
                jobs::enqueue(&ctx.pool, next).await?;
            }
            true
        }
        Err(_) if last_attempt && p.pass >= 2 => {
            recordings::finalize_transcript(&ctx.pool, source).await?;
            false
        }
        Err(_) if last_attempt => {
            recordings::transcript_failed(&ctx.pool, source).await?;
            true
        }
        Err(_) => false,
    };
    if let (true, 1, Source::Voicemail(id)) = (finished, p.pass, source) {
        let msg = voicemail::get_message(&ctx.pool, tenant, id).await?;
        let mut conn = ctx.pool.acquire().await?;
        voicemail::enqueue_mail(&mut conn, tenant, &msg).await?;
    }
    result.map(|_| ())
}

/// Runs one transcription; returns the segments and an engine label.
async fn run_engine(
    ctx: &Ctx,
    tenant: talkops_core::tenant::TenantId,
    engine: TranscriptionEngine,
    file: &Path,
    language: &str,
    s: &settings::TenantSettings,
) -> anyhow::Result<(Vec<recordings::Segment>, String)> {
    match engine {
        TranscriptionEngine::Local(quality) => {
            let opts = transcribe::Options {
                quality,
                vocabulary: s.transcription_vocabulary.clone(),
            };
            let segments = ctx.whisper.transcribe(file, language, &opts).await?;
            Ok((
                segments,
                format!("whisper:{}", ctx.whisper.model_name(quality)),
            ))
        }
        TranscriptionEngine::Api => {
            let secrets = ctx.secrets.as_ref().context(
                "TALKOPS_SECRET_KEY is not set for the media worker (needed for the API key)",
            )?;
            let cfg = transcription_api::config(&ctx.pool, tenant, secrets).await?;
            let segments =
                api::transcribe(&cfg, file, language, &s.transcription_vocabulary).await?;
            Ok((segments, format!("api:{}", cfg.model)))
        }
    }
}
