//! The OpenAI-compatible HTTP interface (a subset) of a speech service:
//!
//! - `GET /health`: status, accelerators, queue, models.
//! - `GET /v1/models`: the model names of the service.
//! - `POST /v1/audio/transcriptions`: multipart as OpenAI; `json`, `text`
//!   or `verbose_json` with segments, times and the detected `language`.
//! - `GET /v1/realtime?intent=transcription`: see [crate::realtime].
//! - `/v1/diagnostics`: bounded technical traces, see [crate::diagnostics].
//! - `POST /v1/admin/restart`: stop the service for a model change.
//!
//! The model library routes under `/v1/admin/` depend on the model format
//! and are added by each service; they use [admin] and [blocking].

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Multipart, Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::audio::{self, MODEL_RATE};
use crate::engine::{Choice, EngineError, Priority, SpeechEngine, SpeechTurn, Word};
use crate::turns;

/// Seconds a client waits after an overload answer.
pub const RETRY_AFTER_SECS: u64 = 5;
/// Largest accepted upload (about 50 minutes of 16 kHz PCM16 mono).
pub const MAX_UPLOAD: usize = 100 * 1024 * 1024;

pub struct AppState<E> {
    pub engine: Arc<E>,
    pub api_key: Option<String>,
    /// Opt-in complete-word realtime previews. File decoding is unchanged.
    pub stable_preview: bool,
    /// Model installation; `None` disables `/v1/admin/*`.
    pub admin: Option<ModelAdmin>,
}

impl<E> Clone for AppState<E> {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            api_key: self.api_key.clone(),
            stable_preview: self.stable_preview,
            admin: self.admin.clone(),
        }
    }
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

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl From<EngineError> for ApiError {
    fn from(err: EngineError) -> Self {
        let status = match err {
            EngineError::Overloaded | EngineError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            EngineError::UnknownModel(_) => StatusCode::NOT_FOUND,
            EngineError::Failed(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, err.code(), err.to_string())
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

/// The shared routes, without state. A service merges its `/v1/admin/`
/// library routes and adds the state.
pub fn routes<E: SpeechEngine>() -> Router<AppState<E>> {
    Router::new()
        .route("/health", get(health::<E>))
        .route("/v1/models", get(models::<E>))
        .route("/v1/audio/transcriptions", post(transcribe::<E>))
        .route("/v1/realtime", get(realtime::<E>))
        .route("/v1/diagnostics", get(diagnostic_report::<E>))
        .route(
            "/v1/diagnostics/capture",
            post(diagnostic_start::<E>).delete(diagnostic_stop::<E>),
        )
        .route("/v1/admin/restart", post(admin_restart::<E>))
}

pub fn authorize<E>(state: &AppState<E>, headers: &HeaderMap) -> Result<(), ApiError> {
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

/// Checks the key and returns the admin settings, or `admin_disabled`.
pub fn admin<E>(state: &AppState<E>, headers: &HeaderMap) -> Result<ModelAdmin, ApiError> {
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
pub async fn blocking<T: Send + 'static, Err: Into<ApiError> + Send + 'static>(
    work: impl FnOnce() -> Result<T, Err> + Send + 'static,
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
        .map_err(Into::into)
}

async fn health<E: SpeechEngine>(State(state): State<AppState<E>>) -> Response {
    let (available, body) = state.engine.health();
    let code = if available {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body)).into_response()
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

async fn diagnostic_report<E: SpeechEngine>(
    State(state): State<AppState<E>>,
    headers: HeaderMap,
    Query(query): Query<DiagnosticQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers)?;
    if !crate::diagnostics::valid_scope(&query.scope) {
        return Err(ApiError::bad_request("invalid diagnostic scope"));
    }
    let engine = &state.engine;
    Ok(Json(
        json!({"schema_version": 1, "service_version": env!("CARGO_PKG_VERSION"),
          "service_build_revision": option_env!("TALKOPS_BUILD_REVISION"),
        "generated_unix_ms": crate::diagnostics::now_ms(), "platform": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "stable_preview_default": state.stable_preview, "health": engine.health().1,
        "models": engine.diagnostic_models(), "capture": engine.diagnostics().snapshot(&query.scope),
        "timing_note": "processing and first-output timings; no human-aligned word latency or WER"}),
    ))
}

async fn diagnostic_start<E: SpeechEngine>(
    State(state): State<AppState<E>>,
    headers: HeaderMap,
    Json(input): Json<DiagnosticCapture>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers)?;
    let diagnostics = state.engine.diagnostics();
    diagnostics
        .start(&input.scope, input.minutes, input.include_text)
        .map_err(ApiError::bad_request)?;
    Ok(Json(diagnostics.snapshot(&input.scope)))
}

async fn diagnostic_stop<E: SpeechEngine>(
    State(state): State<AppState<E>>,
    headers: HeaderMap,
    Query(query): Query<DiagnosticQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers)?;
    if !crate::diagnostics::valid_scope(&query.scope) {
        return Err(ApiError::bad_request("invalid diagnostic scope"));
    }
    let diagnostics = state.engine.diagnostics();
    diagnostics.stop(&query.scope);
    Ok(Json(diagnostics.snapshot(&query.scope)))
}

async fn models<E: SpeechEngine>(
    State(state): State<AppState<E>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    authorize(&state, &headers)?;
    let owner = state.engine.owner();
    let data: Vec<_> = state
        .engine
        .model_ids()
        .into_iter()
        .map(|id| json!({"id": id, "object": "model", "created": 0, "owned_by": owner}))
        .collect();
    Ok(Json(json!({"object": "list", "data": data})).into_response())
}

/// The language choice for a model name and the request language.
pub fn choice<E: SpeechEngine>(
    engine: &E,
    model: &str,
    languages: &[String],
) -> Result<Choice, ApiError> {
    let available = engine.languages();
    if model == engine.auto_model() && !available.is_empty() {
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
    match engine.fixed_language(model) {
        Some(lang) => Ok(Choice::Fixed(lang)),
        None => Err(ApiError::new(
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
pub async fn transcribe_samples<E: SpeechEngine>(
    engine: &Arc<E>,
    samples: &[f32],
    choice: &Choice,
) -> Result<(Vec<Segment>, Vec<Word>), EngineError> {
    let mut segments = Vec::new();
    let mut words = Vec::new();
    for (start, end) in turns::split(samples, MODEL_RATE) {
        let mut turn = engine.turn(choice.clone(), Priority::File)?;
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

async fn transcribe<E: SpeechEngine>(
    State(state): State<AppState<E>>,
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
                    // prompt, temperature, ...: not used.
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
    let choice = choice(state.engine.as_ref(), &model, &languages)?;
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
    let language = main_language(&segments).or_else(|| choice.preferred().map(str::to_owned));
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

async fn realtime<E: SpeechEngine>(
    State(state): State<AppState<E>>,
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
    let guard = state.engine.try_session()?;
    let engine = state.engine.clone();
    Ok(upgrade.on_upgrade(move |socket| {
        crate::realtime::session(socket, engine, guard, state.stable_preview)
    }))
}

/// Stops the service so the container restart policy starts it again with
/// the installed models. Open calls lose the live preview for a moment.
async fn admin_restart<E: SpeechEngine>(
    State(state): State<AppState<E>>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let admin = admin(&state, &headers)?;
    tracing::info!("restart requested through the admin interface");
    admin.restart.send_replace(true);
    Ok((StatusCode::ACCEPTED, Json(json!({"restarting": true}))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_normalized() {
        for (given, expected) in [
            ("de-DE", "de"),
            ("German", "de"),
            ("english", "en"),
            ("en_US", "en"),
        ] {
            assert_eq!(normalize_language(given), expected);
        }
    }

    #[test]
    fn main_language_has_the_most_speech() {
        let segment = |start: f32, end: f32, language: &str| Segment {
            id: 0,
            start,
            end,
            text: String::new(),
            language: Some(language.into()),
            avg_logprob: 0.0,
        };
        let segments = [
            segment(0.0, 1.0, "en"),
            segment(1.0, 4.0, "de"),
            segment(4.0, 5.0, "en"),
        ];
        assert_eq!(main_language(&segments).as_deref(), Some("de"));
        assert_eq!(main_language(&[]), None);
    }
}
