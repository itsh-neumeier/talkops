//! The HTTP interface of `coral-stt`. The OpenAI-compatible routes, the
//! realtime sessions and the diagnostics are shared with `cuda-stt` (crate
//! `talkops-stt-service`):
//!
//! - `GET /health`: status, Edge TPUs (kind, load, loaded models), queue.
//! - `GET /v1/models`: `coral-ctc-de`, `coral-ctc-en`, `coral-auto`.
//! - `POST /v1/audio/transcriptions`, `GET /v1/realtime?intent=transcription`,
//!   `/v1/diagnostics`: see [talkops_stt_service::http].
//!
//! - `/v1/admin/models`, `/v1/admin/library[/{id}]`, `/v1/admin/slots/{slot}`,
//!   `POST /v1/admin/restart`: model library of the administrator, see
//!   [crate::install].

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use talkops_stt_service::http::{self as service, admin, blocking};
use talkops_stt_service::{SpeechEngine, Word};

use crate::engine::{DeviceStatus, Engine, EngineError, Priority, QueueStatus, SessionGuard};
use crate::install::{self, LibraryModel, Overview, SlotStatus, Upload};
use crate::models::AUTO;
use crate::recognizer::{Choice, Turn};

pub use talkops_stt_service::http::{
    ApiError, ModelAdmin, RETRY_AFTER_SECS, Segment, Verbose, main_language, normalize_language,
};

pub type AppState = service::AppState<Engine>;

pub fn router(state: AppState) -> Router {
    service::routes::<Engine>()
        .route("/v1/admin/models", axum::routing::get(admin_models))
        .route(
            "/v1/admin/library/{id}",
            axum::routing::delete(admin_delete),
        )
        .route(
            "/v1/admin/slots/{slot}",
            axum::routing::put(admin_select).delete(admin_clear),
        )
        .layer(DefaultBodyLimit::max(service::MAX_UPLOAD))
        .merge(
            Router::new()
                .route("/v1/admin/library", axum::routing::post(admin_add))
                .layer(DefaultBodyLimit::max(
                    2 * install::MAX_MODEL_BYTES + 1024 * 1024,
                )),
        )
        .with_state(state)
}

#[derive(Debug, Serialize)]
pub struct Health {
    /// `ok`, `degraded` (some TPUs fail) or `unavailable`.
    pub status: &'static str,
    pub version: &'static str,
    /// `tpu`, `cpu` or `mock`.
    pub backend: &'static str,
    pub tpus: Vec<DeviceStatus>,
    pub queue: QueueStatus,
    pub models: Vec<String>,
}

pub fn health_of(engine: &Engine) -> Health {
    let tpus = engine.devices();
    let ready = tpus.iter().filter(|d| d.state == "ready").count();
    let models = engine.catalog.ids();
    let status = if ready == 0 || models.is_empty() {
        "unavailable"
    } else if ready < tpus.len() {
        "degraded"
    } else {
        "ok"
    };
    Health {
        status,
        version: env!("CARGO_PKG_VERSION"),
        backend: engine.backend_name(),
        tpus,
        queue: engine.queue_status(),
        models,
    }
}

/// The language choice for a model name and the request language.
pub fn choice(engine: &Engine, model: &str, languages: &[String]) -> Result<Choice, ApiError> {
    service::choice(engine, model, languages)
}

/// Transcribes 16 kHz audio turn by turn.
pub async fn transcribe_samples(
    engine: &Arc<Engine>,
    samples: &[f32],
    choice: &Choice,
) -> Result<(Vec<Segment>, Vec<Word>), EngineError> {
    service::transcribe_samples(engine, samples, choice).await
}

impl SpeechEngine for Engine {
    type Turn = Turn;
    type Session = SessionGuard;

    fn owner(&self) -> &'static str {
        "talkops-coral"
    }

    fn auto_model(&self) -> &'static str {
        AUTO
    }

    fn fixed_language(&self, model: &str) -> Option<String> {
        model
            .strip_prefix("coral-ctc-")
            .filter(|lang| self.catalog.ctc(lang).is_some())
            .map(str::to_owned)
    }

    fn languages(&self) -> Vec<String> {
        self.catalog.languages()
    }

    fn model_ids(&self) -> Vec<String> {
        self.catalog.ids()
    }

    fn diagnostics(&self) -> &talkops_stt_service::diagnostics::Diagnostics {
        &self.diagnostics
    }

    fn turn(self: &Arc<Self>, choice: Choice, priority: Priority) -> Result<Turn, EngineError> {
        Turn::new(self.clone(), choice, priority)
    }

    fn try_session(self: &Arc<Self>) -> Result<SessionGuard, EngineError> {
        Engine::try_session(self).ok_or_else(|| {
            if self.max_sessions() == 0 {
                EngineError::Unavailable
            } else {
                EngineError::Overloaded
            }
        })
    }

    fn queue_status(&self) -> Value {
        json!(Engine::queue_status(self))
    }

    fn health(&self) -> (bool, Value) {
        let health = health_of(self);
        (health.status != "unavailable", json!(health))
    }

    fn diagnostic_models(&self) -> Value {
        self.catalog.files().iter().map(|m| json!({"id": m.name,
            "sha256": m.sha256,
            "language": m.language, "candidate": m.meta.candidate, "window_frames": m.meta.window_frames,
            "overlap_frames": m.meta.overlap(), "subsampling": m.meta.subsampling,
            "window_stride": m.mel.fe.window_stride, "input_layout": m.meta.input_layout})).collect()
    }
}

#[derive(Serialize)]
struct AdminModels {
    backend: &'static str,
    #[serde(flatten)]
    overview: Overview,
}

async fn admin_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminModels>, ApiError> {
    let admin = admin(&state, &headers)?;
    let catalog = state.engine.catalog.clone();
    let overview =
        blocking(move || Ok::<_, install::InstallError>(install::overview(&admin.dir, &catalog)))
            .await?;
    Ok(Json(AdminModels {
        backend: state.engine.backend_name(),
        overview,
    }))
}

/// Multipart fields: `model_json`, `model_edgetpu`, optional `model_cpu`,
/// `slot`, `name`, `license` and `source`.
async fn admin_add(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<LibraryModel>, ApiError> {
    let admin = admin(&state, &headers)?;
    let mut upload = Upload::default();
    let read_error =
        |e: axum::extract::multipart::MultipartError| ApiError::bad_request(e.body_text());
    while let Some(field) = multipart.next_field().await.map_err(read_error)? {
        match field.name() {
            Some("model_json") => upload.meta = field.bytes().await.map_err(read_error)?.to_vec(),
            Some("model_edgetpu") => upload.tpu = field.bytes().await.map_err(read_error)?.to_vec(),
            Some("model_cpu") => {
                let bytes = field.bytes().await.map_err(read_error)?;
                upload.cpu = (!bytes.is_empty()).then(|| bytes.to_vec());
            }
            Some("slot") => upload.slot = field.text().await.map_err(read_error)?,
            Some("name") => upload.name = field.text().await.map_err(read_error)?,
            Some("license") => upload.license = field.text().await.map_err(read_error)?,
            Some("source") => upload.source = field.text().await.map_err(read_error)?,
            _ => {}
        }
    }
    let model = blocking(move || install::add(&admin.dir, upload)).await?;
    tracing::info!(id = %model.id, slot = %model.info.slot, "model added to the library");
    Ok(Json(model))
}

async fn admin_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let admin = admin(&state, &headers)?;
    blocking(move || install::delete(&admin.dir, &id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SelectInput {
    id: String,
}

async fn admin_select(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot): Path<String>,
    Json(input): Json<SelectInput>,
) -> Result<Json<SlotStatus>, ApiError> {
    let admin = admin(&state, &headers)?;
    let status = blocking(move || install::select(&admin.dir, &slot, &input.id)).await?;
    tracing::info!(slot = status.slot, id = ?status.model_id, "model selected; active after a restart");
    Ok(Json(status))
}

async fn admin_clear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot): Path<String>,
) -> Result<Json<SlotStatus>, ApiError> {
    let admin = admin(&state, &headers)?;
    let status = blocking(move || install::clear(&admin.dir, &slot)).await?;
    tracing::info!(
        slot = status.slot,
        "slot emptied; model kept in the library"
    );
    Ok(Json(status))
}
