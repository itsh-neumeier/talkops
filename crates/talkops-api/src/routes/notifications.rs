//! Mail notifications: rules per event type, test mails and the log
//! (ADR 0035).

use axum::Json;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::notifications::{self as n, Delivery, Event, EventDef, Rule, RuleInput};
use talkops_core::roles::Permission;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::notifier::{self, Kind};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_notifications))
        .routes(routes!(save_rule, reset_rule))
        .routes(routes!(test_rule))
        .routes(routes!(get_log))
}

/// An event type of the catalog.
#[derive(Debug, Serialize, ToSchema)]
pub struct EventTypeView {
    pub key: &'static str,
    pub category: &'static str,
    #[schema(value_type = String)]
    pub severity: n::Severity,
    /// Problem events: the event that ends the problem.
    pub recovery: Option<&'static str>,
    /// False: no producer yet (shown as "planned").
    pub implemented: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationsView {
    pub smtp_configured: bool,
    pub categories: Vec<&'static str>,
    /// Configurable event types (no recovery events), in display order.
    pub event_types: Vec<EventTypeView>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationLog {
    pub events: Vec<Event>,
    pub deliveries: Vec<Delivery>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TestResult {
    /// `sent`, `failed`, `no_recipients` or `no_smtp`.
    pub status: String,
    pub recipients: Vec<String>,
    pub error: Option<String>,
}

fn def_or_404(key: &str) -> ApiResult<&'static EventDef> {
    n::def(key)
        .filter(|d| !d.is_recovery)
        .ok_or(ApiError::NotFound)
}

/// Event catalog and rules (admin).
#[utoipa::path(get, path = "/api/v1/settings/notifications", tag = "settings", responses((status = 200, body = NotificationsView)))]
pub async fn get_notifications(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<NotificationsView>> {
    auth.require_perm(Permission::NotificationsManage)?;
    let mut event_types: Vec<EventTypeView> = n::CATALOG
        .iter()
        .filter(|d| !d.is_recovery)
        .map(|d| EventTypeView {
            key: d.key,
            category: d.category,
            severity: d.severity,
            recovery: d.recovery,
            implemented: d.implemented,
        })
        .collect();
    let order = |c: &str| n::CATEGORIES.iter().position(|x| *x == c).unwrap_or(99);
    event_types.sort_by_key(|e| order(e.category));
    Ok(Json(NotificationsView {
        smtp_configured: !talkops_core::mail::get(&state.db, auth.tenant)
            .await?
            .host
            .is_empty(),
        categories: n::CATEGORIES.to_vec(),
        event_types,
        rules: n::rules(&state.db, auth.tenant).await?,
    }))
}

/// Saves the rule of an event type (admin).
#[utoipa::path(put, path = "/api/v1/settings/notifications/rules/{event_type}", tag = "settings", params(("event_type" = String, Path)), request_body = RuleInput, responses((status = 200, body = Rule)))]
pub async fn save_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(event_type): Path<String>,
    Json(input): Json<RuleInput>,
) -> ApiResult<Json<Rule>> {
    auth.require_perm(Permission::NotificationsManage)?;
    let d = def_or_404(&event_type)?;
    let rule = n::save_rule(&state.db, auth.tenant, d, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "notification_rule",
        Some(d.key.to_owned()),
        json!({"enabled": rule.enabled, "min_severity": rule.min_severity,
               "users": rule.recipient_users.len(), "roles": rule.recipient_roles.len(),
               "emails": rule.recipient_emails.len(), "delay_secs": rule.delay_secs,
               "send_recovery": rule.send_recovery, "digest_secs": rule.digest_secs,
               "rate_limit_per_hour": rule.rate_limit_per_hour}),
    )
    .await?;
    Ok(Json(rule))
}

/// Resets the rule of an event type to the defaults (admin).
#[utoipa::path(delete, path = "/api/v1/settings/notifications/rules/{event_type}", tag = "settings", params(("event_type" = String, Path)), responses((status = 200, body = Rule)))]
pub async fn reset_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(event_type): Path<String>,
) -> ApiResult<Json<Rule>> {
    auth.require_perm(Permission::NotificationsManage)?;
    let d = def_or_404(&event_type)?;
    let rule = n::reset_rule(&state.db, auth.tenant, d).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "reset",
        "notification_rule",
        Some(d.key.to_owned()),
        json!({}),
    )
    .await?;
    Ok(Json(rule))
}

/// Sends a test notification for an event type to the recipients of its
/// stored rule, with sample values (admin). The rate limit does not apply.
#[utoipa::path(post, path = "/api/v1/settings/notifications/rules/{event_type}/test", tag = "settings", params(("event_type" = String, Path)), responses((status = 200, body = TestResult)))]
pub async fn test_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(event_type): Path<String>,
) -> ApiResult<Json<TestResult>> {
    auth.require_perm(Permission::NotificationsManage)?;
    let d = def_or_404(&event_type)?;
    let rule = n::rule(&state.db, auth.tenant, d).await?;
    let sample = Event {
        id: 0,
        tenant_id: auth.tenant,
        event_type: d.key.to_owned(),
        source: String::new(),
        severity: d.severity.as_str().to_owned(),
        params: json!({"name": "Test", "extension": "21", "number": "112",
                       "call_uuid": "00000000-0000-4000-8000-000000000021",
                       "status": "Test dialing attempt (no call)",
                       "user": "test", "ip": "192.0.2.1", "error": "test",
                       "entity": "test", "action": "test"}),
        created_at: Utc::now(),
    };
    let outcome = notifier::deliver(
        &state.db,
        &state.secrets,
        auth.tenant,
        &rule,
        d,
        Kind::Test,
        std::slice::from_ref(&sample),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    audit::record(
        &state.db,
        &auth.actor(),
        "test",
        "notification_rule",
        Some(d.key.to_owned()),
        json!({"status": outcome.status, "recipients": outcome.recipients.len()}),
    )
    .await?;
    Ok(Json(TestResult {
        status: outcome.status.to_owned(),
        recipients: outcome.recipients,
        error: outcome.error,
    }))
}

/// Recent events and deliveries (admin).
#[utoipa::path(get, path = "/api/v1/settings/notifications/log", tag = "settings", responses((status = 200, body = NotificationLog)))]
pub async fn get_log(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<NotificationLog>> {
    auth.require_perm(Permission::NotificationsManage)?;
    Ok(Json(NotificationLog {
        events: n::recent_events(&state.db, auth.tenant, 100).await?,
        deliveries: n::deliveries(&state.db, auth.tenant, 100).await?,
    }))
}
