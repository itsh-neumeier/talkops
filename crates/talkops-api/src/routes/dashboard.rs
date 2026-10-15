//! Dashboard data: call statistics and calls in progress.

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use talkops_core::extensions;
use talkops_core::settings;
use talkops_core::stats::{self, CallStats, Range};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::active_calls::{self, OverviewCall};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use talkops_core::roles::Permission;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(call_stats))
        .routes(routes!(active_calls))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct StatsQuery {
    /// `1h`, `1d` (default), `1w` or `1m`.
    #[param(value_type = Option<String>)]
    pub range: Option<Range>,
}

/// Calls per time slot by direction, missed calls and answer rate
/// (operator or admin).
#[utoipa::path(get, path = "/api/v1/stats/calls", tag = "system", params(StatsQuery), responses((status = 200, body = CallStats)))]
pub async fn call_stats(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<StatsQuery>,
) -> ApiResult<Json<CallStats>> {
    auth.require_perm(Permission::CallsActive)?;
    let zone = settings::get(&state.db, auth.tenant).await?.timezone;
    Ok(Json(
        stats::calls(
            &state.db,
            auth.tenant,
            q.range.unwrap_or(Range::Day),
            chrono::Utc::now(),
            &zone,
        )
        .await?,
    ))
}

/// Calls in progress (operator or admin); empty without FreeSWITCH.
#[utoipa::path(get, path = "/api/v1/telephony/calls", tag = "system", responses((status = 200, body = [OverviewCall])))]
pub async fn active_calls(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<OverviewCall>>> {
    auth.require_perm(Permission::CallsActive)?;
    let mut calls = active_calls::list(&state, auth.tenant).await?;
    if !auth.can(Permission::CallsLiveTranscriptAll) {
        let own = extensions::list_for_user(&state.db, auth.tenant, auth.id).await?;
        for call in &mut calls {
            call.transcript_available &= own.iter().any(|e| {
                call.caller.extension_id == Some(e.id) || call.called.extension_id == Some(e.id)
            });
        }
    }
    Ok(Json(calls))
}
