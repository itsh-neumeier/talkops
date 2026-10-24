//! The OpenAI-compatible HTTP interface (a subset):
//!
//! - `GET /health`: status, Edge TPUs (kind, load, loaded models), queue.
//! - `GET /v1/models`: `coral-ctc-de`, `coral-ctc-en`, `coral-auto`.
//! - `POST /v1/audio/transcriptions`: multipart as OpenAI; `json`, `text`
//!   or `verbose_json` with segments, times and the detected `language`.
//! - `GET /v1/realtime?intent=transcription`: see [crate::realtime].
//!
//! An optional key protects `/v1/*` (`Authorization: Bearer <key>`).
//! Overload answers HTTP 503 with `Retry-After` and the error code
//! `overloaded`.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Multipart, Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::audio::{self, MODEL_RATE};
use crate::ctc::Word;
use crate::engine::{DeviceStatus, Engine, EngineError, Priority, QueueStatus};
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
        .layer(DefaultBodyLimit::max(MAX_UPLOAD))
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
    Ok(upgrade.on_upgrade(move |socket| crate::realtime::session(socket, engine, guard)))
}
