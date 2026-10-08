//! Troubleshooting (admin): FreeSWITCH log capture with SIP trace, active
//! channels, and restarting the services.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::users::Role;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::diagnostics::{self, CaptureStatus, Channel, DiagnosticsError, LogLine};
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_diagnostics))
        .routes(routes!(start_capture, stop_capture))
        .routes(routes!(log_lines, clear_log))
        .routes(routes!(download_log))
        .routes(routes!(restart))
        .routes(routes!(reset_sessions))
}

impl From<DiagnosticsError> for ApiError {
    fn from(err: DiagnosticsError) -> Self {
        ApiError::Internal(err.to_string())
    }
}

#[derive(Serialize, ToSchema)]
pub struct DiagnosticsView {
    pub version: &'static str,
    pub server_started_at: DateTime<Utc>,
    /// FreeSWITCH `status` output; `None` when not connected.
    pub freeswitch: Option<String>,
    pub capture: CaptureStatus,
    pub channels: Vec<Channel>,
}

/// Versions, uptime, capture state and active call legs.
#[utoipa::path(get, path = "/api/v1/diagnostics", tag = "system", responses((status = 200, body = DiagnosticsView)))]
pub async fn get_diagnostics(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<DiagnosticsView>> {
    auth.require(Role::Admin)?;
    let (freeswitch, channels) = match state.telephony.esl.get().await {
        Some(client) => {
            let status = client.api("status").await.ok();
            let channels = client
                .api("show channels as json")
                .await
                .map(|j| diagnostics::parse_channels(&j))
                .unwrap_or_default();
            (status, channels)
        }
        None => (None, Vec::new()),
    };
    Ok(Json(DiagnosticsView {
        version: talkops_core::VERSION,
        server_started_at: state.diagnostics.started_at,
        freeswitch,
        capture: state.diagnostics.status(),
        channels,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct CaptureRequest {
    /// `debug`, `info`, `notice` or `warning`.
    #[serde(default = "default_level")]
    pub level: String,
    /// Log all SIP messages (`sofia global siptrace on`).
    #[serde(default)]
    pub sip_trace: bool,
    /// 1–60; the capture stops by itself.
    #[serde(default = "default_minutes")]
    pub minutes: u32,
}

fn default_level() -> String {
    "debug".into()
}

fn default_minutes() -> u32 {
    10
}

/// Starts capturing the FreeSWITCH log (replaces a running capture and
/// clears the buffer's end marker, not its lines).
#[utoipa::path(post, path = "/api/v1/diagnostics/capture", tag = "system", request_body = CaptureRequest, responses((status = 200, body = CaptureStatus)))]
pub async fn start_capture(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CaptureRequest>,
) -> ApiResult<Json<CaptureStatus>> {
    auth.require(Role::Admin)?;
    if !diagnostics::LEVELS.contains(&req.level.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "level must be one of {}",
            diagnostics::LEVELS.join(", ")
        )));
    }
    if !(1..=diagnostics::MAX_MINUTES).contains(&req.minutes) {
        return Err(ApiError::BadRequest(format!(
            "minutes must be 1 to {}",
            diagnostics::MAX_MINUTES
        )));
    }
    let status = state
        .diagnostics
        .start(&req.level, req.sip_trace, req.minutes)
        .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "start_capture",
        "diagnostics",
        None,
        json!({"level": req.level, "sip_trace": req.sip_trace, "minutes": req.minutes}),
    )
    .await?;
    Ok(Json(status))
}

/// Stops the running capture; the buffer stays for download.
#[utoipa::path(delete, path = "/api/v1/diagnostics/capture", tag = "system", responses((status = 200, body = CaptureStatus)))]
pub async fn stop_capture(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<CaptureStatus>> {
    auth.require(Role::Admin)?;
    state.diagnostics.stop();
    audit::record(
        &state.db,
        &auth.actor(),
        "stop_capture",
        "diagnostics",
        None,
        json!({}),
    )
    .await?;
    Ok(Json(state.diagnostics.status()))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct LogQuery {
    /// Only lines after this sequence number.
    #[serde(default)]
    pub after: u64,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    2000
}

#[derive(Serialize, ToSchema)]
pub struct LogPage {
    pub lines: Vec<LogLine>,
    pub capture: CaptureStatus,
}

/// Captured lines for the live view (poll with `after` = last seen `seq`).
#[utoipa::path(get, path = "/api/v1/diagnostics/log", tag = "system", params(LogQuery), responses((status = 200, body = LogPage)))]
pub async fn log_lines(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<LogQuery>,
) -> ApiResult<Json<LogPage>> {
    auth.require(Role::Admin)?;
    Ok(Json(LogPage {
        lines: state.diagnostics.lines_after(q.after, q.limit.min(5000)),
        capture: state.diagnostics.status(),
    }))
}

/// Empties the buffer.
#[utoipa::path(delete, path = "/api/v1/diagnostics/log", tag = "system", responses((status = 204)))]
pub async fn clear_log(State(state): State<AppState>, auth: AuthUser) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    state.diagnostics.clear();
    audit::record(
        &state.db,
        &auth.actor(),
        "clear_log",
        "diagnostics",
        None,
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The whole buffer as a text file, to attach to a support request.
#[utoipa::path(get, path = "/api/v1/diagnostics/log.txt", tag = "system", responses((status = 200, content_type = "text/plain")))]
pub async fn download_log(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Response> {
    auth.require(Role::Admin)?;
    let name = format!(
        "talkops-freeswitch-{}.log",
        Utc::now().format("%Y%m%d-%H%M%S")
    );
    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        state.diagnostics.dump(),
    )
        .into_response())
}

#[derive(Deserialize, ToSchema)]
pub struct RestartRequest {
    /// `freeswitch` or `all` (FreeSWITCH, media worker and this server).
    pub target: String,
}

/// Restarts services: they exit and Docker's restart policy starts them
/// again. Active calls are dropped.
#[utoipa::path(post, path = "/api/v1/diagnostics/restart", tag = "system", request_body = RestartRequest, responses((status = 202)))]
pub async fn restart(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<RestartRequest>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let all = match req.target.as_str() {
        "freeswitch" => false,
        "all" => true,
        _ => {
            return Err(ApiError::BadRequest(
                "target must be freeswitch or all".into(),
            ));
        }
    };
    let client = state
        .telephony
        .esl
        .get()
        .await
        .ok_or_else(|| ApiError::Internal("FreeSWITCH is not connected".into()))?;
    audit::record(
        &state.db,
        &auth.actor(),
        "restart",
        "system",
        Some(req.target.clone()),
        json!({}),
    )
    .await?;
    tracing::warn!(target = %req.target, user = %auth.username, "restart requested");
    state.diagnostics.stop();
    // FreeSWITCH exits; `restart: unless-stopped` brings it back.
    client
        .bgapi("fsctl shutdown")
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if all {
        sqlx::query("SELECT pg_notify($1, 'restart')")
            .bind(diagnostics::CONTROL_CHANNEL)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        tokio::spawn(async {
            // Let the response reach the browser first.
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            tracing::warn!("exiting for restart");
            std::process::exit(0);
        });
    }
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize, ToSchema)]
pub struct SessionsRequest {
    /// `hangup` (end every call with a BYE), `reregister` (sign all trunks
    /// off and on again) or `all` (both).
    pub action: String,
}

#[derive(Serialize, ToSchema)]
pub struct SessionsResult {
    /// Call legs ended.
    pub hung_up: usize,
    pub reregistered: bool,
}

/// Clears sessions with the providers: ends all calls (FreeSWITCH sends a
/// BYE for every leg, so the provider releases them too) and/or signs the
/// trunks off and on again.
#[utoipa::path(post, path = "/api/v1/diagnostics/sessions", tag = "system", request_body = SessionsRequest, responses((status = 200, body = SessionsResult)))]
pub async fn reset_sessions(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<SessionsRequest>,
) -> ApiResult<Json<SessionsResult>> {
    auth.require(Role::Admin)?;
    let (hangup, reregister) = match req.action.as_str() {
        "hangup" => (true, false),
        "reregister" => (false, true),
        "all" => (true, true),
        _ => {
            return Err(ApiError::BadRequest(
                "action must be hangup, reregister or all".into(),
            ));
        }
    };
    let client = state
        .telephony
        .esl
        .get()
        .await
        .ok_or_else(|| ApiError::Internal("FreeSWITCH is not connected".into()))?;
    let esl = |e: talkops_esl::EslError| ApiError::Internal(e.to_string());
    let mut hung_up = 0;
    if hangup {
        hung_up = client
            .api("show channels as json")
            .await
            .map(|j| diagnostics::parse_channels(&j).len())
            .unwrap_or(0);
        client.api("hupall MANAGER_REQUEST").await.map_err(esl)?;
    }
    if reregister {
        // REGISTER with Expires: 0, then a fresh registration.
        client
            .api("sofia profile external unregister all")
            .await
            .map_err(esl)?;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        client
            .api("sofia profile external register all")
            .await
            .map_err(esl)?;
    }
    audit::record(
        &state.db,
        &auth.actor(),
        "reset_sessions",
        "system",
        Some(req.action.clone()),
        json!({"hung_up": hung_up}),
    )
    .await?;
    tracing::warn!(action = %req.action, hung_up, user = %auth.username, "sessions reset");
    Ok(Json(SessionsResult {
        hung_up,
        reregistered: reregister,
    }))
}
