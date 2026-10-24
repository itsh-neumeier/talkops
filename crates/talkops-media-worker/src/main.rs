//! Media worker: claims TTS and transcription jobs from the Postgres job queue.
//!
//! On start it renders missing system prompts with Piper; afterwards it runs
//! TTS jobs and transcriptions (whisper.cpp) in two independent lanes, so a
//! long transcription never delays a greeting. Unknown job kinds are never
//! claimed.

mod api;
mod fax;
mod live;
mod realtime;
mod resources;
mod transcribe;
mod tts;
mod utterances;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use talkops_core::crypto::SecretBox;
use talkops_core::jobs::{self, Job};
use talkops_core::recordings::{self, Source};
use talkops_core::telemetry::{self, LogFormat};
use talkops_core::transcription_jobs::{self as tjobs, Payload as TranscribePayload};
use talkops_core::transcription_profiles::{self as profiles, Engine, Purpose};
use talkops_core::voicemail_config as vmc;
use talkops_core::{audio, settings, transcription_api, voicemail};
use tokio::sync::Notify;

use crate::transcribe::{Checkpoint, Control, Interrupt, Outcome, Whisper};
use crate::tts::Piper;

/// Job kinds per lane; each lane works through its jobs one at a time.
const LANES: &[&[&str]] = &[
    &[
        audio::JOB_TTS_CLIP,
        voicemail::JOB_TTS_GREETING,
        vmc::JOB_TTS_MESSAGE_INFO,
        vmc::JOB_TTS_PROMPTS,
    ],
    &[recordings::JOB_TRANSCRIBE],
    &[talkops_core::live_transcripts::JOB],
    &[talkops_core::fax::PREPARE, talkops_core::fax::RECEIVE],
    &[talkops_core::wifi_survey::JOB_ANALYSE],
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

    let _writer_guard = talkops_core::db::operation_guard(&args.database_url, false).await?;
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
        worker_id: args.worker_id.clone(),
        streams: realtime::Streams::default(),
        current_job: std::sync::Mutex::new(None),
        coral_backoff: CoralBackoff::default(),
    });
    spawn_resource_reports(ctx.clone());
    match tts::ensure_prompts(&ctx.piper, &args.sounds_dir).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(count = n, "system prompts rendered"),
        Err(err) => tracing::error!(error = %err, "rendering system prompts failed"),
    }
    // Reworded voicemail prompts and other voices (e.g. after a volume reset).
    match tenant_prompts(&ctx, None).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(count = n, "tenant prompts rendered"),
        Err(err) => tracing::warn!(error = %err, "rendering tenant prompts failed"),
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
    /// Holds realtime stream leases (`TALKOPS_WORKER_ID`).
    worker_id: String,
    /// Realtime live previews this worker streams.
    streams: realtime::Streams,
    /// The transcription job that runs now (for the resource report).
    current_job: std::sync::Mutex<Option<uuid::Uuid>>,
    /// Coral connections that failed recently (ADR 0046).
    coral_backoff: CoralBackoff,
}

/// After a failure, a Coral connection is not tried again for 30 s (the
/// backoff of ADR 0024); its local fallback runs at once meanwhile.
#[derive(Debug, Default)]
pub(crate) struct CoralBackoff(
    std::sync::Mutex<std::collections::HashMap<uuid::Uuid, std::time::Instant>>,
);

impl CoralBackoff {
    pub(crate) const WAIT: Duration = Duration::from_secs(30);

    fn active(&self, provider: uuid::Uuid) -> bool {
        self.0
            .lock()
            .ok()
            .and_then(|m| m.get(&provider).copied())
            .is_some_and(|at| at.elapsed() < Self::WAIT)
    }

    fn failed(&self, provider: uuid::Uuid) {
        if let Ok(mut m) = self.0.lock() {
            m.insert(provider, std::time::Instant::now());
        }
    }

    fn succeeded(&self, provider: uuid::Uuid) {
        if let Ok(mut m) = self.0.lock() {
            m.remove(&provider);
        }
    }
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
            Ok(JobEnd::Done) => jobs::complete(pool, id).await?,
            Ok(JobEnd::Paused {
                progress,
                checkpoint,
            }) => {
                tracing::info!(job = %id, progress, "job paused");
                jobs::pause_running(pool, id, progress, checkpoint).await?;
            }
            Ok(JobEnd::Stopped) => {
                tracing::info!(job = %id, "job stopped");
                jobs::cancel_running(pool, id).await?;
                tjobs::after_cancel(pool, job.tenant_id, &job.payload).await?;
            }
            Err(err) => {
                let status = jobs::fail(pool, id, &format!("{err:#}")).await?;
                tracing::warn!(job = %id, kind = %job.kind, ?status, error = %err, "job failed");
            }
        }
    }
    Ok(())
}

/// How a job ended without an error.
enum JobEnd {
    Done,
    /// A user paused the job; `checkpoint` is its resume point.
    Paused {
        progress: f32,
        checkpoint: Option<serde_json::Value>,
    },
    /// A user stopped the job.
    Stopped,
}

async fn run(ctx: &Ctx, job: &Job) -> anyhow::Result<JobEnd> {
    let done = |r: anyhow::Result<()>| r.map(|_| JobEnd::Done);
    match job.kind.as_str() {
        talkops_core::fax::PREPARE | talkops_core::fax::RECEIVE => {
            done(fax::run(&ctx.pool, job).await)
        }
        audio::JOB_TTS_CLIP => done(tts_clip(ctx, job).await),
        voicemail::JOB_TTS_GREETING => done(tts_greeting(ctx, job).await),
        vmc::JOB_TTS_MESSAGE_INFO => done(tts_message_info(ctx, job).await),
        vmc::JOB_TTS_PROMPTS => done(tenant_prompts(ctx, Some(job.tenant_id)).await.map(|_| ())),
        recordings::JOB_TRANSCRIBE => {
            *ctx.current_job.lock().expect("job lock") = Some(job.id);
            let end = transcribe(ctx, job).await;
            *ctx.current_job.lock().expect("job lock") = None;
            end
        }
        talkops_core::live_transcripts::JOB => done(live::run(ctx, job).await),
        talkops_core::wifi_survey::JOB_ANALYSE => done(wifi_survey_analyse(ctx, job).await),
        other => anyhow::bail!("no handler for job kind `{other}`"),
    }
}

/// Rates the roams of a finished Wi-Fi survey (ADR 0041).
async fn wifi_survey_analyse(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    let id: uuid::Uuid = job
        .payload
        .get("survey_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("payload without survey_id"))?
        .parse()?;
    match talkops_core::wifi_survey::analyse_and_store(&ctx.pool, job.tenant_id, id).await {
        Ok(result) => {
            tracing::info!(survey = %id, roams = result.roams.len(), "Wi-Fi survey analysed");
            Ok(())
        }
        // Deleted in the meantime: nothing to do.
        Err(talkops_core::error::CoreError::NotFound) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Reports the resource usage of this worker every 5 seconds.
fn spawn_resource_reports(ctx: std::sync::Arc<Ctx>) {
    tokio::spawn(async move {
        let mut sampler = resources::Sampler::new();
        loop {
            let sample = tokio::task::spawn_blocking(move || {
                let s = sampler.sample();
                (sampler, s)
            })
            .await;
            let Ok((back, s)) = sample else { return };
            sampler = back;
            let status = tjobs::WorkerStatus {
                worker_id: ctx.worker_id.clone(),
                cpu_percent: s.cpu_percent,
                cpus: s.cpus as i32,
                load1: s.load1,
                rss_bytes: s.rss_bytes as i64,
                mem_used_bytes: s.mem_used_bytes as i64,
                mem_total_bytes: s.mem_total_bytes as i64,
                job_id: *ctx.current_job.lock().expect("job lock"),
                updated_at: Default::default(),
            };
            if let Err(err) = tjobs::report_worker(&ctx.pool, &status).await {
                tracing::debug!(error = %err, "resource report failed");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

/// Passes progress to the job row every second and pause or stop
/// requests from the UI to the transcription.
fn spawn_job_monitor(
    pool: PgPool,
    id: uuid::Uuid,
    mut progress: tokio::sync::watch::Receiver<f32>,
    interrupt: tokio::sync::watch::Sender<Option<Interrupt>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let value = *progress.borrow_and_update();
            match jobs::report_progress(&pool, id, value).await {
                Ok(Some(jobs::Control::Pause)) => {
                    let _ = interrupt.send(Some(Interrupt::Pause));
                }
                Ok(Some(jobs::Control::Stop)) => {
                    let _ = interrupt.send(Some(Interrupt::Stop));
                }
                Ok(None) => {}
                Err(err) => tracing::debug!(error = %err, "progress report failed"),
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            // The transcription ended.
            if progress.has_changed().is_err() {
                return;
            }
        }
    })
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
    let result: anyhow::Result<i32> = async {
        let text = talkops_core::speech::with_speed(&clip.text, clip.speech_speed)?;
        ctx.piper
            .render_with(voice.model, &[(text, path.clone())])
            .await?;
        wav_duration_ms(&path)
    }
    .await;
    let duration = result.as_ref().ok().copied();
    audio::set_rendered(&ctx.pool, p.clip_id, duration).await?;
    result.map(|_| ())
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
    let current = voicemail::get_box(&ctx.pool, job.tenant_id, p.owner).await?;
    if current.greeting != "tts"
        || current.greeting_text != p.text
        || current.speech_speed != p.speech_speed
    {
        return Ok(());
    }
    let path = ctx.voicemail_dir.join(&p.file);
    let result = async {
        let text = talkops_core::speech::with_speed(&p.text, p.speech_speed)?;
        ctx.piper.render(&p.language, &[(text, path)]).await
    }
    .await;
    voicemail::set_greeting_status_at_speed(
        &ctx.pool,
        p.owner,
        &p.text,
        p.speech_speed,
        result.is_ok(),
    )
    .await?;
    result
}

/// Renders the announcement before a voicemail message (caller, time).
async fn tts_message_info(ctx: &Ctx, job: &Job) -> anyhow::Result<()> {
    let p: vmc::MessageInfoJob =
        serde_json::from_value(job.payload.clone()).context("invalid payload")?;
    anyhow::ensure!(
        !p.file
            .split('/')
            .any(|part| part == ".." || part.is_empty())
            && p.file.ends_with("-info.wav"),
        "invalid output path"
    );
    let path = ctx.voicemail_dir.join(&p.file);
    // The message may be gone already (deleted right away).
    if !path.parent().is_some_and(Path::is_dir) {
        return Ok(());
    }
    ctx.piper.render_with(&p.voice, &[(p.text, path)]).await
}

/// Renders the prompts tenants reworded or gave another voice (all tenants
/// with a voicemail configuration, or one). Returns how many were rendered.
async fn tenant_prompts(
    ctx: &Ctx,
    only: Option<talkops_core::tenant::TenantId>,
) -> anyhow::Result<usize> {
    let tenants: Vec<talkops_core::tenant::TenantId> = sqlx::query_scalar(
        "SELECT tenant_id FROM tenant_settings
         WHERE ($1::uuid IS NOT NULL OR voicemail_config <> '{}'::jsonb OR EXISTS (
             SELECT 1 FROM voicemail_boxes b WHERE b.tenant_id = tenant_settings.tenant_id
             AND b.speech_speed <> 1.0)) AND ($1::uuid IS NULL OR tenant_id = $1)",
    )
    .bind(only)
    .fetch_all(&ctx.pool)
    .await?;
    let mut rendered = 0;
    for tenant in tenants {
        let config = vmc::get(&ctx.pool, tenant).await?;
        let mut speeds: Vec<f64> = sqlx::query_scalar(
            "SELECT DISTINCT speech_speed FROM voicemail_boxes WHERE tenant_id = $1",
        )
        .bind(tenant)
        .fetch_all(&ctx.pool)
        .await?;
        if !speeds.contains(&1.0) {
            speeds.push(1.0);
        }
        for speed in speeds {
            for lang in talkops_core::prompts::LANGUAGES {
                let model = config.voice_model(lang);
                let missing: Vec<(String, PathBuf)> = talkops_core::prompts::keys()
                    .filter_map(|key| {
                        let text = config.rendered_text(&key, lang, speed)?;
                        let path = talkops_core::prompts::path_for(
                            &ctx.sounds_dir,
                            &key,
                            lang,
                            model,
                            &text,
                        );
                        (!path.is_file()).then_some((text, path))
                    })
                    .collect();
                if missing.is_empty() {
                    continue;
                }
                tracing::info!(%tenant, lang, count = missing.len(), "rendering tenant prompts");
                ctx.piper.render_with(model, &missing).await?;
                rendered += missing.len();
            }
        }
        // Refresh the caller/date audio of stored messages after settings
        // changes. Content hashes separate old jobs from current settings.
        let boxes: Vec<uuid::Uuid> =
            sqlx::query_scalar("SELECT extension_id FROM voicemail_boxes WHERE tenant_id = $1")
                .bind(tenant)
                .fetch_all(&ctx.pool)
                .await?;
        for ext in boxes {
            for msg in voicemail::list_messages(&ctx.pool, tenant, ext).await? {
                if let Some(p) = vmc::message_info_job(&ctx.pool, tenant, &msg).await? {
                    let path = ctx.voicemail_dir.join(&p.file);
                    if !path.is_file() && ctx.voicemail_dir.join(&msg.file).is_file() {
                        ctx.piper.render_with(&p.voice, &[(p.text, path)]).await?;
                        rendered += 1;
                    }
                }
            }
        }
    }
    Ok(rendered)
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
    #[serde(default = "talkops_core::speech::default_speed")]
    speech_speed: f64,
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

/// Transcribes a call recording or voicemail in up to two passes: a quick
/// one whose transcript is shown (and mailed) right away, then optionally a
/// more accurate one that replaces it. A voicemail's e-mail waits for the
/// first pass and is sent once it is there or has finally failed.
///
/// A manual job (`engine` in the payload) runs one final pass with the
/// selected engine. Users can pause and stop the job; a local Whisper run
/// continues at the paused position.
async fn transcribe(ctx: &Ctx, job: &Job) -> anyhow::Result<JobEnd> {
    let p: TranscribePayload =
        serde_json::from_value(job.payload.clone()).context("invalid payload")?;
    let tenant = job.tenant_id;
    let tenant_settings = settings::get(&ctx.pool, tenant).await?;
    let voicemail = p.voicemail_id.is_some();
    let manual = p.engine.is_some();
    let (first, refine) = match &p.engine {
        Some(choice) => (choice.engine(&ctx.pool, tenant).await?, None),
        None => {
            let first = profiles::engine(&ctx.pool, tenant, Purpose::for_pass(voicemail, 1))
                .await?
                .unwrap_or(Engine::Local(settings::TranscriptionQuality::Fast));
            // A second pass equal to the first adds nothing.
            let refine = profiles::engine(&ctx.pool, tenant, Purpose::for_pass(voicemail, 2))
                .await?
                .filter(|e| *e != first);
            (first, refine)
        }
    };
    let (source, file, language, audio_secs) = match (p.recording_id, p.voicemail_id) {
        (Some(id), None) => {
            let rec = recordings::get(&ctx.pool, tenant, id).await?;
            let lang = tenant_settings.default_language.clone();
            (
                Source::Recording(id),
                ctx.recordings_dir.join(rec.file),
                lang,
                rec.duration_secs,
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
                msg.duration_secs,
            )
        }
        _ => anyhow::bail!("payload needs recording_id or voicemail_id"),
    };
    let engine = if p.pass >= 2 {
        match refine.clone() {
            Some(engine) => engine,
            // Switched off since the first pass.
            None => {
                recordings::finalize_transcript(&ctx.pool, source).await?;
                return Ok(JobEnd::Done);
            }
        }
    } else {
        first
    };
    let key = tjobs::engine_key(&engine);
    // A resume point is only valid for the same engine.
    let resume: Option<Checkpoint> = job
        .checkpoint
        .as_ref()
        .filter(|c| c.get("engine").and_then(|e| e.as_str()) == Some(key.as_str()))
        .and_then(|c| serde_json::from_value(c.get("state")?.clone()).ok());
    let resumed = resume.is_some();
    let purpose = Purpose::for_pass(voicemail, p.pass);
    let language = talkops_core::prompts::language(&language);
    let started = std::time::Instant::now();
    let (progress_tx, progress_rx) =
        tokio::sync::watch::channel(resume.as_ref().map_or(0.0, |c| c.progress));
    let (interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(None);
    let monitor = spawn_job_monitor(ctx.pool.clone(), job.id, progress_rx, interrupt_tx);
    let control = Control {
        progress: progress_tx,
        interrupt: interrupt_rx,
    };
    let result = run_engine_controlled(
        ctx,
        tenant,
        purpose,
        engine,
        &file,
        language,
        &tenant_settings.transcription_vocabulary,
        &control,
        resume,
    )
    .await;
    drop(control);
    monitor.abort();
    let last_attempt = job.attempts >= job.max_attempts;
    let result = match result {
        Ok(Run::Done(done)) => Ok(done),
        Ok(Run::Paused(checkpoint)) => {
            let progress = checkpoint.as_ref().map_or(0.0, |c| c.progress);
            return Ok(JobEnd::Paused {
                progress,
                checkpoint: checkpoint
                    .map(|state| serde_json::json!({"engine": key, "state": state})),
            });
        }
        Ok(Run::Stopped) => return Ok(JobEnd::Stopped),
        Err(err) => Err(err),
    };
    let finished = match &result {
        Ok(done) => {
            let is_final = manual || p.pass >= 2 || refine.is_none();
            let meta = recordings::TranscriptMeta {
                engine: &done.engine,
                is_final,
                fallback_from: done.fallback_from.as_deref(),
            };
            // Coral reports the language it detected; others keep the setting.
            let language = done.language.as_deref().unwrap_or(language);
            recordings::save_transcript(&ctx.pool, tenant, source, language, &done.segments, meta)
                .await?;
            let secs = started.elapsed().as_secs_f32();
            tracing::info!(
                ?source,
                pass = p.pass,
                manual,
                engine = %done.engine,
                fallback_from = ?done.fallback_from,
                segments = done.segments.len(),
                secs,
                "transcribed"
            );
            if !resumed && done.fallback_from.is_none() {
                let _ = tjobs::record_speed(&ctx.pool, &key, audio_secs as f32, secs).await;
            }
            if !is_final {
                let payload = TranscribePayload {
                    pass: 2,
                    ..p.clone()
                };
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
        Err(_) if last_attempt && (p.pass >= 2 || manual) => {
            recordings::finalize_transcript(&ctx.pool, source).await?;
            false
        }
        Err(_) if last_attempt => {
            recordings::transcript_failed(&ctx.pool, source).await?;
            true
        }
        Err(_) => false,
    };
    if let (true, 1, false, Source::Voicemail(id)) = (finished, p.pass, manual, source) {
        let msg = voicemail::get_message(&ctx.pool, tenant, id).await?;
        let mut conn = ctx.pool.acquire().await?;
        voicemail::enqueue_mail(&mut conn, tenant, &msg).await?;
    }
    result.map(|_| JobEnd::Done)
}

/// A finished transcription.
#[derive(Debug)]
pub(crate) struct Transcribed {
    pub segments: Vec<recordings::Segment>,
    /// The engine that made the text, e.g. `whisper:base` or `api:Coral/coral-auto`.
    pub engine: String,
    /// The selected engine that was not available (local fallback).
    pub fallback_from: Option<String>,
    /// The language the engine detected (Coral), if any.
    pub language: Option<String>,
}

/// The result of a transcription that users can pause and stop.
enum Run {
    Done(Transcribed),
    /// Paused; a local run has a resume point, a cloud run starts again.
    Paused(Option<Checkpoint>),
    Stopped,
}

/// Like [run_engine], with progress, pause and stop for jobs.
#[allow(clippy::too_many_arguments)]
async fn run_engine_controlled(
    ctx: &Ctx,
    tenant: talkops_core::tenant::TenantId,
    purpose: Purpose,
    engine: Engine,
    file: &Path,
    language: &str,
    vocabulary: &str,
    control: &Control,
    resume: Option<Checkpoint>,
) -> anyhow::Result<Run> {
    match engine {
        Engine::Local(quality) => {
            let opts = transcribe::Options {
                quality,
                vocabulary: vocabulary.to_owned(),
            };
            let label = format!("whisper:{}", ctx.whisper.model_name(quality));
            Ok(
                match ctx
                    .whisper
                    .transcribe_with(file, language, &opts, Some(control), resume)
                    .await?
                {
                    Outcome::Done(segments) => Run::Done(Transcribed {
                        segments,
                        engine: label,
                        fallback_from: None,
                        language: None,
                    }),
                    Outcome::Paused(checkpoint) => Run::Paused(Some(checkpoint)),
                    Outcome::Stopped => Run::Stopped,
                },
            )
        }
        cloud => {
            // The provider gives no progress; a pause starts the request again.
            let mut interrupt = control.interrupt.clone();
            let work = run_engine(ctx, tenant, purpose, cloud, file, language, vocabulary);
            tokio::pin!(work);
            loop {
                tokio::select! {
                    result = &mut work => {
                        let done = result?;
                        let _ = control.progress.send(1.0);
                        return Ok(Run::Done(done));
                    }
                    changed = interrupt.changed() => {
                        if changed.is_err() {
                            return Ok(Run::Done(work.await?));
                        }
                        match *interrupt.borrow() {
                            Some(Interrupt::Pause) => return Ok(Run::Paused(None)),
                            Some(Interrupt::Stop) => return Ok(Run::Stopped),
                            None => {}
                        }
                    }
                }
            }
        }
    }
}

/// Runs one transcription. Audio sent to a cloud provider is added to the
/// usage. A Coral connection that is not available, overloaded or fails
/// is replaced by its local fallback model (ADR 0046).
pub(crate) async fn run_engine(
    ctx: &Ctx,
    tenant: talkops_core::tenant::TenantId,
    purpose: Purpose,
    engine: Engine,
    file: &Path,
    language: &str,
    vocabulary: &str,
) -> anyhow::Result<Transcribed> {
    match engine {
        Engine::Local(quality) => {
            let (segments, label) =
                run_local(ctx, purpose, quality, file, language, vocabulary).await?;
            Ok(Transcribed {
                segments,
                engine: label,
                fallback_from: None,
                language: None,
            })
        }
        Engine::Cloud {
            provider_id,
            model,
            price,
            coral,
            fallback,
            ..
        } => {
            let cfg = cloud_config(ctx, tenant, provider_id, &model).await?;
            let fallback = fallback.filter(|_| coral);
            if let Some(quality) = fallback
                && ctx.coral_backoff.active(provider_id)
            {
                return local_fallback(ctx, purpose, quality, &cfg, file, language, vocabulary)
                    .await;
            }
            match api::transcribe(&cfg, file, language, vocabulary).await {
                Ok(done) => {
                    if coral {
                        ctx.coral_backoff.succeeded(provider_id);
                    } else {
                        profiles::record_usage(
                            &ctx.pool,
                            tenant,
                            purpose,
                            &cfg.label(),
                            done.seconds,
                            price,
                        )
                        .await?;
                    }
                    Ok(Transcribed {
                        segments: done.segments,
                        engine: format!("api:{}", cfg.label()),
                        fallback_from: None,
                        language: done.language,
                    })
                }
                Err(err) => match fallback {
                    Some(quality) => {
                        tracing::warn!(connection = %cfg.label(), error = %format!("{err:#}"), "Coral not available, using the local fallback");
                        ctx.coral_backoff.failed(provider_id);
                        local_fallback(ctx, purpose, quality, &cfg, file, language, vocabulary)
                            .await
                    }
                    None => Err(err),
                },
            }
        }
    }
}

/// Local Whisper in place of an unavailable Coral connection. The live
/// preview always uses `base`.
async fn local_fallback(
    ctx: &Ctx,
    purpose: Purpose,
    quality: settings::TranscriptionQuality,
    cfg: &transcription_api::ApiConfig,
    file: &Path,
    language: &str,
    vocabulary: &str,
) -> anyhow::Result<Transcribed> {
    let quality = if purpose == Purpose::Live {
        settings::TranscriptionQuality::Base
    } else {
        quality
    };
    let (segments, label) = run_local(ctx, purpose, quality, file, language, vocabulary).await?;
    Ok(Transcribed {
        segments,
        engine: label,
        fallback_from: Some(cfg.label()),
        language: None,
    })
}

async fn run_local(
    ctx: &Ctx,
    purpose: Purpose,
    quality: settings::TranscriptionQuality,
    file: &Path,
    language: &str,
    vocabulary: &str,
) -> anyhow::Result<(Vec<recordings::Segment>, String)> {
    let opts = transcribe::Options {
        quality,
        vocabulary: vocabulary.to_owned(),
    };
    let whisper = if purpose == Purpose::Live {
        // Previews leave CPU for calls and the other transcriptions.
        Whisper {
            threads: ctx.whisper.threads.clamp(1, 2),
            ..ctx.whisper.clone()
        }
    } else {
        ctx.whisper.clone()
    };
    let segments = whisper.transcribe(file, language, &opts).await?;
    Ok((segments, format!("whisper:{}", whisper.model_name(quality))))
}

/// A cloud connection with its decrypted key. A connection without a key
/// (Coral) also works without TALKOPS_SECRET_KEY.
pub(crate) async fn cloud_config(
    ctx: &Ctx,
    tenant: talkops_core::tenant::TenantId,
    provider_id: uuid::Uuid,
    model: &str,
) -> anyhow::Result<transcription_api::ApiConfig> {
    Ok(
        transcription_api::config(&ctx.pool, tenant, ctx.secrets.as_ref(), provider_id, model)
            .await?,
    )
}

#[cfg(all(test, unix))]
mod tests;
