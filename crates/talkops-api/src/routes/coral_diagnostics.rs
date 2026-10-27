//! Permission-checked, tenant-scoped Coral capture and support report.
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use talkops_core::{audit, roles::Permission, transcription_api, transcription_profiles};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    AppState,
    auth::AuthUser,
    error::{ApiError, ApiResult},
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(connections))
        .routes(routes!(status))
        .routes(routes!(start_capture, stop_capture))
        .routes(routes!(download))
}

#[derive(serde::Serialize, ToSchema)]
pub struct Connection {
    pub id: Uuid,
    pub name: String,
}

#[utoipa::path(get, path = "/api/v1/diagnostics/coral", tag = "system", responses((status = 200, body = [Connection])))]
pub async fn connections(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Connection>>> {
    auth.require_perm(Permission::DiagnosticsManage)?;
    Ok(Json(
        transcription_api::list(&state.db, auth.tenant)
            .await?
            .into_iter()
            .filter(|p| p.kind == "coral")
            .map(|p| Connection {
                id: p.id,
                name: p.name,
            })
            .collect(),
    ))
}

#[utoipa::path(get, path = "/api/v1/diagnostics/coral/{id}", tag = "system", params(("id" = Uuid, Path)), responses((status = 200, body = Object)))]
pub async fn status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let mut result = send(request(
        &cfg,
        reqwest::Method::GET,
        &format!("/diagnostics?scope={}", auth.tenant),
    ))
    .await?;
    let count = result["capture"]["events"].as_array().map_or(0, Vec::len);
    if let Some(capture) = result["capture"].as_object_mut() {
        capture.remove("events");
        capture.insert("events_kept".into(), json!(count));
    }
    sanitize(&mut result, cfg.key.as_deref(), false);
    Ok(Json(result))
}

#[derive(Deserialize, ToSchema)]
pub struct CaptureInput {
    pub minutes: u32,
    #[serde(default)]
    pub include_text: bool,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ReportQuery {
    #[serde(default)]
    pub include_text: bool,
}

async fn config(
    state: &AppState,
    auth: &AuthUser,
    id: Uuid,
) -> ApiResult<transcription_api::ApiConfig> {
    auth.require_perm(Permission::DiagnosticsManage)?;
    let cfg =
        transcription_api::config(&state.db, auth.tenant, Some(&state.secrets), id, "").await?;
    if !cfg.is_coral() {
        return Err(ApiError::BadRequest("select a Coral connection".into()));
    }
    Ok(cfg)
}

async fn read_response(mut response: reqwest::Response) -> ApiResult<Value> {
    if !response.status().is_success() {
        return Err(ApiError::BadRequest(format!(
            "Coral diagnostics returned HTTP {}",
            response.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::BadRequest("Coral diagnostics response interrupted".into()))?
    {
        if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
            return Err(ApiError::BadRequest(
                "Coral diagnostics response exceeds 4 MiB".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::BadRequest("Coral diagnostics response is not JSON".into()))
}

fn request(
    cfg: &transcription_api::ApiConfig,
    method: reqwest::Method,
    path: &str,
) -> reqwest::RequestBuilder {
    let req = super::transcription::client()
        .request(method, format!("{}{path}", cfg.url.trim_end_matches('/')));
    match &cfg.key {
        Some(key) => req.bearer_auth(key),
        None => req,
    }
}

async fn send(req: reqwest::RequestBuilder) -> ApiResult<Value> {
    read_response(
        req.send().await.map_err(|_| {
            ApiError::BadRequest("Coral diagnostics service is not reachable".into())
        })?,
    )
    .await
}

#[utoipa::path(post, path = "/api/v1/diagnostics/coral/{id}/capture", tag = "system", params(("id" = Uuid, Path)), request_body = CaptureInput, responses((status = 200, body = Object)))]
pub async fn start_capture(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<CaptureInput>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    if !(1..=30).contains(&input.minutes) {
        return Err(ApiError::BadRequest(
            "capture minutes must be 1 to 30".into(),
        ));
    }
    if input.include_text {
        auth.require_perm(Permission::CallsLiveTranscriptAll)?;
    }
    let result = send(request(&cfg, reqwest::Method::POST, "/diagnostics/capture")
        .json(&json!({"scope": auth.tenant.to_string(), "minutes": input.minutes, "include_text": input.include_text}))).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "start_coral_capture",
        "diagnostics",
        Some(id.to_string()),
        json!({"minutes": input.minutes, "include_text": input.include_text}),
    )
    .await?;
    // Never return retained text in a control response.
    let mut result = result;
    sanitize(&mut result, cfg.key.as_deref(), false);
    Ok(Json(result))
}

#[utoipa::path(delete, path = "/api/v1/diagnostics/coral/{id}/capture", tag = "system", params(("id" = Uuid, Path)), responses((status = 200, body = Object)))]
pub async fn stop_capture(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let cfg = config(&state, &auth, id).await?;
    let mut result = send(request(
        &cfg,
        reqwest::Method::DELETE,
        &format!("/diagnostics/capture?scope={}", auth.tenant),
    ))
    .await?;
    sanitize(&mut result, cfg.key.as_deref(), false);
    audit::record(
        &state.db,
        &auth.actor(),
        "stop_coral_capture",
        "diagnostics",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    Ok(Json(result))
}

fn sanitize(value: &mut Value, key: Option<&str>, include_text: bool) {
    match value {
        Value::Object(map) => {
            map.retain(|name, _| {
                let field = name.replace(['-', '_'], "").to_ascii_lowercase();
                !matches!(
                    field.as_str(),
                    "key"
                        | "apikey"
                        | "authorization"
                        | "password"
                        | "secret"
                        | "token"
                        | "accesstoken"
                        | "audio"
                ) && (include_text
                    || !matches!(
                        field.as_str(),
                        "text"
                            | "transcript"
                            | "delta"
                            | "segments"
                            | "words"
                            | "prompt"
                            | "vocabulary"
                            | "keywords"
                    ))
            });
            for child in map.values_mut() {
                sanitize(child, key, include_text);
            }
        }
        Value::Array(items) => {
            for child in items {
                sanitize(child, key, include_text);
            }
        }
        Value::String(text) => {
            if let Some(key) = key.filter(|key| !key.is_empty()) {
                *text = text.replace(key, "[REDACTED]");
            }
        }
        _ => {}
    }
}

#[utoipa::path(get, path = "/api/v1/diagnostics/coral/{id}/download", tag = "system", params(("id" = Uuid, Path), ReportQuery), responses((status = 200, body = Object)))]
pub async fn download(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(query): Query<ReportQuery>,
) -> ApiResult<Response> {
    let cfg = config(&state, &auth, id).await?;
    if query.include_text {
        auth.require_perm(Permission::CallsLiveTranscriptAll)?;
    }
    let service = send(request(
        &cfg,
        reqwest::Method::GET,
        &format!("/diagnostics?scope={}", auth.tenant),
    ))
    .await?;
    let profiles = transcription_profiles::list(&state.db, auth.tenant).await?;
    let previews =
        talkops_core::live_transcripts::diagnostic_states(&state.db, auth.tenant).await?;
    let mut report = json!({"schema_version": 1, "talkops_version": talkops_core::VERSION,
        "talkops_build_revision": option_env!("TALKOPS_BUILD_REVISION"),
        "generated_at": chrono::Utc::now(), "provider_id": id, "profiles": profiles,
        "live_preview_states": previews, "coral": service, "includes_text": query.include_text,
        "system_load": talkops_core::system_load::sample(),
        "notes": ["Audio is not included. Capture is bounded and stops automatically.",
            "Fallback states are a snapshot. Capture errors and preview IDs help correlate failures.",
            "WER needs a separately corrected reference transcript. Event timing is not word-aligned ground truth."]});
    sanitize(&mut report, cfg.key.as_deref(), query.include_text);
    audit::record(
        &state.db,
        &auth.actor(),
        "export_coral_diagnostics",
        "diagnostics",
        Some(id.to_string()),
        json!({"include_text": query.include_text}),
    )
    .await?;
    let bytes =
        serde_json::to_vec_pretty(&report).map_err(|e| ApiError::Internal(e.to_string()))?;
    let mut response = bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"talkops-coral-diagnostics.json\""),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_redacts_nested_keys_and_text_without_consent() {
        let mut report = json!({"password": "pw", "apiKey":"key123", "transcript":"private", "events": [{"text": "private", "details": {"error": "key123 rejected", "authorization": "Bearer key123"}}]});
        sanitize(&mut report, Some("key123"), false);
        assert!(report.get("password").is_none());
        assert!(report.get("apiKey").is_none() && report.get("transcript").is_none());
        assert!(report["events"][0].get("text").is_none());
        assert_eq!(
            report["events"][0]["details"]["error"],
            "[REDACTED] rejected"
        );
    }
}
