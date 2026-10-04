//! TalkOps control plane HTTP server.

pub mod config;
pub mod esl;
pub mod routes;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::esl::EslHandle;

/// Shared state handed to every request handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub esl: EslHandle,
    pub xmlcurl_password: Arc<str>,
}

#[derive(OpenApi)]
#[openapi(
    info(title = "TalkOps API", description = "Control plane API of the TalkOps PBX"),
    paths(routes::health::healthz, routes::health::readyz, routes::health::status),
    components(schemas(routes::health::Health, routes::health::Status, routes::health::Component)),
    tags((name = "system", description = "Health and status"))
)]
pub struct ApiDoc;

/// Builds the full application router. The web UI is served from `web_dir`
/// as a single-page app (unknown paths fall back to `index.html`).
pub fn app(state: AppState, web_dir: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/status", axum::routing::get(routes::health::status))
        .route(
            "/openapi.json",
            axum::routing::get(|| async { axum::Json(ApiDoc::openapi()) }),
        );

    let mut router = Router::new()
        .route("/healthz", axum::routing::get(routes::health::healthz))
        .route("/readyz", axum::routing::get(routes::health::readyz))
        .route("/fs/xml", axum::routing::post(routes::fs_xml::handle))
        .nest("/api/v1", api)
        .with_state(state);

    if let Some(dir) = web_dir.filter(|d| d.join("index.html").is_file()) {
        let spa = ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")));
        router = router.fallback_service(spa);
    }

    router.layer(TraceLayer::new_for_http())
}
