//! TalkOps control plane HTTP server.

pub mod auth;
pub mod callcenter;
pub mod config;
pub mod doors;
pub mod error;
pub mod esl;
pub mod fsxml;
pub mod mailer;
pub mod menu;
pub mod retention;
pub mod routes;
pub mod telephony;
pub mod util;
pub mod voicemail;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::presets::PresetCatalog;
use talkops_provisioning::PhoneCatalog;
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
    pub phone_catalog: Arc<PhoneCatalog>,
    /// Writable provisioning data directory (firmware images).
    pub provisioning_dir: Arc<PathBuf>,
    pub media: Arc<MediaPaths>,
    /// `host:port` FreeSWITCH connects to for interactive calls (`socket`).
    pub outbound_socket: Arc<str>,
    /// Wakes the queue (mod_callcenter) sync after changes.
    pub queue_sync: Arc<tokio::sync::Notify>,
    pub profile: Arc<ProfileSettings>,
    pub xmlcurl_password: Arc<str>,
    pub limiter: Arc<LoginLimiter>,
}

impl AppState {
    pub fn new(
        db: PgPool,
        secrets: SecretBox,
        catalog: PresetCatalog,
        phone_catalog: PhoneCatalog,
        provisioning_dir: PathBuf,
        profile: ProfileSettings,
        xmlcurl_password: &str,
    ) -> Self {
        Self {
            db,
            telephony: Telephony::default(),
            secrets,
            catalog: Arc::new(catalog),
            phone_catalog: Arc::new(phone_catalog),
            provisioning_dir: Arc::new(provisioning_dir),
            media: Arc::new(MediaPaths::default()),
            outbound_socket: Arc::from("127.0.0.1:8084"),
            queue_sync: Arc::new(tokio::sync::Notify::new()),
            profile: Arc::new(profile),
            xmlcurl_password: Arc::from(xmlcurl_password),
            limiter: Arc::new(LoginLimiter::default()),
        }
    }
}

/// Shared media volumes (same paths in the FreeSWITCH container).
#[derive(Debug, Clone)]
pub struct MediaPaths {
    pub voicemail: PathBuf,
    pub sounds: PathBuf,
    pub recordings: PathBuf,
    /// Door station snapshots.
    pub snapshots: PathBuf,
}

impl Default for MediaPaths {
    fn default() -> Self {
        Self {
            voicemail: PathBuf::from("/var/lib/talkops/voicemail"),
            sounds: PathBuf::from("/var/lib/talkops/sounds"),
            recordings: PathBuf::from("/var/lib/talkops/recordings"),
            snapshots: PathBuf::from("/var/lib/talkops/snapshots"),
        }
    }
}

impl AppState {
    pub fn with_media(mut self, media: MediaPaths) -> Self {
        self.media = Arc::new(media);
        self
    }

    pub fn with_queue_sync(mut self, trigger: Arc<tokio::sync::Notify>) -> Self {
        self.queue_sync = trigger;
        self
    }

    pub fn with_outbound_socket(mut self, addr: &str) -> Self {
        self.outbound_socket = Arc::from(addr);
        self
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
        (name = "phones", description = "Provisioned phones, firmware and phonebook"),
        (name = "voicemail", description = "Voicemail boxes and messages"),
        (name = "routing", description = "Ring groups, time conditions, menus and queues"),
        (name = "recordings", description = "Call recordings, transcripts and search"),
        (name = "doors", description = "Door stations: opener, live picture, events"),
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
        .merge(routes::phones::router())
        .merge(routes::voicemail::router())
        .merge(routes::groups::router())
        .merge(routes::time_conditions::router())
        .merge(routes::ivr::router())
        .merge(routes::queues::router())
        .merge(routes::recordings::router())
        .merge(routes::doors::router())
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
        .route(
            "/hooks/door/{id}/open",
            axum::routing::post(routes::doors::hook_open),
        )
        .route(
            "/provisioning/{*path}",
            axum::routing::get(routes::provisioning::serve),
        )
        .with_state(state);

    if let Some(dir) = web_dir.filter(|d| d.join("index.html").is_file()) {
        let spa = ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")));
        router = router.fallback_service(spa);
    }

    router.layer(TraceLayer::new_for_http())
}
