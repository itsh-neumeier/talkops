//! The OpenAI-compatible HTTP interface (a subset):
//!
//! - `GET /health`: status, Edge TPUs (kind, load, loaded models), queue.
//! - `GET /v1/models`: `coral-ctc-de`, `coral-ctc-en`, `coral-auto`.
//! - `POST /v1/audio/transcriptions`: multipart as OpenAI; `json`, `text`
//!   or `verbose_json` with segments, times and the detected `language`.
//! - `GET /v1/realtime?intent=transcription`: see [crate::realtime].
//!
//! - `/v1/admin/models[/{slot}[/restore]]`, `POST /v1/admin/restart`:
//!   model installation by an administrator, see [crate::install].
//!
//! An optional key protects `/v1/*` (`Authorization: Bearer <key>`).
//! Overload answers HTTP 503 with `Retry-After` and the error code
//! `overloaded`.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

use crate::audio::{self, MODEL_RATE};
use crate::ctc::Word;
use crate::engine::{DeviceStatus, Engine, EngineError, Priority, QueueStatus};
use crate::install::{self, InstallError, SlotStatus, Upload};
use crate::models::AUTO;
use crate::recognizer::{Choice, Turn};
use crate::turns;

/// Seconds a client waits after an overload answer.
pub const RETRY_AFTER_SECS: u64 = 5;
/// Largest accepted upload (about 50 minutes of 16 kHz PCM16 mono).
const MAX_UPLOAD: usize = 100 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub api_key: Option<String>,
    /// Opt-in complete-word realtime previews. File decoding is unchanged.
    pub stable_preview: bool,
    /// Model installation; `None` disables `/v1/admin/*`.
    pub admin: Option<ModelAdmin>,
}

/// Model directory and restart request of the admin interface.
#[derive(Clone)]
pub struct ModelAdmin {
    pub dir: PathBuf,
    /// Set to `true` to stop the service; the container restart policy
    /// starts it again with the installed models.
    pub restart: Arc<tokio::sync::watch::Sender<bool>>,
}

/// An error in the OpenAI format.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }
}

impl From<EngineError> for ApiError {
    fn from(err: EngineError) -> Self {
        let message = err.to_string();
        match err {
            EngineError::Overloaded => {
                Self::new(StatusCode::SERVICE_UNAVAILABLE, "overloaded", message)
            }
            EngineError::Unavailable => {
                Self::new(StatusCode::SERVICE_UNAVAILABLE, "unavailable", message)
            }
            EngineError::UnknownModel(_) => {
                Self::new(StatusCode::NOT_FOUND, "model_not_found", message)
            }
            EngineError::Failed(_) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "inference_failed",
                message,
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let kind = if self.status.is_server_error() {
            "server_error"
        } else {
            "invalid_request_error"
        };
        let body = Json(json!({"error": {
            "message": self.message, "type": kind, "code": self.code, "param": null
        }}));
        let mut res = (self.status, body).into_response();
        if self.status == StatusCode::SERVICE_UNAVAILABLE {
            res.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(RETRY_AFTER_SECS));
        }
        res
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/audio/transcriptions", post(transcribe))
        .route("/v1/realtime", get(realtime))
        .route("/v1/diagnostics", get(diagnostic_report))
        .route(
            "/v1/diagnostics/capture",
            post(diagnostic_start).delete(diagnostic_stop),
        )
        .route("/v1/admin/models", get(admin_models))
        .route(
            "/v1/admin/models/{slot}",
            axum::routing::delete(admin_remove),
        )
        .route("/v1/admin/models/{slot}/restore", post(admin_restore))
        .route("/v1/admin/restart", post(admin_restart))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD))
        .merge(
            Router::new()
                .route("/v1/admin/models/{slot}", post(admin_install))
                .layer(DefaultBodyLimit::max(
                    2 * install::MAX_MODEL_BYTES + 1024 * 1024,
                )),
        )
        .with_state(state)
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(key) = &state.api_key else {
        return Ok(());
    };
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if given == Some(key.as_str()) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key",
            "missing or wrong API key",
        ))
    }
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

async fn health(State(state): State<AppState>) -> Response {
    let health = health_of(&state.engine);
    let code = if health.status == "unavailable" {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    (code, Json(health)).into_response()
}

#[derive(Deserialize)]
struct DiagnosticQuery {
    scope: String,
}

#[derive(Deserialize)]
struct DiagnosticCapture {
    scope: String,
    minutes: u32,
    #[serde(default)]
    include_text: bool,
}

async fn diagnostic_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DiagnosticQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers)?;
    if !crate::diagnostics::valid_scope(&query.scope) {
        return Err(ApiError::bad_request("invalid diagnostic scope"));
    }
    let models: Vec<_> = state.engine.catalog.files().iter().map(|m| json!({"id": m.name,
        "sha256": m.sha256,
        "language": m.language, "candidate": m.meta.candidate, "window_frames": m.meta.window_frames,
        "overlap_frames": m.meta.overlap(), "subsampling": m.meta.subsampling,
        "window_stride": m.mel.fe.window_stride, "input_layout": m.meta.input_layout})).collect();
    Ok(Json(
        json!({"schema_version": 1, "service_version": env!("CARGO_PKG_VERSION"),
          "service_build_revision": option_env!("TALKOPS_BUILD_REVISION"),
        "generated_unix_ms": crate::diagnostics::now_ms(), "platform": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "stable_preview_default": state.stable_preview, "health": health_of(&state.engine),
        "models": models, "capture": state.engine.diagnostics.snapshot(&query.scope),
        "timing_note": "processing and first-output timings; no human-aligned word latency or WER"}),
    ))
}

async fn diagnostic_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<DiagnosticCapture>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers)?;
    state
        .engine
        .diagnostics
        .start(&input.scope, input.minutes, input.include_text)
        .map_err(ApiError::bad_request)?;
    Ok(Json(state.engine.diagnostics.snapshot(&input.scope)))
}

async fn diagnostic_stop(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DiagnosticQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers)?;
    if !crate::diagnostics::valid_scope(&query.scope) {
        return Err(ApiError::bad_request("invalid diagnostic scope"));
    }
    state.engine.diagnostics.stop(&query.scope);
    Ok(Json(state.engine.diagnostics.snapshot(&query.scope)))
}

async fn models(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    authorize(&state, &headers)?;
    let data: Vec<_> = state
        .engine
        .catalog
        .ids()
        .into_iter()
        .map(|id| json!({"id": id, "object": "model", "created": 0, "owned_by": "talkops-coral"}))
        .collect();
    Ok(Json(json!({"object": "list", "data": data})).into_response())
}

/// The language choice for a model name and the request language.
pub fn choice(engine: &Engine, model: &str, languages: &[String]) -> Result<Choice, ApiError> {
    let available = engine.catalog.languages();
    if model == AUTO && !available.is_empty() {
        // Requested languages first (tenant language), then the others.
        let mut order: Vec<String> = languages
            .iter()
            .map(|l| normalize_language(l))
            .filter(|l| available.contains(l))
            .collect();
        for lang in available {
            if !order.contains(&lang) {
                order.push(lang);
            }
        }
        return Ok(Choice::Auto(order));
    }
    match model.strip_prefix("coral-ctc-") {
        Some(lang) if engine.catalog.ctc(lang).is_some() => Ok(Choice::Fixed(lang.to_owned())),
        _ => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "model_not_found",
            format!("the model `{model}` does not exist"),
        )),
    }
}

/// `de`, `de-DE`, `german` -> `de`.
pub fn normalize_language(language: &str) -> String {
    let l = language.trim().to_ascii_lowercase();
    match l.as_str() {
        "german" | "deutsch" => "de".into(),
        "english" => "en".into(),
        _ => l.split(['-', '_']).next().unwrap_or_default().to_owned(),
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Segment {
    pub id: usize,
    pub start: f32,
    pub end: f32,
    pub text: String,
    pub language: Option<String>,
    pub avg_logprob: f32,
}

#[derive(Debug, Serialize)]
pub struct Verbose {
    pub task: &'static str,
    pub language: Option<String>,
    pub duration: f32,
    pub text: String,
    pub segments: Vec<Segment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<Word>>,
}

/// Transcribes 16 kHz audio turn by turn.
pub async fn transcribe_samples(
    engine: &Arc<Engine>,
    samples: &[f32],
    choice: &Choice,
) -> Result<(Vec<Segment>, Vec<Word>), EngineError> {
    let mut segments = Vec::new();
    let mut words = Vec::new();
    for (start, end) in turns::split(samples, MODEL_RATE) {
        let mut turn = Turn::new(engine.clone(), choice.clone(), Priority::File)?;
        turn.push(&samples[start..end]).await?;
        let d = turn.finish().await?;
        if d.text.trim().is_empty() {
            continue;
        }
        let offset = start as f32 / MODEL_RATE as f32;
        words.extend(d.words.iter().map(|w| Word {
            word: w.word.clone(),
            start: w.start + offset,
            end: w.end + offset,
        }));
        segments.push(Segment {
            id: segments.len(),
            start: offset,
            end: end as f32 / MODEL_RATE as f32,
            text: d.text,
            language: d.language,
            avg_logprob: if d.confidence.is_finite() {
                d.confidence
            } else {
                0.0
            },
        });
    }
    Ok((segments, words))
}

/// The language with the most speech.
pub fn main_language(segments: &[Segment]) -> Option<String> {
    let mut by: Vec<(String, f32)> = Vec::new();
    for s in segments {
        let Some(lang) = &s.language else { continue };
        match by.iter_mut().find(|(l, _)| l == lang) {
            Some(entry) => entry.1 += s.end - s.start,
            None => by.push((lang.clone(), s.end - s.start)),
        }
    }
    by.into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(l, _)| l)
}

async fn transcribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Result<Response, ApiError> {
    authorize(&state, &headers)?;
    let (mut file, mut model, mut language) = (None, None, None);
    let mut format = "json".to_owned();
    let mut word_times = false;
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?
    {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "file" => {
                file = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| ApiError::bad_request(e.to_string()))?,
                )
            }
            _ => {
                let value = field
                    .text()
                    .await
                    .map_err(|e| ApiError::bad_request(e.to_string()))?;
                match name.as_str() {
                    "model" => model = Some(value),
                    "language" => language = Some(value),
                    "response_format" => format = value,
                    "timestamp_granularities[]" | "timestamp_granularities" => {
                        word_times |= value == "word"
                    }
                    // prompt, temperature, ...: a CTC model has no use for them.
                    _ => {}
                }
            }
        }
    }
    let file = file.ok_or_else(|| ApiError::bad_request("the file field is missing"))?;
    let model = model.ok_or_else(|| ApiError::bad_request("the model field is missing"))?;
    if !matches!(format.as_str(), "json" | "text" | "verbose_json") {
        return Err(ApiError::bad_request(format!(
            "response_format `{format}` is not supported (json, text, verbose_json)"
        )));
    }
    let languages: Vec<String> = language.into_iter().collect();
    let choice = choice(&state.engine, &model, &languages)?;
    let (samples, rate) =
        audio::decode_wav(&file).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let duration = samples.len() as f32 / rate.max(1) as f32;
    let samples = tokio::task::spawn_blocking(move || audio::resample(&samples, rate, MODEL_RATE))
        .await
        .map_err(|e| ApiError::from(EngineError::Failed(e.to_string())))?;
    let (segments, words) = transcribe_samples(&state.engine, &samples, &choice).await?;
    let text = segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let language = main_language(&segments).or_else(|| match &choice {
        Choice::Fixed(l) => Some(l.clone()),
        Choice::Auto(order) => order.first().cloned(),
    });
    Ok(match format.as_str() {
        "text" => text.into_response(),
        "json" => Json(json!({"text": text, "language": language})).into_response(),
        _ => Json(Verbose {
            task: "transcribe",
            language,
            duration,
            text,
            segments,
            words: word_times.then_some(words),
        })
        .into_response(),
    })
}

#[derive(Debug, Deserialize)]
struct RealtimeQuery {
    intent: Option<String>,
}

async fn realtime(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RealtimeQuery>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    authorize(&state, &headers)?;
    if query.intent.as_deref() != Some("transcription") {
        return Err(ApiError::bad_request(
            "only transcription sessions are supported (?intent=transcription)",
        ));
    }
    let Some(guard) = state.engine.try_session() else {
        let err = if state.engine.max_sessions() == 0 {
            EngineError::Unavailable
        } else {
            EngineError::Overloaded
        };
        return Err(err.into());
    };
    let engine = state.engine.clone();
    Ok(upgrade.on_upgrade(move |socket| {
        crate::realtime::session(socket, engine, guard, state.stable_preview)
    }))
}

impl From<InstallError> for ApiError {
    fn from(err: InstallError) -> Self {
        match err {
            InstallError::Invalid(message) => Self::bad_request(message),
            InstallError::Io(_) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_directory",
                // Usually a read-only models volume.
                format!("{err}; check that the models volume is writable"),
            ),
        }
    }
}

fn admin(state: &AppState, headers: &HeaderMap) -> Result<ModelAdmin, ApiError> {
    authorize(state, headers)?;
    state.admin.clone().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "admin_disabled",
            "model installation is disabled",
        )
    })
}

/// Runs blocking file work of the admin interface.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, InstallError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_directory",
                e.to_string(),
            )
        })?
        .map_err(ApiError::from)
}

#[derive(Serialize)]
struct AdminModels {
    backend: &'static str,
    models: Vec<SlotStatus>,
}

async fn admin_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminModels>, ApiError> {
    let admin = admin(&state, &headers)?;
    let catalog = state.engine.catalog.clone();
    let models = blocking(move || Ok(install::list(&admin.dir, &catalog))).await?;
    Ok(Json(AdminModels {
        backend: state.engine.backend_name(),
        models,
    }))
}

/// Multipart fields: `model_json`, `model_edgetpu`, optional `model_cpu`,
/// `license` and `source`.
async fn admin_install(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<SlotStatus>, ApiError> {
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
            Some("license") => upload.license = field.text().await.map_err(read_error)?,
            Some("source") => upload.source = field.text().await.map_err(read_error)?,
            _ => {}
        }
    }
    let status = blocking(move || install::install(&admin.dir, &slot, upload)).await?;
    tracing::info!(slot = status.slot, sha256 = ?status.tpu_sha256, "model installed; active after a restart");
    Ok(Json(status))
}

async fn admin_restore(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot): Path<String>,
) -> Result<Json<SlotStatus>, ApiError> {
    let admin = admin(&state, &headers)?;
    let status = blocking(move || install::restore(&admin.dir, &slot)).await?;
    tracing::info!(
        slot = status.slot,
        "previous model restored; active after a restart"
    );
    Ok(Json(status))
}

async fn admin_remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot): Path<String>,
) -> Result<Json<SlotStatus>, ApiError> {
    let admin = admin(&state, &headers)?;
    let status = blocking(move || install::remove(&admin.dir, &slot)).await?;
    tracing::info!(slot = status.slot, "model removed; kept as backup");
    Ok(Json(status))
}

/// Stops the service so the container restart policy starts it again with
/// the installed models. Open calls lose the Coral preview for a moment.
async fn admin_restart(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let admin = admin(&state, &headers)?;
    tracing::info!("restart requested through the admin interface");
    admin.restart.send_replace(true);
    Ok((StatusCode::ACCEPTED, Json(json!({"restarting": true}))))
}
