//! `talkops-cuda-stt` binary: serve, check the health of a running service
//! (container health check), or check a model directory (child process of
//! the library check).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use talkops_cuda_stt::engine::{Backend, Engine, Settings};
use talkops_cuda_stt::gpu::Gpu;
use talkops_cuda_stt::http::{AppState, CHECK_TIMEOUT, Library, router};
use talkops_cuda_stt::library::{self, Limits, ModelCheck};
use talkops_cuda_stt::mock::{self, MockBackend};
use talkops_cuda_stt::models::{Catalog, MANIFEST, Manifest};
use talkops_cuda_stt::sherpa::{self, SherpaBackend};
use talkops_stt_service::http::ModelAdmin;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BackendKind {
    /// ONNX Runtime with the CUDA execution provider (production).
    Cuda,
    /// ONNX Runtime on the CPU (reference and model checks; slow).
    Cpu,
    /// Deterministic fake models without GPU (CI).
    Mock,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogFormat {
    Pretty,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "talkops-cuda-stt", version, about)]
struct Args {
    /// Listen address. Keep it on loopback: audio must not leave the server.
    #[arg(long, env = "TALKOPS_CUDA_LISTEN", default_value = "127.0.0.1:8096")]
    listen: SocketAddr,

    #[arg(long, env = "TALKOPS_CUDA_BACKEND", value_enum, default_value = "cuda")]
    backend: BackendKind,

    /// Directory with `library/` and `selection.json`.
    #[arg(
        long,
        env = "TALKOPS_CUDA_MODELS_DIR",
        default_value = "/var/lib/talkops/models/cuda"
    )]
    models_dir: PathBuf,

    /// Jobs that can wait for the GPU; more are rejected (HTTP 503).
    #[arg(long, env = "TALKOPS_CUDA_QUEUE", default_value_t = 32)]
    queue: usize,

    /// Realtime sessions (one per call channel).
    #[arg(long, env = "TALKOPS_CUDA_SESSIONS", default_value_t = 8)]
    sessions: usize,

    /// Most jobs of one model in one GPU batch.
    #[arg(long, env = "TALKOPS_CUDA_BATCH", default_value_t = 8)]
    batch: usize,

    /// A live turn decodes its audio again after this much new audio.
    #[arg(long, env = "TALKOPS_CUDA_PARTIAL_INTERVAL_MS", default_value_t = 1000)]
    partial_interval_ms: u64,

    /// A longer turn is cut at a pause (at least 10 s).
    #[arg(long, env = "TALKOPS_CUDA_MAX_TURN_SECS", default_value_t = 15)]
    max_turn_secs: u64,

    /// Experimental complete-word live previews; final decoding stays unchanged.
    #[arg(long, env = "TALKOPS_CUDA_STABLE_PREVIEW", default_value_t = false,
          action = clap::ArgAction::Set)]
    stable_preview: bool,

    /// Model installation through `/v1/admin/*` (TalkOps UI). Needs a
    /// writable models directory.
    #[arg(long, env = "TALKOPS_CUDA_MODEL_ADMIN", default_value_t = true,
          action = clap::ArgAction::Set)]
    model_admin: bool,

    /// Load each new library model in a child process before it is accepted.
    #[arg(long, env = "TALKOPS_CUDA_CHECK_LOAD", default_value_t = true,
          action = clap::ArgAction::Set)]
    check_load: bool,

    /// Library limits.
    #[arg(long, env = "TALKOPS_CUDA_MAX_MODEL_MB", default_value_t = 4096)]
    max_model_mb: u64,
    #[arg(long, env = "TALKOPS_CUDA_MAX_LIBRARY_MB", default_value_t = 20480)]
    max_library_mb: u64,
    #[arg(long, env = "TALKOPS_CUDA_MAX_MODELS", default_value_t = 10)]
    max_models: usize,

    /// Optional key that clients send as `Authorization: Bearer <key>`.
    #[arg(long, env = "TALKOPS_CUDA_API_KEY", hide_env_values = true)]
    api_key: Option<String>,

    /// CPU threads of ONNX Runtime.
    #[arg(long, env = "TALKOPS_CUDA_THREADS", default_value_t = 2)]
    threads: i32,

    /// Mock backend: delay per batch.
    #[arg(long, env = "TALKOPS_CUDA_MOCK_DELAY_MS", default_value_t = 0)]
    mock_delay_ms: u64,

    #[arg(long, env = "TALKOPS_LOG_FORMAT", value_enum, default_value = "pretty")]
    log_format: LogFormat,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Exit 0 if `/health` of the running service answers 200.
    Healthcheck,
    /// Load the model in a directory with `model.json` and decode silence.
    CheckModel {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long, default_value = "cpu")]
        provider: String,
    },
}

async fn healthcheck(addr: SocketAddr) -> anyhow::Result<()> {
    let mut stream =
        tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(addr))
            .await
            .context("timeout")??;
    stream
        .write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .await?;
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut response))
        .await
        .context("timeout")??;
    let head = String::from_utf8_lossy(&response[..response.len().min(16)]).into_owned();
    anyhow::ensure!(head.contains(" 200"), "unhealthy: {head}");
    Ok(())
}

fn check_model(dir: &std::path::Path, provider: &str) -> anyhow::Result<()> {
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(dir.join(MANIFEST))?)?;
    sherpa::check(dir, &manifest, provider).map_err(anyhow::Error::msg)
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match &args.command {
        Some(Command::Healthcheck) => return healthcheck(args.listen).await,
        Some(Command::CheckModel { dir, provider }) => return check_model(dir, provider),
        None => {}
    }
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    match args.log_format {
        LogFormat::Json => tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init(),
        LogFormat::Pretty => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), backend = ?args.backend, "cuda-stt starting");
    if !args.listen.ip().is_loopback() {
        tracing::warn!(listen = %args.listen, "not a loopback address: other hosts can send audio to this service");
    }
    library::clean(&args.models_dir);
    let mut catalog = Catalog::load(&args.models_dir);
    for (slot, err) in &catalog.errors {
        tracing::warn!(slot, error = %err, "selected model cannot be read");
    }
    let (backend, gpu, check): (Arc<dyn Backend>, Option<Gpu>, ModelCheck) = match args.backend {
        BackendKind::Mock => {
            if catalog.slots.is_empty() {
                catalog = mock::catalog();
            }
            (
                Arc::new(MockBackend::new(Duration::from_millis(args.mock_delay_ms))),
                None,
                ModelCheck::Files,
            )
        }
        kind => {
            let provider = if kind == BackendKind::Cuda {
                "cuda"
            } else {
                "cpu"
            };
            if catalog.slots.is_empty() {
                tracing::warn!(dir = %args.models_dir.display(), "no model selected; install one in the TalkOps UI");
            }
            let check = if args.check_load {
                ModelCheck::Load {
                    exe: std::env::current_exe()?,
                    // The check runs on the CPU: the GPU memory stays for the
                    // running models.
                    provider: "cpu".into(),
                    timeout: CHECK_TIMEOUT,
                }
            } else {
                ModelCheck::Files
            };
            (
                Arc::new(SherpaBackend {
                    provider,
                    threads: args.threads.max(1),
                }),
                (kind == BackendKind::Cuda).then(|| Gpu::open(0)).flatten(),
                check,
            )
        }
    };
    tracing::info!(slots = ?catalog.slots.iter().map(|(s, e)| (s, &e.manifest.name)).collect::<Vec<_>>(), "selected models");
    let engine = Engine::start(
        backend,
        catalog,
        gpu,
        Settings {
            queue: args.queue,
            sessions: args.sessions,
            batch: args.batch,
            partial_interval: Duration::from_millis(args.partial_interval_ms),
            max_turn: Duration::from_secs(args.max_turn_secs),
        },
    );
    let (restart, mut restart_requested) = tokio::sync::watch::channel(false);
    let state = AppState {
        engine: engine.clone(),
        api_key: args.api_key.filter(|k| !k.is_empty()),
        stable_preview: args.stable_preview,
        admin: args.model_admin.then(|| ModelAdmin {
            dir: args.models_dir.clone(),
            restart: Arc::new(restart),
        }),
    };
    let library = Library {
        limits: Limits {
            max_model_bytes: args.max_model_mb << 20,
            max_library_bytes: args.max_library_mb << 20,
            max_models: args.max_models,
        },
        check,
        client: Some(
            reqwest::Client::builder()
                .user_agent(concat!("talkops-cuda-stt/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(20))
                .build()?,
        ),
    };
    let listener = tokio::net::TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("listen on {}", args.listen))?;
    tracing::info!(listen = %args.listen, "listening");
    let mut drain = restart_requested.clone();
    let server = axum::serve(listener, router(state, library)).with_graceful_shutdown(async move {
        tokio::select! {
            _ = shutdown_signal() => {},
            _ = drain.wait_for(|requested| *requested) => {},
        }
    });
    tokio::select! {
        result = server => result?,
        // Realtime sessions can keep connections open: do not wait for them.
        _ = async {
            let _ = restart_requested.wait_for(|requested| *requested).await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        } => tracing::warn!("open connections closed for the requested restart"),
    }
    engine.shutdown();
    Ok(())
}
