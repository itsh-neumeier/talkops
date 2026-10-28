//! Coral model installation (ADR 0048): the administrator uploads compiled
//! models that they may use; TalkOps forwards them to coral-stt, which
//! checks and stores them in the models volume.

use std::time::Duration;

use axum::{
    Json,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, header},
};
use serde_json::{Value, json};
use talkops_core::{audit, roles::Permission, transcription_api};
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
        .routes(routes!(install_model, remove_model))
        .layer(DefaultBodyLimit::max(UPLOAD_LIMIT));
    OpenApiRouter::new()
        .routes(routes!(list_models))
        .routes(routes!(restore_model))
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

/// Sends a request and returns the JSON answer. coral-stt errors (for
/// example a rejected model file) keep their message for the UI.
async fn send(req: reqwest::RequestBuilder) -> ApiResult<Value> {
    let response = req
        .send()
        .await
        .map_err(|_| ApiError::BadRequest("Coral service is not reachable".into()))?;
    let status = response.status();
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

/// Model slots on disk: license, digest, active in the running service.
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

fn audit_details(slot: &str, result: &Value) -> Value {
    json!({"slot": slot, "tpu_sha256": result["tpu_sha256"],
        "license": result["info"]["license"], "source": result["info"]["source"]})
}

/// Installs a model into a slot. Multipart fields as coral-stt:
/// `model_json`, `model_edgetpu`, optional `model_cpu`, `license`, `source`.
/// The model is active after a service restart.
#[utoipa::path(post, path = "/api/v1/coral/{id}/models/{slot}", tag = "settings", params(("id" = Uuid, Path), ("slot" = String, Path)), request_body(content = Vec<u8>, content_type = "multipart/form-data"), responses((status = 200, body = Object)))]
pub async fn install_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let slot = slot(&name)?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("multipart/form-data"))
        .ok_or_else(|| ApiError::BadRequest("multipart/form-data is required".into()))?
        .to_owned();
    let result = send(
        request(&cfg, reqwest::Method::POST, &format!("/models/{slot}"))
            .timeout(UPLOAD_TIMEOUT)
            .header(header::CONTENT_TYPE, content_type)
            .body(body),
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "install_coral_model",
        "transcription_provider",
        Some(id.to_string()),
        audit_details(slot, &result),
    )
    .await?;
    Ok(Json(result))
}

/// Removes the model of a slot; it stays available as the backup.
#[utoipa::path(delete, path = "/api/v1/coral/{id}/models/{slot}", tag = "settings", params(("id" = Uuid, Path), ("slot" = String, Path)), responses((status = 200, body = Object)))]
pub async fn remove_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let slot = slot(&name)?;
    let result = send(request(
        &cfg,
        reqwest::Method::DELETE,
        &format!("/models/{slot}"),
    ))
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "remove_coral_model",
        "transcription_provider",
        Some(id.to_string()),
        json!({"slot": slot}),
    )
    .await?;
    Ok(Json(result))
}

/// Swaps the model of a slot with its backup.
#[utoipa::path(post, path = "/api/v1/coral/{id}/models/{slot}/restore", tag = "settings", params(("id" = Uuid, Path), ("slot" = String, Path)), responses((status = 200, body = Object)))]
pub async fn restore_model(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, name)): Path<(Uuid, String)>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let slot = slot(&name)?;
    let result = send(request(
        &cfg,
        reqwest::Method::POST,
        &format!("/models/{slot}/restore"),
    ))
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "restore_coral_model",
        "transcription_provider",
        Some(id.to_string()),
        audit_details(slot, &result),
    )
    .await?;
    Ok(Json(result))
}

/// Restarts coral-stt so it loads the installed models. Needs a container
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
    fn slots_are_an_allow_list() {
        assert_eq!(slot("de").unwrap(), "de");
        assert!(slot("../models").is_err());
        assert!(slot("DE").is_err());
    }
}
