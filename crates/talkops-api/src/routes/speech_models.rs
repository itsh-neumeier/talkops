//! Coral model library (ADR 0048): the administrator uploads compiled
//! models that they may use and selects one per slot; TalkOps forwards the
//! requests to coral-stt, which checks and stores the models.

use std::time::Duration;

use axum::{
    Json,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
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

/// Model slots of coral-stt.
const SLOTS: [&str; 3] = ["de", "en", "langid"];
/// Two model files of at most 64 MiB each plus metadata (coral-stt limit).
const UPLOAD_LIMIT: usize = 129 * 1024 * 1024;
/// Hashing and storing two large files takes longer than the client default.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(120);

pub fn router() -> OpenApiRouter<AppState> {
    let upload = OpenApiRouter::new()
        .routes(routes!(add_model))
        .layer(DefaultBodyLimit::max(UPLOAD_LIMIT));
    OpenApiRouter::new()
        .routes(routes!(list_models))
        .routes(routes!(delete_model))
        .routes(routes!(select_model, clear_slot))
        .routes(routes!(restart))
        .merge(upload)
}

async fn config(
    state: &AppState,
    auth: &AuthUser,
    id: Uuid,
) -> ApiResult<transcription_api::ApiConfig> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    let cfg =
        transcription_api::config(&state.db, auth.tenant, Some(&state.secrets), id, "").await?;
    if !cfg.is_coral() {
        return Err(ApiError::BadRequest("select a Coral connection".into()));
    }
    Ok(cfg)
}

fn slot(name: &str) -> ApiResult<&'static str> {
    SLOTS
        .into_iter()
        .find(|s| *s == name)
        .ok_or_else(|| ApiError::BadRequest("unknown model slot; use de, en or langid".into()))
}

/// Library ids are the first 12 hex digits of the model digest.
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
/// coral-stt errors (for example a rejected model file) keep their message
/// for the UI.
async fn send(req: reqwest::RequestBuilder) -> ApiResult<Value> {
    let response = req
        .send()
        .await
        .map_err(|_| ApiError::BadRequest("Coral service is not reachable".into()))?;
    let status = response.status();
    if status == reqwest::StatusCode::NO_CONTENT {
        return Ok(Value::Null);
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| ApiError::BadRequest("Coral service answer is not JSON".into()))?;
    if status.is_success() {
        return Ok(body);
    }
    let message = body["error"]["message"]
        .as_str()
        .unwrap_or("Coral service rejected the request");
    Err(ApiError::BadRequest(format!(
        "Coral service (HTTP {}): {message}",
        status.as_u16()
    )))
}

/// Slots, library models with license and digest, and library usage.
#[utoipa::path(get, path = "/api/v1/coral/{id}/models", tag = "settings", params(("id" = Uuid, Path)), responses((status = 200, body = Object)))]
pub async fn list_models(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    Ok(Json(
        send(request(&cfg, reqwest::Method::GET, "/models")).await?,
    ))
}

/// Adds a model to the library. Multipart fields as coral-stt:
/// `model_json`, `model_edgetpu`, optional `model_cpu`, `slot`, `name`,
/// `license`, `source`.
#[utoipa::path(post, path = "/api/v1/coral/{id}/library", tag = "settings", params(("id" = Uuid, Path)), request_body(content = Vec<u8>, content_type = "multipart/form-data"), responses((status = 200, body = Object)))]
pub async fn add_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("multipart/form-data"))
        .ok_or_else(|| ApiError::BadRequest("multipart/form-data is required".into()))?
        .to_owned();
    let result = send(
        request(&cfg, reqwest::Method::POST, "/library")
            .timeout(UPLOAD_TIMEOUT)
            .header(header::CONTENT_TYPE, content_type)
            .body(body),
    )
    .await?;
    let info = &result["info"];
    audit::record(
        &state.db,
        &auth.actor(),
        "add_coral_model",
        "transcription_provider",
        Some(id.to_string()),
        json!({"model": result["id"], "slot": info["slot"], "name": info["name"],
            "tpu_sha256": info["tpu_sha256"], "license": info["license"], "source": info["source"]}),
    )
    .await?;
    Ok(Json(result))
}

/// Deletes a library model that no slot uses.
#[utoipa::path(delete, path = "/api/v1/coral/{id}/library/{model}", tag = "settings", params(("id" = Uuid, Path), ("model" = String, Path)), responses((status = 204)))]
pub async fn delete_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, model)): Path<(Uuid, String)>,
) -> ApiResult<StatusCode> {
    let cfg = config(&state, &auth, id).await?;
    let model = model_id(&model)?;
    send(request(
        &cfg,
        reqwest::Method::DELETE,
        &format!("/library/{model}"),
    ))
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete_coral_model",
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
#[utoipa::path(put, path = "/api/v1/coral/{id}/slots/{slot}", tag = "settings", params(("id" = Uuid, Path), ("slot" = String, Path)), request_body = SelectInput, responses((status = 200, body = Object)))]
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
        request(&cfg, reqwest::Method::PUT, &format!("/slots/{slot}"))
            .timeout(UPLOAD_TIMEOUT)
            .json(&json!({"id": model})),
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "select_coral_model",
        "transcription_provider",
        Some(id.to_string()),
        json!({"slot": slot, "model": model, "tpu_sha256": result["tpu_sha256"]}),
    )
    .await?;
    Ok(Json(result))
}

/// Empties a slot; its model stays in the library.
#[utoipa::path(delete, path = "/api/v1/coral/{id}/slots/{slot}", tag = "settings", params(("id" = Uuid, Path), ("slot" = String, Path)), responses((status = 200, body = Object)))]
pub async fn clear_slot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let slot = slot(&name)?;
    let result = send(request(
        &cfg,
        reqwest::Method::DELETE,
        &format!("/slots/{slot}"),
    ))
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "clear_coral_slot",
        "transcription_provider",
        Some(id.to_string()),
        json!({"slot": slot}),
    )
    .await?;
    Ok(Json(result))
}

/// Restarts coral-stt so it loads the selected models. Needs a container
/// restart policy; live previews pause for a few seconds.
#[utoipa::path(post, path = "/api/v1/coral/{id}/restart", tag = "settings", params(("id" = Uuid, Path)), responses((status = 200, body = Object)))]
pub async fn restart(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let result = send(request(&cfg, reqwest::Method::POST, "/restart")).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "restart_coral",
        "transcription_provider",
        Some(id.to_string()),
        json!({}),
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
}
