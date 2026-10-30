//! Transcription settings (admin, ADR 0024): cloud connections, the engine
//! per purpose and the cloud usage. A Coral connection (ADR 0046) is tested
//! with `/health` (TPU status) and `/v1/models` of the coral-stt service.

use std::sync::OnceLock;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::transcription_api::{self, Provider, ProviderInput};
use talkops_core::transcription_profiles::{self, OPENAI_PRICES, Profile, Usage};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use talkops_core::roles::Permission;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(overview))
        .routes(routes!(update_profiles))
        .routes(routes!(create_provider))
        .routes(routes!(update_provider, delete_provider))
        .routes(routes!(test_provider))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelPrice {
    pub model: String,
    /// USD per audio minute.
    pub price_per_minute: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TranscriptionOverview {
    pub providers: Vec<Provider>,
    /// One per purpose: live, recording, recording_refine, voicemail,
    /// voicemail_refine.
    pub profiles: Vec<Profile>,
    /// Cloud usage of this and the previous month.
    pub usage: Vec<Usage>,
    /// OpenAI list prices, used when a profile sets no own price.
    pub openai_prices: Vec<ModelPrice>,
}

/// Connections, profiles and usage, without keys (admin).
#[utoipa::path(get, path = "/api/v1/settings/transcription", tag = "settings", responses((status = 200, body = TranscriptionOverview)))]
pub async fn overview(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<TranscriptionOverview>> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    Ok(Json(TranscriptionOverview {
        providers: transcription_api::list(&state.db, auth.tenant).await?,
        profiles: transcription_profiles::list(&state.db, auth.tenant).await?,
        usage: transcription_profiles::usage(&state.db, auth.tenant).await?,
        openai_prices: OPENAI_PRICES
            .iter()
            .map(|(model, price)| ModelPrice {
                model: (*model).into(),
                price_per_minute: *price,
            })
            .collect(),
    }))
}

/// Sets the engine of the given purposes (admin).
#[utoipa::path(put, path = "/api/v1/settings/transcription/profiles", tag = "settings", request_body = Vec<Profile>, responses((status = 200, body = Vec<Profile>)))]
pub async fn update_profiles(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<Vec<Profile>>,
) -> ApiResult<Json<Vec<Profile>>> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    let profiles = transcription_profiles::update(&state.db, auth.tenant, &input).await?;
    let summary: Vec<_> = profiles
        .iter()
        .map(|p| {
            json!({"purpose": p.purpose, "engine": p.engine, "local_model": p.local_model,
                   "provider_id": p.provider_id, "model": p.model, "realtime": p.realtime,
                   "coral_stable_preview": p.coral_stable_preview})
        })
        .collect();
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "transcription_profiles",
        None,
        json!({"profiles": summary}),
    )
    .await?;
    Ok(Json(profiles))
}

/// Adds a cloud connection (admin). The key is stored encrypted.
#[utoipa::path(post, path = "/api/v1/settings/transcription/providers", tag = "settings", request_body = ProviderInput, responses((status = 201, body = Provider)))]
pub async fn create_provider(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<ProviderInput>,
) -> ApiResult<(StatusCode, Json<Provider>)> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    let p = transcription_api::create(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "transcription_provider",
        Some(p.id.to_string()),
        json!({"name": p.name, "kind": p.kind, "url": p.url, "has_key": p.has_key}),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(p)))
}

/// Changes a cloud connection (admin); `key: null` keeps the key.
#[utoipa::path(put, path = "/api/v1/settings/transcription/providers/{id}", tag = "settings", params(("id" = Uuid, Path)), request_body = ProviderInput, responses((status = 200, body = Provider)))]
pub async fn update_provider(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Provider>> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    let p = transcription_api::update(&state.db, auth.tenant, &state.secrets, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "transcription_provider",
        Some(id.to_string()),
        json!({"name": p.name, "kind": p.kind, "url": p.url, "key_changed": input.key.is_some()}),
    )
    .await?;
    Ok(Json(p))
}

/// Deletes a cloud connection that no purpose uses (admin).
#[utoipa::path(delete, path = "/api/v1/settings/transcription/providers/{id}", tag = "settings", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_provider(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    let old = transcription_api::get(&state.db, auth.tenant, id).await?;
    transcription_api::delete(&state.db, auth.tenant, id)
        .await
        .map_err(|err| match err {
            // In use: the message names the purposes.
            talkops_core::error::CoreError::Conflict(msg) => ApiError::Conflict(msg),
            other => other.into(),
        })?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "transcription_provider",
        Some(id.to_string()),
        json!({"name": old.name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiTest {
    /// Models of this connection's profiles that the server does not offer;
    /// `null` if the server lists no models.
    pub missing_models: Option<Vec<String>>,
    /// Models that look like speech recognition (whisper, transcribe, voxtral).
    pub speech_models: Vec<String>,
    /// Coral and CUDA connections: the `/health` answer of coral-stt or
    /// cuda-stt (status, TPUs or GPU, loaded models, queue).
    #[schema(value_type = Option<Object>)]
    pub coral: Option<serde_json::Value>,
}

pub(super) fn client() -> &'static reqwest::Client {
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

/// Checks URL and key of a saved connection: lists the server's models
/// (`GET {url}/models`), without sending audio (admin).
#[utoipa::path(post, path = "/api/v1/settings/transcription/providers/{id}/test", tag = "settings", params(("id" = Uuid, Path)), responses((status = 200, body = ApiTest)))]
pub async fn test_provider(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiTest>> {
    auth.require_perm(Permission::TranscriptionSettings)?;
    let cfg =
        transcription_api::config(&state.db, auth.tenant, Some(&state.secrets), id, "").await?;
    // coral-stt and cuda-stt answer /health with 503 while no device or
    // model is ready; the body still tells why.
    let coral = if cfg.is_local() {
        let res = client()
            .get(format!("{}/health", cfg.root()))
            .send()
            .await
            .map_err(|e| {
                ApiError::BadRequest(format!("speech service not reachable: {}", root_cause(&e)))
            })?;
        Some(
            res.json::<serde_json::Value>().await.map_err(|_| {
                ApiError::BadRequest("the speech service gave no health status".into())
            })?,
        )
    } else {
        None
    };
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
    audit::record(
        &state.db,
        &auth.actor(),
        "test",
        "transcription_provider",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    if !status.is_success() {
        // Some servers have no model list; transcription may still work.
        return Ok(Json(ApiTest {
            missing_models: None,
            speech_models: Vec::new(),
            coral,
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
            [
                "whisper",
                "transcribe",
                "voxtral",
                "parakeet",
                "coral",
                "cuda",
            ]
            .iter()
            .any(|w| id.contains(w))
        })
        .cloned()
        .collect();
    speech.sort();
    let missing = if ids.is_empty() {
        None
    } else {
        let mut used: Vec<String> = transcription_profiles::list(&state.db, auth.tenant)
            .await?
            .into_iter()
            .filter(|p| p.engine == "cloud" && p.provider_id == Some(id))
            .map(|p| p.model)
            .filter(|m| !ids.contains(m))
            .collect();
        used.sort();
        used.dedup();
        Some(used)
    };
    Ok(Json(ApiTest {
        missing_models: missing,
        speech_models: speech,
        coral,
    }))
}

fn root_cause(err: &(dyn std::error::Error + 'static)) -> String {
    let mut e = err;
    while let Some(next) = e.source() {
        e = next;
    }
    e.to_string()
}
