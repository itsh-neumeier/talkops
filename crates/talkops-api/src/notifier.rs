//! Notification worker (ADR 0035): reads the event outbox, applies the
//! rules (delay, recovery, digest, rate limit) and sends the mails. Also runs
//! the periodic health checks (disk space, media jobs, backup age) that
//! produce events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::Utc;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::mail;
use talkops_core::mail_render::{self, Values};
use talkops_core::notifications::{
    self as n, AlertState, Decision, Event, EventDef, NewDelivery, RecoveryAction, Rule, Severity,
};
use talkops_core::settings;
use talkops_core::tenant::TenantId;

use crate::MediaPaths;
use crate::mailer::{self, MailError, local_time};

/// Processed events and the delivery log are kept this long.
const KEEP_DAYS: i32 = 30;

/// True at most once per `every` for `key` (in this process). Producers use
/// it for events that would otherwise repeat on every request.
pub fn once_per(key: &str, every: Duration) -> bool {
    static SEEN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    let mut seen = SEEN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    seen.retain(|_, t| now.duration_since(*t) < Duration::from_secs(86_400));
    match seen.get(key) {
        Some(t) if now.duration_since(*t) < every => false,
        _ => {
            seen.insert(key.to_owned(), now);
            true
        }
    }
}

/// What a delivery sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Alert,
    Recovery,
    Digest,
    Test,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Recovery => "recovery",
            Self::Digest => "digest",
            Self::Test => "test",
        }
    }
}

/// Placeholder values of a notification mail. `rule_def` is the event type
/// of the rule; `shown` the event type whose title is used (the recovery
/// event for a recovery mail).
pub fn values(
    rule_def: &EventDef,
    shown: &EventDef,
    kind: Kind,
    events: &[Event],
    lang: &str,
    zone: &str,
) -> Values {
    let i = mail_render::lang_index(lang);
    let last = events.last();
    let params = last.map(|e| e.params.clone()).unwrap_or_default();
    let severity = if kind == Kind::Recovery {
        Severity::Info
    } else {
        last.and_then(|e| Severity::parse(&e.severity))
            .unwrap_or(rule_def.severity)
    };
    let category = n::category_label(rule_def.category, i);
    let at = last.map(|e| e.created_at).unwrap_or_else(Utc::now);
    let mut v = Values::new();
    let status = match (kind, i) {
        (Kind::Recovery, 0) => "Behoben",
        (Kind::Recovery, _) => "Resolved",
        (Kind::Digest, 0) => "Zusammenfassung",
        (Kind::Digest, _) => "Digest",
        (Kind::Test, 0) => "Test",
        (Kind::Test, _) => "Test",
        (Kind::Alert, 0) => "Problem",
        (Kind::Alert, _) => "Problem",
    };
    let title = if kind == Kind::Digest {
        if i == 0 {
            format!("{} Ereignisse: {category}", events.len())
        } else {
            format!("{} events: {category}", events.len())
        }
    } else {
        n::format_title(shown.title[i], &params)
    };
    let title = if kind == Kind::Test {
        format!("[Test] {title}")
    } else {
        title
    };
    if kind == Kind::Digest {
        let lines: Vec<String> = events
            .iter()
            .map(|e| {
                format!(
                    "{} – {}",
                    local_time(e.created_at, zone),
                    n::format_title(shown.title[i], &e.params)
                )
            })
            .collect();
        v.insert("event.digest".into(), lines.join("\n"));
    } else {
        v.insert("event.details".into(), n::details_text(&params));
    }
    if kind == Kind::Recovery {
        v.insert("event.recovery".into(), "1".into());
    }
    v.insert("event.title".into(), title);
    v.insert("event.category".into(), category.to_owned());
    v.insert("event.severity".into(), severity.label(i).to_owned());
    v.insert("event.status".into(), status.to_owned());
    v.insert("event.time".into(), local_time(at, zone));
    v.insert("event.count".into(), events.len().to_string());
    v.insert("now".into(), local_time(Utc::now(), zone));
    v
}

/// Outcome of a delivery, as written to the log.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub status: &'static str,
    pub recipients: Vec<String>,
    pub error: Option<String>,
}

/// Renders and sends one notification mail to the recipients of `rule`, and
/// logs the result.
pub async fn deliver(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    rule: &Rule,
    shown: &EventDef,
    kind: Kind,
    events: &[Event],
) -> Result<Outcome, MailError> {
    let rule_def = n::def(&rule.event_type)
        .ok_or_else(|| MailError::Other(format!("unknown event type {}", rule.event_type)))?;
    let outcome =
        send_to_recipients(db, secrets, tenant, rule, rule_def, shown, kind, events).await;
    let (outcome, subject) = match outcome {
        Ok(x) => x,
        Err(err) => (
            Outcome {
                status: "failed",
                recipients: Vec::new(),
                error: Some(err.to_string()),
            },
            String::new(),
        ),
    };
    n::log_delivery(
        db,
        tenant,
        &NewDelivery {
            event_type: &rule.event_type,
            kind: kind.as_str(),
            subject: &subject,
            recipients: &outcome.recipients,
            status: outcome.status,
            error: outcome.error.as_deref(),
            event_count: events.len().max(1) as i32,
        },
    )
    .await?;
    match outcome.status {
        "sent" => {
            tracing::info!(event_type = %rule.event_type, kind = kind.as_str(), "notification sent")
        }
        status => {
            tracing::warn!(event_type = %rule.event_type, status, error = ?outcome.error, "notification not sent")
        }
    }
    Ok(outcome)
}

#[allow(clippy::too_many_arguments)]
async fn send_to_recipients(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    rule: &Rule,
    rule_def: &EventDef,
    shown: &EventDef,
    kind: Kind,
    events: &[Event],
) -> Result<(Outcome, String), MailError> {
    let outcome = |status, recipients: &[String], error: Option<String>| Outcome {
        status,
        recipients: recipients.to_vec(),
        error,
    };
    let recipients = n::recipients(db, tenant, rule).await?;
    if recipients.is_empty() {
        return Ok((outcome("no_recipients", &[], None), String::new()));
    }
    if kind != Kind::Test
        && n::sent_last_hour(db, tenant, &rule.event_type).await?
            >= i64::from(rule.rate_limit_per_hour)
    {
        return Ok((outcome("rate_limited", &recipients, None), String::new()));
    }
    let Some(cfg) = mail::config(db, tenant, secrets).await? else {
        return Ok((outcome("no_smtp", &recipients, None), String::new()));
    };
    let s = settings::get(db, tenant).await?;
    let lang = s.default_language.clone();
    let vals = values(rule_def, shown, kind, events, &lang, &s.timezone);
    let (design, logo, rendered) =
        mailer::render_for(db, tenant, &mail_render::NOTIFICATION, &lang, &vals).await?;
    let subject = rendered.subject.clone();
    let mut errors = Vec::new();
    for to in &recipients {
        let result = async {
            let msg = mailer::build_message(
                mailer::sender(&cfg.from, &design.sender_name)?,
                to,
                rendered.clone(),
                logo.as_ref(),
                Vec::new(),
            )?;
            mailer::send(&cfg, msg).await
        }
        .await;
        if let Err(err) = result {
            errors.push(format!("{to}: {err}"));
        }
    }
    let status = if errors.is_empty() { "sent" } else { "failed" };
    let error = (!errors.is_empty()).then(|| errors.join("; "));
    Ok((outcome(status, &recipients, error), subject))
}

/// Handles one new event from the outbox.
async fn handle(db: &PgPool, secrets: &SecretBox, event: &Event) -> Result<(), MailError> {
    let Some(d) = n::def(&event.event_type) else {
        return Ok(());
    };
    let tenant = event.tenant_id;
    if d.is_recovery {
        for problem in n::problems_of(d.key) {
            let Some(alert) = n::open_alert(db, tenant, problem.key, &event.source).await? else {
                continue;
            };
            let Some(state) = alert.state() else {
                continue;
            };
            let rule = n::rule(db, tenant, problem).await?;
            n::close_alert(db, alert.id).await?;
            if n::on_recovery(state, &rule) == RecoveryAction::SendRecovery {
                deliver(
                    db,
                    secrets,
                    tenant,
                    &rule,
                    d,
                    Kind::Recovery,
                    std::slice::from_ref(event),
                )
                .await?;
            }
        }
        return Ok(());
    }
    let rule = n::rule(db, tenant, d).await?;
    let severity = Severity::parse(&event.severity).unwrap_or(d.severity);
    let open = if d.recovery.is_some() {
        n::open_alert(db, tenant, d.key, &event.source)
            .await?
            .and_then(|a| a.state())
    } else {
        None
    };
    match n::decide(d, &rule, severity, open) {
        Decision::Ignore => {}
        Decision::SendNow => {
            if d.recovery.is_some() {
                n::open_problem(db, event, 0).await?;
            }
            deliver(
                db,
                secrets,
                tenant,
                &rule,
                d,
                Kind::Alert,
                std::slice::from_ref(event),
            )
            .await?;
        }
        Decision::Schedule { delay_secs } => n::open_problem(db, event, delay_secs).await?,
        Decision::Digest { digest_secs } => n::add_to_digest(db, event, digest_secs).await?,
    }
    Ok(())
}

/// Sends pending alerts whose delay passed and digests whose time came.
async fn handle_due(db: &PgPool, secrets: &SecretBox) -> Result<(), MailError> {
    for alert in n::due_alerts(db).await? {
        let Some(d) = n::def(&alert.event_type) else {
            n::close_alert(db, alert.id).await?;
            continue;
        };
        let events = n::events_by_ids(db, &alert.event_ids).await?;
        let rule = n::rule(db, alert.tenant_id, d).await?;
        if alert.state() == Some(AlertState::Pending) {
            n::mark_alerted(db, alert.id).await?;
            if rule.enabled {
                deliver(db, secrets, alert.tenant_id, &rule, d, Kind::Alert, &events).await?;
            }
        } else {
            n::close_alert(db, alert.id).await?;
            if rule.enabled && !events.is_empty() {
                deliver(
                    db,
                    secrets,
                    alert.tenant_id,
                    &rule,
                    d,
                    Kind::Digest,
                    &events,
                )
                .await?;
            }
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Periodic checks
// ----------------------------------------------------------------------------

/// Free space below this share or size counts as low.
const DISK_LOW_PERCENT: u64 = 10;
const DISK_LOW_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub fn disk_low(free: u64, total: u64) -> bool {
    total > 0 && (free * 100 / total < DISK_LOW_PERCENT || free < DISK_LOW_BYTES)
}

fn human_bytes(b: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    format!("{:.1} GB", b as f64 / GB)
}

#[derive(Default)]
struct Checks {
    /// Last state per check source: true = problem.
    state: HashMap<String, bool>,
    last_stale_day: Option<chrono::NaiveDate>,
}

impl Checks {
    /// Emits `problem` or `recovery` when the state of `source` changes (or
    /// is new).
    async fn transition(
        &mut self,
        db: &PgPool,
        source: &str,
        bad: bool,
        problem: (&str, serde_json::Value),
        recovery: (&str, serde_json::Value),
    ) {
        if self.state.insert(source.to_owned(), bad) == Some(bad) {
            return;
        }
        let (event, params) = if bad { problem } else { recovery };
        n::emit_logged(db, TenantId::DEFAULT, event, source, params).await;
    }
}

async fn run_checks(db: &PgPool, paths: &[PathBuf], checks: &mut Checks) {
    let tenant = TenantId::DEFAULT;
    for path in paths {
        let (Ok(free), Ok(total)) = (fs2::available_space(path), fs2::total_space(path)) else {
            continue;
        };
        let p = path.display().to_string();
        let percent = (free * 100).checked_div(total).unwrap_or(0);
        checks
            .transition(
                db,
                &format!("disk:{p}"),
                disk_low(free, total),
                (
                    "storage.disk_low",
                    serde_json::json!({"path": p, "free": human_bytes(free), "percent": percent}),
                ),
                ("storage.disk_ok", serde_json::json!({"path": p})),
            )
            .await;
    }

    // Media jobs that failed in the last 30 minutes.
    let failed: Result<i64, _> = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE tenant_id = $1 AND status = 'failed'
           AND kind <> 'mail.voicemail' AND updated_at > now() - interval '30 minutes'",
    )
    .bind(tenant)
    .fetch_one(db)
    .await;
    if let Ok(count) = failed
        && count >= 3
        && once_per("jobs_failing", Duration::from_secs(3600))
    {
        n::emit_logged(
            db,
            tenant,
            "services.jobs_failing",
            "",
            serde_json::json!({"count": count}),
        )
        .await;
    }

    // Queued media jobs that nobody takes.
    let waiting: Result<Option<f64>, _> = sqlx::query_scalar(
        "SELECT extract(epoch FROM now() - min(run_at))::float8 FROM jobs
         WHERE tenant_id = $1 AND status = 'queued' AND kind <> 'mail.voicemail' AND run_at < now()
           AND NOT EXISTS (SELECT 1 FROM jobs j WHERE j.kind <> 'mail.voicemail'
                           AND j.status <> 'queued' AND j.updated_at > now() - interval '30 minutes')",
    )
    .bind(tenant)
    .fetch_one(db)
    .await;
    if let Ok(waiting) = waiting {
        let minutes = waiting.map(|s| (s / 60.0) as i64).unwrap_or(0);
        checks
            .transition(
                db,
                "media_worker",
                minutes >= 30,
                (
                    "services.worker_inactive",
                    serde_json::json!({"minutes": minutes}),
                ),
                ("services.worker_active", serde_json::json!({})),
            )
            .await;
    }

    // Daily backup without a new archive for two days.
    if let Ok(b) = talkops_core::backups::get(db, tenant).await
        && b.enabled
    {
        let last = b
            .last_file
            .as_deref()
            .and_then(crate::backup::stamp)
            .map(|t| t.and_utc());
        let days = last.map(|t| (Utc::now() - t).num_days());
        let today = Utc::now().date_naive();
        if days.is_none_or(|d| d >= 2)
            && b.last_run_at.is_some()
            && checks.last_stale_day != Some(today)
        {
            checks.last_stale_day = Some(today);
            let days = days.map(|d| d.to_string()).unwrap_or_else(|| "–".into());
            n::emit_logged(
                db,
                tenant,
                "backup.stale",
                "",
                serde_json::json!({"days": days}),
            )
            .await;
        }
    }
}

// ----------------------------------------------------------------------------
// FreeSWITCH state (from the telephony poller)
// ----------------------------------------------------------------------------

/// The parts of the live status that the monitor compares.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Observed {
    pub connected: bool,
    /// FreeSWITCH uptime in seconds.
    pub uptime: Option<u64>,
    /// Gateway name -> (state, last error).
    pub gateways: HashMap<String, (String, Option<String>)>,
    /// Registered SIP users (lower case).
    pub registered: std::collections::BTreeSet<String>,
}

/// An event found by [diff]: type, source and parameters.
pub type Found = (&'static str, String, serde_json::Value);

fn gateway_ok(state: &str) -> bool {
    matches!(state, "REGED" | "NOREG")
}

fn gateway_down(state: &str) -> bool {
    matches!(
        state,
        "FAIL_WAIT" | "FAILED" | "EXPIRED" | "UNREGED" | "TIMEOUT" | "NOAVAIL"
    )
}

/// Events between two observations. `prev` is `None` on the first poll.
pub fn diff(prev: Option<&Observed>, cur: &Observed) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    match (prev.map(|p| p.connected), cur.connected) {
        (Some(true) | None, false) => {
            out.push(("freeswitch.esl_down", String::new(), serde_json::json!({})));
        }
        (Some(false), true) => {
            out.push(("freeswitch.esl_up", String::new(), serde_json::json!({})));
        }
        _ => {}
    }
    let Some(prev) = prev else {
        return out;
    };
    if let (Some(before), Some(now)) = (prev.uptime, cur.uptime)
        && now < before
    {
        out.push((
            "freeswitch.restarted",
            String::new(),
            serde_json::json!({ "uptime_secs": now }),
        ));
    }
    if !(prev.connected && cur.connected) {
        return out;
    }
    let mut names: Vec<&String> = cur.gateways.keys().collect();
    names.sort();
    for name in names {
        let (state, error) = &cur.gateways[name];
        let before = prev.gateways.get(name).map(|(s, _)| s.as_str());
        let params = serde_json::json!({ "gateway": name, "state": state, "error": error });
        if gateway_down(state) && before.is_none_or(|b| !gateway_down(b)) {
            let rejected = error
                .as_deref()
                .is_some_and(|e| e.contains("401") || e.contains("403"));
            let event = if rejected {
                "trunk.rejected"
            } else {
                "trunk.down"
            };
            out.push((event, name.clone(), params));
        } else if gateway_ok(state) && before.is_some_and(|b| !gateway_ok(b)) {
            out.push(("trunk.up", name.clone(), params));
        }
    }
    for user in prev.registered.difference(&cur.registered) {
        out.push((
            "device.unregistered",
            user.clone(),
            serde_json::json!({ "sip_user": user }),
        ));
    }
    for user in cur.registered.difference(&prev.registered) {
        out.push((
            "device.registered",
            user.clone(),
            serde_json::json!({ "sip_user": user }),
        ));
    }
    out
}

/// Turns FreeSWITCH observations into events.
#[derive(Default)]
pub struct Monitor {
    prev: Option<Observed>,
}

impl Monitor {
    pub async fn observe(&mut self, db: &PgPool, mut cur: Observed) {
        // Keep the last known uptime while disconnected, so that a restart
        // shows after the reconnect.
        if cur.uptime.is_none() {
            cur.uptime = self.prev.as_ref().and_then(|p| p.uptime);
        }
        let found = diff(self.prev.as_ref(), &cur);
        self.prev = Some(cur);
        for (event, source, mut params) in found {
            if event.starts_with("trunk.") {
                let name = trunk_name(db, &source).await;
                params["name"] = serde_json::Value::String(name.unwrap_or_else(|| source.clone()));
            } else if event.starts_with("device.") {
                params["name"] = serde_json::Value::String(source.clone());
                if let Ok(Some(d)) = talkops_core::extensions::device_auth(db, &source).await {
                    params["user"] = serde_json::Value::String(format!(
                        "{} {}",
                        d.extension_number, d.display_name
                    ));
                }
            }
            n::emit_logged(db, TenantId::DEFAULT, event, &source, params).await;
        }
    }
}

/// Name of the trunk (and account) behind a gateway name `gw-<account id>`.
async fn trunk_name(db: &PgPool, gateway: &str) -> Option<String> {
    let id: uuid::Uuid = gateway.strip_prefix("gw-")?.parse().ok()?;
    sqlx::query_scalar(
        "SELECT t.name || ' (' || a.username || ')' FROM trunk_accounts a
         JOIN trunks t ON t.id = a.trunk_id WHERE a.id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// Runs the notification worker in the background.
pub fn spawn(db: PgPool, secrets: SecretBox, media: Arc<MediaPaths>) {
    tokio::spawn(async move {
        let paths = {
            let mut p = vec![media.recordings.clone(), media.voicemail.clone()];
            p.dedup();
            p
        };
        let mut checks = Checks::default();
        let mut last_checks = Instant::now() - Duration::from_secs(3600);
        let mut last_purge = Instant::now() - Duration::from_secs(86_400);
        loop {
            loop {
                let events = match n::claim_events(&db, 50).await {
                    Ok(e) => e,
                    Err(err) => {
                        tracing::debug!(error = %err, "cannot read notification events");
                        break;
                    }
                };
                if events.is_empty() {
                    break;
                }
                for event in &events {
                    if let Err(err) = handle(&db, &secrets, event).await {
                        tracing::warn!(event_type = %event.event_type, error = %err, "notification failed");
                    }
                }
            }
            if let Err(err) = handle_due(&db, &secrets).await {
                tracing::warn!(error = %err, "notification failed");
            }
            if last_checks.elapsed() >= Duration::from_secs(300) {
                last_checks = Instant::now();
                run_checks(&db, &paths, &mut checks).await;
            }
            if last_purge.elapsed() >= Duration::from_secs(86_400) {
                last_purge = Instant::now();
                match n::purge(&db, KEEP_DAYS).await {
                    Ok(0) => {}
                    Ok(count) => tracing::info!(count, "old notification entries deleted"),
                    Err(err) => {
                        tracing::warn!(error = %err, "cannot delete old notification entries")
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(id: i64, params: serde_json::Value) -> Event {
        Event {
            id,
            tenant_id: TenantId::DEFAULT,
            event_type: "trunk.down".into(),
            source: "gw-1".into(),
            severity: "warning".into(),
            params,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn values_for_alert_recovery_and_digest() {
        let down = n::def("trunk.down").unwrap();
        let up = n::def("trunk.up").unwrap();
        let e = event(1, json!({"name": "Telekom", "state": "FAIL_WAIT"}));
        let v = values(
            down,
            down,
            Kind::Alert,
            std::slice::from_ref(&e),
            "en",
            "UTC",
        );
        assert_eq!(
            v["event.title"],
            "Trunk Telekom: registration lost (FAIL_WAIT)"
        );
        assert_eq!(v["event.severity"], "Warning");
        assert_eq!(v["event.category"], "Trunks");
        assert!(v["event.details"].contains("state: FAIL_WAIT"));
        assert!(!v.contains_key("event.recovery"));

        let v = values(
            down,
            up,
            Kind::Recovery,
            std::slice::from_ref(&e),
            "de",
            "UTC",
        );
        assert_eq!(v["event.title"], "Trunk Telekom: wieder registriert");
        assert_eq!(v["event.severity"], "Information");
        assert_eq!(v["event.recovery"], "1");

        let cfg = n::def("config.system").unwrap();
        let events = [
            event(
                2,
                json!({"entity": "smtp", "action": "update", "user": "admin"}),
            ),
            event(
                3,
                json!({"entity": "settings", "action": "update", "user": "admin"}),
            ),
        ];
        let v = values(cfg, cfg, Kind::Digest, &events, "en", "UTC");
        assert_eq!(v["event.title"], "2 events: Configuration");
        assert_eq!(v["event.count"], "2");
        assert_eq!(v["event.digest"].lines().count(), 2);
        assert!(v["event.digest"].contains("System settings: smtp update (admin)"));
    }

    #[test]
    fn mails_render_with_default_texts() {
        let d = n::def("backup.failed").unwrap();
        let e = event(1, json!({"error": "<disk full>"}));
        let design = mail_render::Design::default();
        for lang in ["de", "en"] {
            let i = mail_render::lang_index(lang);
            let v = values(d, d, Kind::Alert, std::slice::from_ref(&e), lang, "UTC");
            let k = &mail_render::NOTIFICATION;
            let m = mail_render::render(&design, k, lang, k.subject[i], k.body[i], &v).unwrap();
            assert!(m.subject.contains("disk full"), "{}", m.subject);
            assert!(m.html.contains("&lt;disk full&gt;"));
            assert!(!m.html.contains("<disk full>"));
            assert!(m.text.contains("error: <disk full>"));
        }
    }

    fn obs(connected: bool, gws: &[(&str, &str, Option<&str>)], users: &[&str]) -> Observed {
        Observed {
            connected,
            uptime: connected.then_some(100),
            gateways: gws
                .iter()
                .map(|(n, s, e)| (n.to_string(), (s.to_string(), e.map(str::to_owned))))
                .collect(),
            registered: users.iter().map(|u| u.to_string()).collect(),
        }
    }

    fn types(found: &[Found]) -> Vec<&str> {
        found.iter().map(|f| f.0).collect()
    }

    #[test]
    fn freeswitch_state_changes() {
        let up = obs(true, &[("gw-1", "REGED", None)], &["100"]);
        // First poll: nothing to compare; connected means no event.
        assert!(diff(None, &up).is_empty());
        assert_eq!(
            types(&diff(None, &obs(false, &[], &[]))),
            ["freeswitch.esl_down"]
        );

        let down = obs(true, &[("gw-1", "FAIL_WAIT", Some("503"))], &["100"]);
        assert_eq!(types(&diff(Some(&up), &down)), ["trunk.down"]);
        assert!(diff(Some(&down), &down).is_empty());
        assert_eq!(types(&diff(Some(&down), &up)), ["trunk.up"]);
        let rejected = obs(
            true,
            &[("gw-1", "FAIL_WAIT", Some("403 Forbidden"))],
            &["100"],
        );
        assert_eq!(types(&diff(Some(&up), &rejected)), ["trunk.rejected"]);
        // TRYING is neither down nor up.
        let trying = obs(true, &[("gw-1", "TRYING", None)], &["100"]);
        assert!(diff(Some(&up), &trying).is_empty());

        let gone = obs(true, &[("gw-1", "REGED", None)], &[]);
        assert_eq!(types(&diff(Some(&up), &gone)), ["device.unregistered"]);
        assert_eq!(types(&diff(Some(&gone), &up)), ["device.registered"]);

        // Lost connection: only the ESL event, no device or trunk events.
        let lost = obs(false, &[], &[]);
        assert_eq!(types(&diff(Some(&up), &lost)), ["freeswitch.esl_down"]);
        assert_eq!(types(&diff(Some(&lost), &up)), ["freeswitch.esl_up"]);
        let mut restarted = up.clone();
        restarted.uptime = Some(5);
        assert_eq!(
            types(&diff(Some(&up), &restarted)),
            ["freeswitch.restarted"]
        );
    }

    #[test]
    fn disk_threshold() {
        const GB: u64 = 1024 * 1024 * 1024;
        assert!(disk_low(GB, 100 * GB));
        assert!(disk_low(5 * GB, 100 * GB));
        assert!(!disk_low(20 * GB, 100 * GB));
        assert!(disk_low(GB, 5 * GB));
        assert!(!disk_low(0, 0));
    }

    #[test]
    fn once_per_key() {
        assert!(once_per("test-key", Duration::from_secs(60)));
        assert!(!once_per("test-key", Duration::from_secs(60)));
        assert!(once_per("other-key", Duration::from_secs(60)));
    }
}
