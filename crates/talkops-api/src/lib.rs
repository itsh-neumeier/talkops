//! TalkOps control plane HTTP server.

pub mod auth;
pub mod config;
pub mod error;
pub mod esl;
pub mod fsxml;
pub mod routes;
pub mod telephony;
pub mod util;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::presets::PresetCatalog;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::auth::LoginLimiter;
use crate::fsxml::sofia::ProfileSettings;
use crate::telephony::Telephony;

/// Shared state handed to every request handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub telephony: Telephony,
    pub secrets: SecretBox,
    pub catalog: Arc<PresetCatalog>,
    pub profile: Arc<ProfileSettings>,
    pub xmlcurl_password: Arc<str>,
    pub limiter: Arc<LoginLimiter>,
}

impl AppState {
    pub fn new(
        db: PgPool,
        secrets: SecretBox,
        catalog: PresetCatalog,
        profile: ProfileSettings,
        xmlcurl_password: &str,
    ) -> Self {
        Self {
            db,
            telephony: Telephony::default(),
            secrets,
            catalog: Arc::new(catalog),
            profile: Arc::new(profile),
            xmlcurl_password: Arc::from(xmlcurl_password),
            limiter: Arc::new(LoginLimiter::default()),
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(title = "TalkOps API", description = "Control plane API of the TalkOps PBX. \
        Authenticate with POST /api/v1/auth/login; state-changing requests need the headers \
        `X-Requested-With: TalkOps` and `X-CSRF-Token` (from /api/v1/auth/me)."),
    tags(
        (name = "system", description = "Health and status"),
        (name = "auth", description = "Login, session and first-run setup"),
        (name = "users", description = "Users and roles"),
        (name = "extensions", description = "Extensions and devices"),
        (name = "trunks", description = "SIP trunks, accounts, numbers and presets"),
        (name = "settings", description = "Telephony settings, call log and audit log"),
    )
)]
pub struct ApiDoc;

/// Builds the API router together with its OpenAPI document.
pub fn api_router() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(routes::health::router())
        .merge(routes::auth::router())
        .merge(routes::users::router())
        .merge(routes::extensions::router())
        .merge(routes::trunks::router())
        .merge(routes::settings::router())
        .split_for_parts();
    (router, api)
}

/// Builds the full application router. The web UI is served from `web_dir`
/// as a single-page app (unknown paths fall back to `index.html`).
pub fn app(state: AppState, web_dir: Option<&Path>) -> Router {
    let (api, openapi) = api_router();
    let openapi = Arc::new(openapi);
    let mut router = Router::new()
        .merge(api)
        .route(
            "/api/v1/openapi.json",
            axum::routing::get(move || {
                let doc = openapi.clone();
                async move { axum::Json((*doc).clone()) }
            }),
        )
        .route("/fs/xml", axum::routing::post(routes::fs::xml_curl))
        .route("/fs/cdr", axum::routing::post(routes::fs::xml_cdr))
        .with_state(state);

    if let Some(dir) = web_dir.filter(|d| d.join("index.html").is_file()) {
        let spa = ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")));
        router = router.fallback_service(spa);
    }

    router.layer(TraceLayer::new_for_http())
}
