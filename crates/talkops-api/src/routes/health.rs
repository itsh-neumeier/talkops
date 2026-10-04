//! Liveness, readiness and component status.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(healthz))
        .routes(routes!(readyz))
        .routes(routes!(status))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Health {
    pub status: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Component {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Status {
    pub version: &'static str,
    pub database: Component,
    pub freeswitch: Component,
}

/// Liveness: the process is running and serving HTTP.
#[utoipa::path(get, path = "/healthz", tag = "system", responses((status = 200, body = Health)))]
pub async fn healthz() -> Json<Health> {
    Json(Health { status: "ok" })
}

/// Readiness: the database is reachable. FreeSWITCH is deliberately not part
/// of readiness because FreeSWITCH itself depends on TalkOps being ready.
#[utoipa::path(
    get, path = "/readyz", tag = "system",
    responses((status = 200, body = Health), (status = 503, body = Health))
)]
pub async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    match talkops_core::db::ping(&state.db).await {
        Ok(()) => (StatusCode::OK, Json(Health { status: "ready" })),
        Err(err) => {
            tracing::warn!(error = %err, "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Health {
                    status: "database unavailable",
                }),
            )
        }
    }
}

/// Status of TalkOps and its dependencies.
#[utoipa::path(get, path = "/api/v1/status", tag = "system", responses((status = 200, body = Status)))]
pub async fn status(State(state): State<AppState>) -> Json<Status> {
    let database = match talkops_core::db::ping(&state.db).await {
        Ok(()) => Component {
            ok: true,
            detail: None,
        },
        Err(_) => Component {
            ok: false,
            detail: Some("unreachable".into()),
        },
    };
    let freeswitch = match state.telephony.esl.get().await {
        None => Component {
            ok: false,
            detail: Some("event socket not connected".into()),
        },
        Some(client) => match client.api("status").await {
            Ok(out) => Component {
                ok: true,
                detail: out.lines().next().map(str::to_owned),
            },
            Err(err) => Component {
                ok: false,
                detail: Some(err.to_string()),
            },
        },
    };
    Json(Status {
        version: talkops_core::VERSION,
        database,
        freeswitch,
    })
}
