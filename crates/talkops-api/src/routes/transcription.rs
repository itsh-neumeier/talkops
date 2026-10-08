//! Settings of the OpenAI-compatible transcription API (admin).

use std::sync::OnceLock;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::transcription_api::{self, TranscriptionApi, TranscriptionApiInput};
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_api, update_api))
        .routes(routes!(test_api))
}

/// Transcription API settings, without the key (admin).
#[utoipa::path(get, path = "/api/v1/settings/transcription-api", tag = "settings", responses((status = 200, body = TranscriptionApi)))]
pub async fn get_api(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<TranscriptionApi>> {
    auth.require(Role::Admin)?;
    Ok(Json(transcription_api::get(&state.db, auth.tenant).await?))
}

/// Changes the transcription API (admin). The key is stored encrypted.
#[utoipa::path(put, path = "/api/v1/settings/transcription-api", tag = "settings", request_body = TranscriptionApiInput, responses((status = 200, body = TranscriptionApi)))]
pub async fn update_api(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TranscriptionApiInput>,
) -> ApiResult<Json<TranscriptionApi>> {
    auth.require(Role::Admin)?;
    let s = transcription_api::update(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "transcription_api",
        None,
        json!({"url": s.url, "model": s.model, "key_changed": input.key.is_some()}),
    )
    .await?;
    Ok(Json(s))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiTest {
    /// The configured model is offered by the server (if it lists models).
    pub model_found: Option<bool>,
    /// Models that look like speech recognition (whisper, transcribe, voxtral).
    pub speech_models: Vec<String>,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        talkops_doorbell::ensure_tls_provider();
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(format!("TalkOps/{}", talkops_core::VERSION))
            .build()
            .expect("HTTP client")
    })
}

/// Checks URL and key with the saved settings: lists the server's models
/// (`GET {url}/models`), without sending audio (admin).
#[utoipa::path(post, path = "/api/v1/settings/transcription-api/test", tag = "settings", responses((status = 200, body = ApiTest)))]
pub async fn test_api(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<ApiTest>> {
    auth.require(Role::Admin)?;
    let cfg = transcription_api::config(&state.db, auth.tenant, &state.secrets).await?;
    let mut req = client().get(format!("{}/models", cfg.url.trim_end_matches('/')));
    if let Some(key) = &cfg.key {
        req = req.bearer_auth(key);
    }
    let res = req
        .send()
        .await
        .map_err(|e| ApiError::BadRequest(format!("not reachable: {}", root_cause(&e))))?;
    let status = res.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ApiError::BadRequest(format!(
            "API key rejected (HTTP {})",
            status.as_u16()
        )));
    }
    if !status.is_success() {
        // Some servers have no model list; transcription may still work.
        return Ok(Json(ApiTest {
            model_found: None,
            speech_models: Vec::new(),
        }));
    }
    #[derive(serde::Deserialize)]
    struct Models {
        data: Vec<Model>,
    }
    #[derive(serde::Deserialize)]
    struct Model {
        id: String,
    }
    let ids: Vec<String> = res
        .json::<Models>()
        .await
        .map(|m| m.data.into_iter().map(|m| m.id).collect())
        .unwrap_or_default();
    let mut speech: Vec<String> = ids
        .iter()
        .filter(|id| {
            let id = id.to_ascii_lowercase();
            ["whisper", "transcribe", "voxtral", "parakeet"]
                .iter()
                .any(|w| id.contains(w))
        })
        .cloned()
        .collect();
    speech.sort();
    audit::record(
        &state.db,
        &auth.actor(),
        "test",
        "transcription_api",
        None,
        json!({}),
    )
    .await?;
    Ok(Json(ApiTest {
        model_found: (!ids.is_empty()).then(|| ids.contains(&cfg.model)),
        speech_models: speech,
    }))
}

fn root_cause(err: &(dyn std::error::Error + 'static)) -> String {
    let mut e = err;
    while let Some(next) = e.source() {
        e = next;
    }
    e.to_string()
}
