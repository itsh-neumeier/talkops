//! Model library of the speech services on this server: coral-stt (ADR 0048)
//! and cuda-stt (ADR 0049). The administrator installs models that they may
//! use and selects one per slot; TalkOps forwards the requests to the
//! service, which checks and stores the models.

use std::time::Duration;

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
};
use serde::Deserialize;
use serde_json::{Value, json};
use talkops_core::{audit, roles::Permission, transcription_api};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    AppState,
    auth::AuthUser,
    error::{ApiError, ApiResult},
};

/// Model slots of both services.
const SLOTS: [&str; 3] = ["de", "en", "langid"];
/// Storing, unpacking and checking a model takes longer than the client
/// default; a CUDA model can have gigabytes.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_models))
        .routes(routes!(add_model))
        .routes(routes!(download_model))
        .routes(routes!(delete_model))
        .routes(routes!(select_model, clear_slot))
        .routes(routes!(restart))
}

async fn config(
    state: &AppState,
    auth: &AuthUser,
    id: Uuid,
) -> ApiResult<transcription_api::ApiConfig> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    let cfg =
        transcription_api::config(&state.db, auth.tenant, Some(&state.secrets), id, "").await?;
    if !cfg.is_local() {
        return Err(ApiError::BadRequest(
            "select a Coral or CUDA connection".into(),
        ));
    }
    Ok(cfg)
}

/// `Coral service` or `CUDA service`, for error messages.
fn service(cfg: &transcription_api::ApiConfig) -> &'static str {
    if cfg.kind == "cuda" {
        "CUDA service"
    } else {
        "Coral service"
    }
}

fn slot(name: &str) -> ApiResult<&'static str> {
    SLOTS
        .into_iter()
        .find(|s| *s == name)
        .ok_or_else(|| ApiError::BadRequest("unknown model slot; use de, en or langid".into()))
}

/// Library ids are the first 12 hex digits of a digest.
fn model_id(id: &str) -> ApiResult<&str> {
    if id.len() == 12 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        Ok(id)
    } else {
        Err(ApiError::BadRequest("invalid model id".into()))
    }
}

fn request(
    cfg: &transcription_api::ApiConfig,
    method: reqwest::Method,
    path: &str,
) -> reqwest::RequestBuilder {
    let req = super::transcription::client().request(
        method,
        format!("{}/admin{path}", cfg.url.trim_end_matches('/')),
    );
    match &cfg.key {
        Some(key) => req.bearer_auth(key),
        None => req,
    }
}

/// Sends a request and returns the JSON answer (`null` for no content).
/// Service errors (for example a rejected model file) keep their message
/// for the UI.
async fn send(
    cfg: &transcription_api::ApiConfig,
    req: reqwest::RequestBuilder,
) -> ApiResult<Value> {
    let name = service(cfg);
    let response = req
        .send()
        .await
        .map_err(|_| ApiError::BadRequest(format!("{name} is not reachable")))?;
    let status = response.status();
    if status == reqwest::StatusCode::NO_CONTENT {
        return Ok(Value::Null);
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| ApiError::BadRequest(format!("{name} answer is not JSON")))?;
    if status.is_success() {
        return Ok(body);
    }
    let message = body["error"]["message"]
        .as_str()
        .unwrap_or("the service rejected the request");
    Err(ApiError::BadRequest(format!(
        "{name} (HTTP {}): {message}",
        status.as_u16()
    )))
}

/// The audit details of an added model (coral-stt: `info`, cuda-stt:
/// `manifest`). Never the file contents.
fn added(result: &Value) -> Value {
    let info = if result["info"].is_object() {
        &result["info"]
    } else {
        &result["manifest"]
    };
    json!({"model": result["id"], "slot": info["slot"], "slots": result["slots"],
        "name": info["name"], "family": info["family"],
        "sha256": info["tpu_sha256"].as_str().or(info["archive_sha256"].as_str()),
        "license": info["license"], "source": info["source"]})
}

/// Slots, library models with license and digest, and library usage.
#[utoipa::path(get, path = "/api/v1/speech-services/{id}/models", tag = "settings", params(("id" = Uuid, Path)), responses((status = 200, body = Object)))]
pub async fn list_models(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    Ok(Json(
        send(&cfg, request(&cfg, reqwest::Method::GET, "/models")).await?,
    ))
}

/// Adds a model to the library. The multipart body goes to the service
/// as it arrives (a CUDA model archive can have gigabytes). Fields:
/// coral-stt `model_json`, `model_edgetpu`, optional `model_cpu`;
/// cuda-stt `archive`, `family`, `languages`, optional `sha256` and
/// `precision`; both `slot`, `name`, `license`, `source`.
#[utoipa::path(post, path = "/api/v1/speech-services/{id}/library", tag = "settings", params(("id" = Uuid, Path)), request_body(content = Vec<u8>, content_type = "multipart/form-data"), responses((status = 200, body = Object)))]
pub async fn add_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Body,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("multipart/form-data"))
        .ok_or_else(|| ApiError::BadRequest("multipart/form-data is required".into()))?
        .to_owned();
    let result = send(
        &cfg,
        request(&cfg, reqwest::Method::POST, "/library")
            .timeout(INSTALL_TIMEOUT)
            .header(header::CONTENT_TYPE, content_type)
            .body(reqwest::Body::wrap_stream(body.into_data_stream())),
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "add_speech_model",
        "transcription_provider",
        Some(id.to_string()),
        added(&result),
    )
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize, ToSchema)]
pub struct DownloadInput {
    /// HTTPS URL of a model archive.
    pub url: String,
    /// Expected SHA-256 of the archive.
    pub sha256: String,
    pub name: String,
    /// `nemo_ctc`, `nemo_transducer`, `canary` or `whisper`.
    pub family: String,
    /// Comma-separated ISO 639-1 codes.
    pub languages: String,
    pub license: String,
    pub source: String,
    #[serde(default)]
    pub precision: String,
    #[serde(default)]
    pub slot: Option<String>,
}

/// cuda-stt only: the service downloads a model archive, checks its
/// SHA-256 and adds it to the library.
#[utoipa::path(post, path = "/api/v1/speech-services/{id}/library/download", tag = "settings", params(("id" = Uuid, Path)), request_body = DownloadInput, responses((status = 200, body = Object)))]
pub async fn download_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<DownloadInput>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    if cfg.kind != "cuda" {
        return Err(ApiError::BadRequest(
            "only a CUDA connection can download models".into(),
        ));
    }
    if let Some(s) = input.slot.as_deref().filter(|s| !s.is_empty()) {
        slot(s)?;
    }
    let body = json!({"url": input.url, "sha256": input.sha256, "name": input.name,
        "family": input.family, "languages": input.languages, "license": input.license,
        "source": input.source, "precision": input.precision, "slot": input.slot});
    let result = send(
        &cfg,
        request(&cfg, reqwest::Method::POST, "/library/download")
            .timeout(INSTALL_TIMEOUT)
            .json(&body),
    )
    .await?;
    let mut details = added(&result);
    details["url"] = json!(input.url);
    audit::record(
        &state.db,
        &auth.actor(),
        "download_speech_model",
        "transcription_provider",
        Some(id.to_string()),
        details,
    )
    .await?;
    Ok(Json(result))
}

/// Deletes a library model that no slot uses.
#[utoipa::path(delete, path = "/api/v1/speech-services/{id}/library/{model}", tag = "settings", params(("id" = Uuid, Path), ("model" = String, Path)), responses((status = 204)))]
pub async fn delete_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, model)): Path<(Uuid, String)>,
) -> ApiResult<StatusCode> {
    let cfg = config(&state, &auth, id).await?;
    let model = model_id(&model)?;
    send(
        &cfg,
        request(&cfg, reqwest::Method::DELETE, &format!("/library/{model}")),
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete_speech_model",
        "transcription_provider",
        Some(id.to_string()),
        json!({"model": model}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct SelectInput {
    /// Library model id.
    pub model: String,
}

/// Selects a library model for a slot. Active after a service restart.
#[utoipa::path(put, path = "/api/v1/speech-services/{id}/slots/{slot}", tag = "settings", params(("id" = Uuid, Path), ("slot" = String, Path)), request_body = SelectInput, responses((status = 200, body = Object)))]
pub async fn select_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
    Json(input): Json<SelectInput>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let slot = slot(&name)?;
    let model = model_id(&input.model)?;
    let result = send(
        &cfg,
        request(&cfg, reqwest::Method::PUT, &format!("/slots/{slot}"))
            .timeout(INSTALL_TIMEOUT)
            .json(&json!({"id": model})),
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "select_speech_model",
        "transcription_provider",
        Some(id.to_string()),
        json!({"slot": slot, "model": model, "tpu_sha256": result["tpu_sha256"]}),
    )
    .await?;
    Ok(Json(result))
}

/// Empties a slot; its model stays in the library.
#[utoipa::path(delete, path = "/api/v1/speech-services/{id}/slots/{slot}", tag = "settings", params(("id" = Uuid, Path), ("slot" = String, Path)), responses((status = 200, body = Object)))]
pub async fn clear_slot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let slot = slot(&name)?;
    let result = send(
        &cfg,
        request(&cfg, reqwest::Method::DELETE, &format!("/slots/{slot}")),
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "clear_speech_slot",
        "transcription_provider",
        Some(id.to_string()),
        json!({"slot": slot}),
    )
    .await?;
    Ok(Json(result))
}

/// Restarts the service so it loads the selected models. Needs a container
/// restart policy; live previews pause for a few seconds.
#[utoipa::path(post, path = "/api/v1/speech-services/{id}/restart", tag = "settings", params(("id" = Uuid, Path)), responses((status = 200, body = Object)))]
pub async fn restart(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let result = send(&cfg, request(&cfg, reqwest::Method::POST, "/restart")).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "restart_speech_service",
        "transcription_provider",
        Some(id.to_string()),
        json!({"kind": cfg.kind}),
    )
    .await?;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_and_ids_are_allow_lists() {
        assert_eq!(slot("de").unwrap(), "de");
        assert!(slot("../models").is_err());
        assert!(slot("DE").is_err());
        assert!(model_id("0123456789ab").is_ok());
        assert!(model_id("0123456789AB").is_err());
        assert!(model_id("../../etc/pa").is_err());
    }

    #[test]
    fn audit_details_of_both_services() {
        let coral = json!({"id": "0123456789ab", "info": {"slot": "de", "name": "QN",
            "tpu_sha256": "aa", "license": "x", "source": "y"}});
        assert_eq!(added(&coral)["sha256"], "aa");
        assert_eq!(added(&coral)["slot"], "de");
        let cuda = json!({"id": "0123456789ab", "slots": ["de", "en"], "manifest": {"name": "P",
            "family": "nemo_transducer", "archive_sha256": "bb", "license": "CC-BY-4.0", "source": "z",
            "files": {"encoder": "e.onnx"}}});
        let d = added(&cuda);
        assert_eq!(
            (d["sha256"].as_str(), d["family"].as_str()),
            (Some("bb"), Some("nemo_transducer"))
        );
        assert_eq!(d["slots"], json!(["de", "en"]));
        assert!(d.get("files").is_none());
    }
}
