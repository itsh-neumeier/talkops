//! `talkops-coral-stt` binary: serve, or check the health of a running
//! service (container health check).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use talkops_coral_stt::engine::{Backend, Engine};
use talkops_coral_stt::http::{AppState, ModelAdmin, router};
use talkops_coral_stt::mock::{self, MockBackend};
use talkops_coral_stt::models::Catalog;
use talkops_coral_stt::tflite::TfliteBackend;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendKind {
    /// Edge TPUs through libedgetpu (USB and PCIe/M.2).
    Tpu,
    /// TensorFlow Lite on the CPU (reference and tests; not for production).
    Cpu,
    /// Deterministic fake models without hardware (CI).
    Mock,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogFormat {
    Pretty,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "talkops-coral-stt", version, about)]
struct Args {
    /// Listen address. Keep it on loopback: audio must not leave the server.
    #[arg(long, env = "TALKOPS_CORAL_LISTEN", default_value = "127.0.0.1:8095")]
    listen: SocketAddr,

    #[arg(long, env = "TALKOPS_CORAL_BACKEND", value_enum, default_value = "tpu")]
    backend: BackendKind,

    /// Directory with `de/`, `en/` and `langid/` model directories.
    #[arg(
        long,
        env = "TALKOPS_CORAL_MODELS_DIR",
        default_value = "/var/lib/talkops/models/coral"
    )]
    models_dir: PathBuf,

    /// `libtensorflowlite_c.so` of the TensorFlow Lite version of libedgetpu.
    #[arg(long, env = "TALKOPS_CORAL_TFLITE_LIB")]
    tflite_lib: Option<PathBuf>,

    /// The Edge TPU delegate library.
    #[arg(
        long,
        env = "TALKOPS_CORAL_EDGETPU_LIB",
        default_value = "libedgetpu.so.1"
    )]
    edgetpu_lib: PathBuf,

    /// Jobs (windows) that can wait for a TPU; more are rejected (HTTP 503).
    #[arg(long, env = "TALKOPS_CORAL_QUEUE", default_value_t = 16)]
    queue: usize,

    /// Realtime sessions (one per call channel) per ready TPU.
    #[arg(long, env = "TALKOPS_CORAL_SESSIONS_PER_TPU", default_value_t = 4)]
    sessions_per_tpu: usize,

    /// Experimental complete-word live previews; final decoding stays unchanged.
    #[arg(long, env = "TALKOPS_CORAL_STABLE_PREVIEW", default_value_t = false,
          action = clap::ArgAction::Set)]
    stable_preview: bool,

    /// Model installation through `/v1/admin/*` (TalkOps UI upload). Needs a
    /// writable models directory.
    #[arg(long, env = "TALKOPS_CORAL_MODEL_ADMIN", default_value_t = true,
          action = clap::ArgAction::Set)]
    model_admin: bool,

    /// Optional key that clients send as `Authorization: Bearer <key>`.
    #[arg(long, env = "TALKOPS_CORAL_API_KEY", hide_env_values = true)]
    api_key: Option<String>,

    /// CPU threads per interpreter (CPU backend and CPU parts of TPU models).
    #[arg(long, env = "TALKOPS_CORAL_THREADS", default_value_t = 1)]
    threads: i32,

    /// Mock backend: number of fake devices and delay per inference.
    #[arg(long, env = "TALKOPS_CORAL_MOCK_DEVICES", default_value_t = 1)]
    mock_devices: usize,
    #[arg(long, env = "TALKOPS_CORAL_MOCK_DELAY_MS", default_value_t = 0)]
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
    if let Some(Command::Healthcheck) = args.command {
        return healthcheck(args.listen).await;
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
    tracing::info!(version = env!("CARGO_PKG_VERSION"), backend = ?args.backend, "coral-stt starting");
    if !args.listen.ip().is_loopback() {
        tracing::warn!(listen = %args.listen, "not a loopback address: other hosts can send audio to this service");
    }
    let (backend, catalog): (Arc<dyn Backend>, Catalog) = match args.backend {
        BackendKind::Mock => (
            Arc::new(MockBackend::new(
                args.mock_devices,
                Duration::from_millis(args.mock_delay_ms),
            )),
            mock::catalog(true),
        ),
        kind => {
            let tpu = matches!(kind, BackendKind::Tpu);
            let file = if tpu {
                "model_edgetpu.tflite"
            } else {
                "model.tflite"
            };
            let catalog = Catalog::load(&args.models_dir, file)?;
            if catalog.ctc.is_empty() {
                tracing::warn!(dir = %args.models_dir.display(), file, "no models found; the service reports unavailable");
            }
            (
                Arc::new(TfliteBackend {
                    library: args.tflite_lib.clone(),
                    delegate: tpu.then(|| args.edgetpu_lib.clone()),
                    threads: args.threads.max(1),
                    sysfs: "/sys/bus/usb/devices".into(),
                    dev: "/dev".into(),
                }),
                catalog,
            )
        }
    };
    tracing::info!(models = ?catalog.ids(), langid = catalog.langid.is_some(), "models");
    let engine = Engine::start(backend, catalog, args.queue, args.sessions_per_tpu);
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
    let listener = tokio::net::TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("listen on {}", args.listen))?;
    tracing::info!(listen = %args.listen, "listening");
    let mut drain = restart_requested.clone();
    let server = axum::serve(listener, router(state)).with_graceful_shutdown(async move {
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
