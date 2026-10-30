//! The HTTP interface of `cuda-stt`. The OpenAI-compatible routes, the
//! realtime sessions and the diagnostics are shared with `coral-stt` (crate
//! `talkops-stt-service`):
//!
//! - `GET /health`: status, GPU (NVML), loaded models with VRAM, queue.
//! - `GET /v1/models`: `cuda-de`, `cuda-en`, `cuda-auto`.
//! - `POST /v1/audio/transcriptions`, `GET /v1/realtime?intent=transcription`,
//!   `/v1/diagnostics`: see [talkops_stt_service::http].
//!
//! Model library of the administrator (see [crate::library]):
//!
//! - `GET /v1/admin/models`: slots, library, usage, presets.
//! - `POST /v1/admin/library`: multipart upload of an archive (`archive`)
//!   with `name`, `family`, `languages`, `license`, `source`, optional
//!   `sha256`, `precision` and `slot`. The archive is written to disk while
//!   it arrives.
//! - `POST /v1/admin/library/download`: JSON with `url` (HTTPS) and the same
//!   statements; `sha256` is required.
//! - `DELETE /v1/admin/library/{id}`, `PUT|DELETE /v1/admin/slots/{slot}`,
//!   `POST /v1/admin/restart`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Multipart, Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post, put};
use axum::{Extension, Json, Router};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use talkops_stt_service::diagnostics::Diagnostics;
use talkops_stt_service::http::{self as service, ApiError, admin, blocking};
use talkops_stt_service::library::{InstallError, invalid};
use talkops_stt_service::{Choice, EngineError, Priority, SpeechEngine};
use tokio::io::AsyncWriteExt;

use crate::engine::{DeviceStatus, Engine, QueueStatus, SessionGuard};
use crate::gpu::GpuStatus;
use crate::library::{self, Install, LibraryModel, Limits, ModelCheck, Overview, Precision};
use crate::models::{AUTO, LANGUAGES};
use crate::recognizer::Turn;

pub type AppState = service::AppState<Engine>;

/// Settings of the model library.
#[derive(Clone)]
pub struct Library {
    pub limits: Limits,
    pub check: ModelCheck,
    /// Download client; `None` disables downloads (tests).
    pub client: Option<reqwest::Client>,
}

/// Suggested models (data file `presets/stt-models/cuda.json`).
pub const PRESETS: &str = include_str!("../../../presets/stt-models/cuda.json");

pub fn router(state: AppState, library: Library) -> Router {
    service::routes::<Engine>()
        .route("/v1/admin/models", get(admin_models))
        .route("/v1/admin/library/download", post(admin_download))
        .route("/v1/admin/library/{id}", delete(admin_delete))
        .route(
            "/v1/admin/slots/{slot}",
            put(admin_select).delete(admin_clear),
        )
        .layer(DefaultBodyLimit::max(service::MAX_UPLOAD))
        .merge(
            // The upload counts its bytes against the model limit itself.
            Router::new()
                .route("/v1/admin/library", post(admin_add))
                .layer(DefaultBodyLimit::disable()),
        )
        .layer(Extension(Arc::new(library)))
        .with_state(state)
}

#[derive(Debug, Serialize)]
pub struct Health {
    /// `ok`, `degraded` (a selected model failed) or `unavailable`.
    pub status: &'static str,
    pub version: &'static str,
    /// `cuda`, `cpu` or `mock`.
    pub backend: &'static str,
    pub gpu: Option<GpuStatus>,
    pub device: Option<DeviceStatus>,
    pub queue: QueueStatus,
    pub models: Vec<String>,
}

pub fn health_of(engine: &Engine) -> Health {
    let device = engine.device();
    let models = engine.model_ids();
    let failed = device
        .as_ref()
        .is_some_and(|d| d.models.iter().any(|m| m.state != "ready"))
        || !engine.catalog.errors.is_empty();
    let status = if !engine.ready() || engine.loaded_languages().is_empty() {
        "unavailable"
    } else if failed {
        "degraded"
    } else {
        "ok"
    };
    Health {
        status,
        version: env!("CARGO_PKG_VERSION"),
        backend: engine.backend_name(),
        gpu: engine.gpu.as_ref().map(|g| g.status()),
        device,
        queue: engine.queue_status(),
        models,
    }
}

impl SpeechEngine for Engine {
    type Turn = Turn;
    type Session = SessionGuard;

    fn owner(&self) -> &'static str {
        "talkops-cuda"
    }

    fn auto_model(&self) -> &'static str {
        AUTO
    }

    fn fixed_language(&self, model: &str) -> Option<String> {
        let lang = model.strip_prefix("cuda-")?;
        self.loaded_languages().into_iter().find(|l| l == lang)
    }

    fn languages(&self) -> Vec<String> {
        self.loaded_languages()
    }

    fn model_ids(&self) -> Vec<String> {
        let languages = self.loaded_languages();
        let mut ids: Vec<String> = languages.iter().map(|l| format!("cuda-{l}")).collect();
        if !languages.is_empty() {
            ids.push(AUTO.into());
        }
        ids
    }

    fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    fn turn(self: &Arc<Self>, choice: Choice, priority: Priority) -> Result<Turn, EngineError> {
        Turn::new(self.clone(), choice, priority)
    }

    fn try_session(self: &Arc<Self>) -> Result<SessionGuard, EngineError> {
        Engine::try_session(self)
    }

    fn queue_status(&self) -> Value {
        json!(Engine::queue_status(self))
    }

    fn health(&self) -> (bool, Value) {
        let health = health_of(self);
        (health.status != "unavailable", json!(health))
    }

    fn diagnostic_models(&self) -> Value {
        let loaded = self.device().map(|d| d.models).unwrap_or_default();
        self.catalog
            .slots
            .iter()
            .map(|(slot, e)| {
                let state = loaded.iter().find(|m| &m.slot == slot);
                json!({"slot": slot, "id": e.id, "name": e.manifest.name, "family": e.manifest.family,
                "files": e.manifest.files, "sha256": e.manifest.sha256, "license": e.manifest.license,
                "state": state.map(|m| m.state), "vram_mib": state.and_then(|m| m.vram_mib),
                "load_secs": state.map(|m| m.load_secs),
                "partial_interval_ms": self.partial_samples as f64 / 16.0,
                "max_turn_secs": self.max_turn_samples as f64 / 16_000.0})
            })
            .collect()
    }
}

#[derive(Serialize)]
struct AdminModels {
    backend: &'static str,
    languages: [&'static str; 2],
    families: [&'static str; 4],
    #[serde(flatten)]
    overview: Overview,
    presets: Value,
}

async fn admin_models(
    State(state): State<AppState>,
    Extension(lib): Extension<Arc<Library>>,
    headers: HeaderMap,
) -> Result<Json<AdminModels>, ApiError> {
    let admin = admin(&state, &headers)?;
    let catalog = state.engine.catalog.clone();
    let loaded = state.engine.device().map(|d| d.models).unwrap_or_default();
    let limits = lib.limits;
    let overview = blocking(move || {
        Ok::<_, InstallError>(library::overview(&admin.dir, &catalog, &loaded, &limits))
    })
    .await?;
    Ok(Json(AdminModels {
        backend: state.engine.backend_name(),
        languages: LANGUAGES,
        families: ["nemo_ctc", "nemo_transducer", "canary", "whisper"],
        overview,
        presets: serde_json::from_str(PRESETS).unwrap_or_default(),
    }))
}

fn precision(value: &str) -> Precision {
    if value.trim() == "int8" {
        Precision::Int8
    } else {
        Precision::Full
    }
}

/// Installs a stored archive, selects it if asked, and removes the archive.
async fn install(
    dir: PathBuf,
    archive: PathBuf,
    install: Install,
    slot: Option<String>,
    lib: Arc<Library>,
) -> Result<Json<LibraryModel>, ApiError> {
    let model = blocking(move || {
        let result = library::add_archive(&dir, &archive, install, &lib.limits, &lib.check);
        let _ = std::fs::remove_file(&archive);
        let mut model = result?;
        if let Some(slot) = slot.filter(|s| !s.is_empty()) {
            library::select(&dir, &slot, &model.id)?;
            model.slots.push(slot);
        }
        Ok::<_, InstallError>(model)
    })
    .await?;
    tracing::info!(id = %model.id, name = %model.manifest.name, family = ?model.manifest.family,
        slots = ?model.slots, "model added to the library");
    Ok(Json(model))
}

async fn create_file(path: &Path) -> Result<tokio::fs::File, ApiError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ApiError::from(InstallError::Io(e)))?;
    }
    tokio::fs::File::create(path)
        .await
        .map_err(|e| InstallError::Io(e).into())
}

async fn admin_add(
    State(state): State<AppState>,
    Extension(lib): Extension<Arc<Library>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<LibraryModel>, ApiError> {
    let admin = admin(&state, &headers)?;
    let archive = library::staging_path(&admin.dir, "upload");
    let result = async {
        let mut fields = Install::default();
        let mut slot = None;
        let mut received = false;
        let read_error =
            |e: axum::extract::multipart::MultipartError| ApiError::bad_request(e.body_text());
        while let Some(mut field) = multipart.next_field().await.map_err(read_error)? {
            let name = field.name().unwrap_or_default().to_owned();
            if name == "archive" {
                let mut file = create_file(&archive).await?;
                let mut bytes = 0u64;
                while let Some(chunk) = field.chunk().await.map_err(read_error)? {
                    bytes += chunk.len() as u64;
                    if bytes > lib.limits.max_model_bytes {
                        return Err(invalid(format!(
                            "the archive is larger than {} MiB",
                            lib.limits.max_model_bytes >> 20
                        ))
                        .into());
                    }
                    file.write_all(&chunk)
                        .await
                        .map_err(|e| ApiError::from(InstallError::Io(e)))?;
                }
                file.flush()
                    .await
                    .map_err(|e| ApiError::from(InstallError::Io(e)))?;
                received = bytes > 0;
                continue;
            }
            let value = field.text().await.map_err(read_error)?;
            match name.as_str() {
                "name" => fields.name = value,
                "family" => fields.family = value,
                "languages" => fields.languages = value,
                "license" => fields.license = value,
                "source" => fields.source = value,
                "sha256" => fields.sha256 = Some(value).filter(|v| !v.trim().is_empty()),
                "precision" => fields.precision = precision(&value),
                "slot" => slot = Some(value),
                _ => {}
            }
        }
        if !received {
            return Err(ApiError::bad_request("the archive field is missing"));
        }
        Ok((fields, slot))
    }
    .await;
    match result {
        Ok((fields, slot)) => install(admin.dir, archive, fields, slot, lib).await,
        Err(err) => {
            let _ = tokio::fs::remove_file(&archive).await;
            Err(err)
        }
    }
}

#[derive(Deserialize)]
struct DownloadInput {
    url: String,
    sha256: String,
    name: String,
    family: String,
    languages: String,
    license: String,
    source: String,
    #[serde(default)]
    precision: String,
    #[serde(default)]
    slot: Option<String>,
}

async fn download(
    client: &reqwest::Client,
    url: &str,
    to: &Path,
    limit: u64,
) -> Result<(), ApiError> {
    let failed = |e: String| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "download_failed",
            format!("download failed: {e}"),
        )
    };
    let response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| failed(e.to_string()))?;
    if response.content_length().is_some_and(|n| n > limit) {
        return Err(invalid(format!("the archive is larger than {} MiB", limit >> 20)).into());
    }
    let mut file = create_file(to).await?;
    let mut bytes = 0u64;
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| failed(e.to_string()))?;
        bytes += chunk.len() as u64;
        if bytes > limit {
            return Err(invalid(format!("the archive is larger than {} MiB", limit >> 20)).into());
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| ApiError::from(InstallError::Io(e)))?;
    }
    file.flush()
        .await
        .map_err(|e| ApiError::from(InstallError::Io(e)))?;
    Ok(())
}

async fn admin_download(
    State(state): State<AppState>,
    Extension(lib): Extension<Arc<Library>>,
    headers: HeaderMap,
    Json(input): Json<DownloadInput>,
) -> Result<Json<LibraryModel>, ApiError> {
    let admin = admin(&state, &headers)?;
    if !input.url.starts_with("https://") || input.url.len() > 2048 {
        return Err(ApiError::bad_request("url: an HTTPS URL is required"));
    }
    let sha = input.sha256.trim().to_ascii_lowercase();
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "sha256: the expected SHA-256 of the archive is required",
        ));
    }
    let Some(client) = &lib.client else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "download_disabled",
            "downloads are disabled",
        ));
    };
    let archive = library::staging_path(&admin.dir, "download");
    tracing::info!(url = %input.url, "model download started");
    if let Err(err) = download(client, &input.url, &archive, lib.limits.max_model_bytes).await {
        let _ = tokio::fs::remove_file(&archive).await;
        return Err(err);
    }
    let fields = Install {
        name: input.name,
        family: input.family,
        languages: input.languages,
        license: input.license,
        source: input.source,
        sha256: Some(sha),
        precision: precision(&input.precision),
    };
    install(admin.dir, archive, fields, input.slot, lib.clone()).await
}

async fn admin_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    let admin = admin(&state, &headers)?;
    blocking(move || library::delete(&admin.dir, &id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SelectInput {
    id: String,
}

async fn admin_select(
    State(state): State<AppState>,
    headers: HeaderMap,
    UrlPath(slot): UrlPath<String>,
    Json(input): Json<SelectInput>,
) -> Result<Json<Value>, ApiError> {
    let admin = admin(&state, &headers)?;
    let (s, id) = (slot.clone(), input.id.clone());
    blocking(move || library::select(&admin.dir, &s, &id)).await?;
    tracing::info!(slot = %slot, id = %input.id, "model selected; active after a restart");
    Ok(Json(json!({"slot": slot, "model_id": input.id})))
}

async fn admin_clear(
    State(state): State<AppState>,
    headers: HeaderMap,
    UrlPath(slot): UrlPath<String>,
) -> Result<Json<Value>, ApiError> {
    let admin = admin(&state, &headers)?;
    let s = slot.clone();
    blocking(move || library::clear(&admin.dir, &s)).await?;
    tracing::info!(slot = %slot, "slot emptied; model kept in the library");
    Ok(Json(json!({"slot": slot, "model_id": null})))
}

/// The default timeout of the model check.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(300);
