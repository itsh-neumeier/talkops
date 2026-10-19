//! Telephony settings, live status, call log and audit log.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderName;
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::cdr::{self, Cdr, CdrQuery, CdrScope};
use talkops_core::extensions;
use talkops_core::settings::{self, TenantSettings};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::active_calls::{Directory, Party};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::telephony::LiveStatus;
use talkops_core::roles::Permission;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_settings, update_settings))
        .routes(routes!(telephony_status))
        .routes(routes!(list_calls))
}

/// Telephony settings (any logged-in user may read them).
#[utoipa::path(get, path = "/api/v1/settings", tag = "settings", responses((status = 200, body = Object)))]
pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<TenantSettings>> {
    Ok(Json(settings::get(&state.db, auth.tenant).await?))
}

fn validate(s: &TenantSettings) -> Result<(), ApiError> {
    let digits = |v: &str| v.bytes().all(|b| b.is_ascii_digit());
    if !digits(&s.country_code) || s.country_code.is_empty() || !digits(&s.area_code) {
        return Err(ApiError::BadRequest(
            "country and area code must be digits".into(),
        ));
    }
    if s.emergency_numbers.is_empty()
        || s.emergency_numbers
            .iter()
            .any(|n| n.is_empty() || !digits(n))
    {
        return Err(ApiError::BadRequest(
            "at least one emergency number (digits) is required".into(),
        ));
    }
    let ip_ok = s.external_ip.is_empty()
        || s.external_ip.parse::<std::net::IpAddr>().is_ok()
        || s.external_ip.strip_prefix("stun:").is_some_and(|h| {
            !h.is_empty()
                && h.chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-:".contains(c))
        });
    if !s.hold_music.is_empty() && talkops_core::audio::music_file(&s.hold_music).is_none() {
        return Err(ApiError::BadRequest("unknown hold music".into()));
    }
    if s.transcription_vocabulary.chars().count() > settings::MAX_VOCABULARY
        || s.transcription_vocabulary.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest(format!(
            "vocabulary: at most {} characters on one line",
            settings::MAX_VOCABULARY
        )));
    }
    if !ip_ok {
        return Err(ApiError::BadRequest(
            "external IP must be an IP address or stun:host[:port]".into(),
        ));
    }
    Ok(())
}

/// Updates telephony settings (admin). Changing the external IP restarts the
/// trunk profile, which interrupts active external calls.
#[utoipa::path(put, path = "/api/v1/settings", tag = "settings", request_body = Object, responses((status = 200, body = Object)))]
pub async fn update_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TenantSettings>,
) -> ApiResult<Json<TenantSettings>> {
    auth.require_perm(Permission::SettingsManage)?;
    validate(&input)?;
    talkops_core::audio::ensure_exists(&state.db, auth.tenant, input.hold_music_clip_id).await?;
    let before = settings::get(&state.db, auth.tenant).await?;
    let after = settings::update(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "settings",
        None,
        serde_json::to_value(&after).unwrap_or(json!({})),
    )
    .await?;
    if before.external_ip != after.external_ip {
        let telephony = state.telephony.clone();
        tokio::spawn(async move { telephony.restart_external_profile().await });
    }
    Ok(Json(after))
}

/// Live registrations and trunk states (operator or admin).
#[utoipa::path(get, path = "/api/v1/telephony/status", tag = "settings", responses((status = 200, body = LiveStatus)))]
pub async fn telephony_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<LiveStatus>> {
    auth.require_perm(Permission::CallsActive)?;
    Ok(Json(state.telephony.snapshot().await))
}

/// A call record with its resolved parties.
#[derive(Serialize, utoipa::ToSchema)]
pub struct CallEntry {
    #[serde(flatten)]
    pub cdr: Cdr,
    /// Caller: an extension, or an external number with its phone book name.
    pub caller: Party,
    /// Called party; for answered inbound calls the answering extension.
    pub called: Party,
    /// Own number (E.164): for inbound calls the number that the caller
    /// dialed, for outbound calls the number that supplied the caller ID.
    pub via_number: Option<String>,
    /// Current membership, only returned to whitelist managers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowlisted: Option<bool>,
}

/// Call log, filtered and sorted. Operators and admins see all calls, users
/// only their own extensions. `X-Total-Count` has the number of matches.
#[utoipa::path(get, path = "/api/v1/calls", tag = "settings", params(CdrQuery), responses((status = 200, body = [CallEntry], headers(("X-Total-Count" = i64, description = "Number of matching calls")))))]
pub async fn list_calls(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<CdrQuery>,
) -> ApiResult<([(HeaderName, String); 1], Json<Vec<CallEntry>>)> {
    let directory = Directory::load(&state, auth.tenant).await?;
    let scope = CdrScope {
        extensions: if auth.can(Permission::CallsViewAll) {
            None
        } else {
            Some(
                extensions::list_for_user(&state.db, auth.tenant, auth.id)
                    .await?
                    .into_iter()
                    .map(|e| e.id)
                    .collect(),
            )
        },
        from_numbers: query
            .from
            .as_deref()
            .map(|t| directory.contact_numbers(t))
            .unwrap_or_default(),
        to_numbers: query
            .to
            .as_deref()
            .map(|t| directory.contact_numbers(t))
            .unwrap_or_default(),
    };
    let (rows, total) = tokio::try_join!(
        cdr::list_scoped(&state.db, auth.tenant, &query, &scope),
        cdr::count(&state.db, auth.tenant, &query, &scope)
    )?;
    let allowed = if auth.can(Permission::BlockingAllowlist) {
        Some(
            talkops_core::blocking::allowed_numbers(
                &state.db,
                auth.tenant,
                &rows
                    .iter()
                    .map(|c| c.caller_number.clone())
                    .collect::<Vec<_>>(),
            )
            .await?
            .into_iter()
            .collect::<std::collections::HashSet<_>>(),
        )
    } else {
        None
    };
    let entries = rows
        .into_iter()
        .map(|cdr| {
            let (caller, called, via_number) = directory.record_parties(&cdr);
            CallEntry {
                allowlisted: allowed
                    .as_ref()
                    .map(|numbers| numbers.contains(&cdr.caller_number)),
                cdr,
                caller,
                called,
                via_number,
            }
        })
        .collect();
    Ok((
        [(HeaderName::from_static("x-total-count"), total.to_string())],
        Json(entries),
    ))
}
