//! Mail notifications (ADR 0035).
//!
//! Producers write events into the outbox table `notification_events`
//! ([emit]). The notification worker in the server claims the events,
//! matches them with the rule of their event type and sends mails with the
//! mail type [crate::mail_render::NOTIFICATION].
//!
//! - Problem events (e.g. `trunk.down`) name a recovery event
//!   (`trunk.up`) and a `source` (the gateway). With a delay, the worker
//!   waits; a recovery in this time cancels the mail. After an alert, the
//!   recovery event sends a recovery mail (if the rule wants it).
//! - Rules with a digest time collect events and send one mail.
//! - Each rule has a limit of mails per hour.
//!
//! The decisions are pure functions ([decide], [on_recovery]) so that they
//! can be tested without a database.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    /// Name in the mail (index 0 = de, 1 = en).
    pub fn label(self, lang: usize) -> &'static str {
        match (self, lang) {
            (Self::Info, 0) => "Information",
            (Self::Info, _) => "Information",
            (Self::Warning, 0) => "Warnung",
            (Self::Warning, _) => "Warning",
            (Self::Critical, 0) => "Kritisch",
            (Self::Critical, _) => "Critical",
        }
    }
}

/// Event categories in the order of the settings page.
pub const CATEGORIES: &[&str] = &[
    "trunks",
    "freeswitch",
    "devices",
    "security",
    "backup",
    "services",
    "config",
];

/// Category name in the mail (index 0 = de, 1 = en).
pub fn category_label(category: &str, lang: usize) -> &'static str {
    let names: [&str; 2] = match category {
        "trunks" => ["Trunks", "Trunks"],
        "freeswitch" => ["FreeSWITCH", "FreeSWITCH"],
        "devices" => ["Geräte", "Devices"],
        "security" => ["Sicherheit", "Security"],
        "backup" => ["Sicherung und Speicher", "Backup and storage"],
        "services" => ["Dienste", "Services"],
        _ => ["Konfiguration", "Configuration"],
    };
    names[lang.min(1)]
}

/// An event type of the catalog with the defaults of its rule.
#[derive(Debug, Clone, Copy)]
pub struct EventDef {
    pub key: &'static str,
    pub category: &'static str,
    pub severity: Severity,
    /// Mail title (de, en) with `{param}` placeholders.
    pub title: [&'static str; 2],
    /// Problem event: the event type that ends the problem.
    pub recovery: Option<&'static str>,
    /// True for recovery events (they have no own rule).
    pub is_recovery: bool,
    /// False: in the catalog, but no producer yet.
    pub implemented: bool,
    pub enabled: bool,
    pub delay_secs: i32,
    pub digest_secs: i32,
}

const fn ev(
    key: &'static str,
    category: &'static str,
    severity: Severity,
    de: &'static str,
    en: &'static str,
) -> EventDef {
    EventDef {
        key,
        category,
        severity,
        title: [de, en],
        recovery: None,
        is_recovery: false,
        implemented: true,
        enabled: true,
        delay_secs: 0,
        digest_secs: 0,
    }
}

impl EventDef {
    const fn rec(mut self, recovery: &'static str) -> Self {
        self.recovery = Some(recovery);
        self
    }
    const fn recovery_event(mut self) -> Self {
        self.is_recovery = true;
        self
    }
    const fn off(mut self) -> Self {
        self.enabled = false;
        self
    }
    const fn delay(mut self, secs: i32) -> Self {
        self.delay_secs = secs;
        self
    }
    const fn digest(mut self, secs: i32) -> Self {
        self.digest_secs = secs;
        self
    }
    const fn todo(mut self) -> Self {
        self.implemented = false;
        self.enabled = false;
        self
    }
}

use Severity::{Critical, Info, Warning};

/// All event types.
pub const CATALOG: &[EventDef] = &[
    // Trunks
    ev(
        "trunk.down",
        "trunks",
        Warning,
        "Trunk {name}: Registrierung verloren ({state})",
        "Trunk {name}: registration lost ({state})",
    )
    .rec("trunk.up")
    .delay(120),
    ev(
        "trunk.rejected",
        "trunks",
        Critical,
        "Trunk {name}: Registrierung abgelehnt ({error})",
        "Trunk {name}: registration rejected ({error})",
    )
    .rec("trunk.up"),
    ev(
        "trunk.up",
        "trunks",
        Info,
        "Trunk {name}: wieder registriert",
        "Trunk {name}: registered again",
    )
    .recovery_event(),
    ev(
        "trunk.outbound_failures",
        "trunks",
        Warning,
        "Trunk {name}: wiederholt fehlgeschlagene ausgehende Anrufe",
        "Trunk {name}: repeated outbound call failures",
    )
    .todo(),
    ev(
        "trunk.changed",
        "trunks",
        Info,
        "Trunk-Konfiguration: {entity} {action} ({user})",
        "Trunk configuration: {entity} {action} ({user})",
    )
    .digest(300)
    .off(),
    // FreeSWITCH
    ev(
        "freeswitch.esl_down",
        "freeswitch",
        Critical,
        "Verbindung zu FreeSWITCH verloren",
        "Connection to FreeSWITCH lost",
    )
    .rec("freeswitch.esl_up")
    .delay(60),
    ev(
        "freeswitch.esl_up",
        "freeswitch",
        Info,
        "Verbindung zu FreeSWITCH wiederhergestellt",
        "Connection to FreeSWITCH restored",
    )
    .recovery_event(),
    ev(
        "freeswitch.restarted",
        "freeswitch",
        Warning,
        "FreeSWITCH wurde neu gestartet",
        "FreeSWITCH restarted",
    ),
    ev(
        "freeswitch.profile_down",
        "freeswitch",
        Critical,
        "SIP-Profil {name} läuft nicht",
        "SIP profile {name} is not running",
    )
    .rec("freeswitch.profile_up")
    .delay(60),
    ev(
        "freeswitch.profile_up",
        "freeswitch",
        Info,
        "SIP-Profil {name} läuft wieder",
        "SIP profile {name} is running again",
    )
    .recovery_event(),
    ev(
        "freeswitch.channels_high",
        "freeswitch",
        Warning,
        "Viele gleichzeitige Gespräche: {sessions} von {max}",
        "Many concurrent calls: {sessions} of {max}",
    )
    .rec("freeswitch.channels_ok"),
    ev(
        "freeswitch.channels_ok",
        "freeswitch",
        Info,
        "Anzahl der Gespräche wieder normal",
        "Call count back to normal",
    )
    .recovery_event(),
    ev(
        "freeswitch.xml_curl_failed",
        "freeswitch",
        Critical,
        "FreeSWITCH-Konfigurationsabruf fehlgeschlagen",
        "FreeSWITCH configuration lookup failed",
    )
    .todo(),
    // Devices
    ev(
        "device.unregistered",
        "devices",
        Warning,
        "Gerät {name} ({user}) ist nicht mehr registriert",
        "Device {name} ({user}) is no longer registered",
    )
    .rec("device.registered")
    .delay(600)
    .off(),
    ev(
        "device.registered",
        "devices",
        Info,
        "Gerät {name} ({user}) ist wieder registriert",
        "Device {name} ({user}) is registered again",
    )
    .recovery_event(),
    // Security
    ev(
        "security.sip_ban",
        "security",
        Warning,
        "SIP-Schutz: Adresse {ip} gesperrt ({failures} Fehlversuche)",
        "SIP guard: address {ip} banned ({failures} failed logins)",
    ),
    ev(
        "security.login_blocked",
        "security",
        Warning,
        "Wiederholt fehlgeschlagene Anmeldungen von {ip}",
        "Repeated failed sign-ins from {ip}",
    ),
    ev(
        "security.admin_new_ip",
        "security",
        Warning,
        "Administrator {user} hat sich von einer neuen Adresse angemeldet: {ip}",
        "Administrator {user} signed in from a new address: {ip}",
    ),
    ev(
        "security.mfa_changed",
        "security",
        Info,
        "Zwei-Faktor-Anmeldung geändert: {action} ({user})",
        "Two-factor sign-in changed: {action} ({user})",
    ),
    ev(
        "security.password_changed",
        "security",
        Info,
        "Passwort geändert: {entity} {action} ({user})",
        "Password changed: {entity} {action} ({user})",
    ),
    ev(
        "security.api_token",
        "security",
        Info,
        "API-Token geändert: {entity} {action} ({user})",
        "API token changed: {entity} {action} ({user})",
    ),
    ev(
        "security.secret_viewed",
        "security",
        Info,
        "Geheimnis angezeigt: {entity} {action} ({user})",
        "Secret viewed: {entity} {action} ({user})",
    )
    .off(),
    // Backup and storage
    ev(
        "backup.failed",
        "backup",
        Critical,
        "Datensicherung fehlgeschlagen: {error}",
        "Backup failed: {error}",
    ),
    ev(
        "backup.succeeded",
        "backup",
        Info,
        "Datensicherung erstellt: {file}",
        "Backup written: {file}",
    )
    .off(),
    ev(
        "backup.target_failed",
        "backup",
        Critical,
        "Sicherungsziel {name}: {error}",
        "Backup target {name}: {error}",
    )
    .rec("backup.target_ok"),
    ev(
        "backup.target_ok",
        "backup",
        Info,
        "Sicherungsziel {name} arbeitet wieder",
        "Backup target {name} works again",
    )
    .recovery_event(),
    ev(
        "backup.stale",
        "backup",
        Warning,
        "Seit {days} Tagen keine erfolgreiche Datensicherung",
        "No successful backup for {days} days",
    ),
    ev(
        "storage.disk_low",
        "backup",
        Warning,
        "Wenig Speicherplatz: {free} frei ({percent} %) in {path}",
        "Low disk space: {free} free ({percent} %) in {path}",
    )
    .rec("storage.disk_ok"),
    ev(
        "storage.disk_ok",
        "backup",
        Info,
        "Wieder genug Speicherplatz in {path}",
        "Enough disk space again in {path}",
    )
    .recovery_event(),
    // Services
    ev(
        "services.jobs_failing",
        "services",
        Warning,
        "{count} Medien-Aufträge in 30 Minuten fehlgeschlagen",
        "{count} media jobs failed in 30 minutes",
    ),
    ev(
        "services.worker_inactive",
        "services",
        Critical,
        "Medien-Worker arbeitet nicht: Aufträge warten seit {minutes} Minuten",
        "Media worker inactive: jobs waiting for {minutes} minutes",
    )
    .rec("services.worker_active"),
    ev(
        "services.worker_active",
        "services",
        Info,
        "Medien-Worker arbeitet wieder",
        "Media worker active again",
    )
    .recovery_event(),
    ev(
        "services.voicemail_mail_failed",
        "services",
        Warning,
        "Voicemail-Mail fehlgeschlagen: {error}",
        "Voicemail mail failed: {error}",
    ),
    ev(
        "services.update_available",
        "services",
        Info,
        "Neue TalkOps-Version {version} verfügbar",
        "New TalkOps version {version} available",
    )
    .todo(),
    // Configuration (from the audit log)
    ev(
        "config.users",
        "config",
        Info,
        "Benutzer und Rollen: {entity} {action} ({user})",
        "Users and roles: {entity} {action} ({user})",
    )
    .digest(300)
    .off(),
    ev(
        "config.routing",
        "config",
        Info,
        "Rufnummern und Routing: {entity} {action} ({user})",
        "Numbers and routing: {entity} {action} ({user})",
    )
    .digest(300)
    .off(),
    ev(
        "config.devices",
        "config",
        Info,
        "Nebenstellen und Geräte: {entity} {action} ({user})",
        "Extensions and devices: {entity} {action} ({user})",
    )
    .digest(300)
    .off(),
    ev(
        "config.system",
        "config",
        Info,
        "System-Einstellungen: {entity} {action} ({user})",
        "System settings: {entity} {action} ({user})",
    )
    .digest(300)
    .off(),
    ev(
        "config.restore",
        "config",
        Warning,
        "Datensicherung wiederhergestellt",
        "Backup restored",
    ),
    ev(
        "config.restart",
        "config",
        Warning,
        "Neustart ausgelöst: {entity_id} ({user})",
        "Restart triggered: {entity_id} ({user})",
    ),
];

pub fn def(key: &str) -> Option<&'static EventDef> {
    CATALOG.iter().find(|d| d.key == key)
}

/// Problem events that `recovery` ends.
pub fn problems_of(recovery: &str) -> impl Iterator<Item = &'static EventDef> + '_ {
    CATALOG.iter().filter(move |d| d.recovery == Some(recovery))
}

/// Fills `{param}` placeholders; missing values become `–`.
pub fn format_title(template: &str, params: &serde_json::Value) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &rest[start + 1..start + end];
        let value = match params.get(name) {
            Some(serde_json::Value::String(s)) if !s.is_empty() => s.clone(),
            Some(serde_json::Value::Null) | None => "–".to_owned(),
            Some(serde_json::Value::String(_)) => "–".to_owned(),
            Some(v) => v.to_string(),
        };
        out.push_str(&value.chars().take(200).collect::<String>());
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

/// Detail lines `key: value` of the parameters (sorted by key).
pub fn details_text(params: &serde_json::Value) -> String {
    let Some(map) = params.as_object() else {
        return String::new();
    };
    let sorted: BTreeMap<_, _> = map.iter().collect();
    sorted
        .into_iter()
        .filter_map(|(k, v)| {
            let v = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => return None,
                other => other.to_string(),
            };
            (!v.is_empty()).then(|| format!("{k}: {}", v.chars().take(300).collect::<String>()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Event type for an audit log entry, if any. Read access, tests and
/// operation (door opener, captures) are not configuration changes.
pub fn audit_event(entity_type: &str, action: &str) -> Option<&'static str> {
    let has = |x: &str| entity_type == x || action == x;
    if ["enable_totp", "disable_totp", "reset_totp"]
        .iter()
        .any(|x| has(x))
    {
        return Some("security.mfa_changed");
    }
    if has("set_password") || has("change_password") || has("reset_password") {
        return Some("security.password_changed");
    }
    if has("new_token") || has("revoke_token") {
        return Some("security.api_token");
    }
    if action.starts_with("view_") || entity_type.starts_with("view_") {
        return Some("security.secret_viewed");
    }
    if has("restore") {
        return Some("config.restore");
    }
    if entity_type == "diagnostics" && action == "restart" {
        return Some("config.restart");
    }
    const IGNORED_ACTIONS: &[&str] = &[
        "test",
        "open",
        "call",
        "login",
        "logout",
        "control",
        "transcribe",
        "read_live_transcript",
        "start_capture",
        "stop_capture",
        "close_live_transcript",
        "clear_log",
        "generate",
        "download",
        "fetch",
        "run",
        "sync",
        "override",
        "update_voicemail",
    ];
    if IGNORED_ACTIONS.iter().any(|x| has(x)) {
        return None;
    }
    let group = |list: &[&str]| list.iter().any(|x| has(x));
    if group(&["trunk", "trunk_account", "trunk_line"]) {
        Some("trunk.changed")
    } else if group(&["user", "role", "roles", "setup"]) {
        Some("config.users")
    } else if group(&[
        "number",
        "ring_group",
        "time_condition",
        "queue",
        "attendant",
        "ivr",
        "call_block",
        "call_block_settings",
        "voicemail_config",
    ]) {
        Some("config.routing")
    } else if group(&[
        "extension",
        "device",
        "phone",
        "phone_settings",
        "phone_media",
        "firmware",
        "provisioning",
        "door_station",
    ]) {
        Some("config.devices")
    } else if group(&[
        "settings",
        "smtp",
        "mail_branding",
        "mail_template",
        "mail_logo",
        "identity_settings",
        "sip_guard",
        "transcription_provider",
        "transcription_profiles",
        "backup_settings",
        "backup_target",
        "backup_deployment",
        "backup_portainer",
        "notification_rule",
        "security",
        "system",
    ]) {
        Some("config.system")
    } else {
        None
    }
}

// ----------------------------------------------------------------------------
// Producers
// ----------------------------------------------------------------------------

/// Writes an event into the outbox. `params` must never contain secrets.
pub async fn emit<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    event_type: &str,
    source: &str,
    params: serde_json::Value,
) -> CoreResult<()> {
    let d = def(event_type)
        .ok_or_else(|| CoreError::Validation(format!("unknown event type {event_type}")))?;
    let source: String = source.chars().take(200).collect();
    sqlx::query(
        "INSERT INTO notification_events (tenant_id, event_type, source, severity, params)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant)
    .bind(d.key)
    .bind(source)
    .bind(d.severity.as_str())
    .bind(params)
    .execute(db)
    .await?;
    Ok(())
}

/// [emit] for code paths that must not fail because of a notification.
pub async fn emit_logged(
    pool: &PgPool,
    tenant: TenantId,
    event_type: &str,
    source: &str,
    params: serde_json::Value,
) {
    if let Err(err) = emit(pool, tenant, event_type, source, params).await {
        tracing::warn!(event_type, error = %err, "cannot store notification event");
    }
}

/// Remembers the address of a login. True if the user logged in before,
/// but never from this address.
pub async fn new_login_address(
    pool: &PgPool,
    tenant: TenantId,
    user: Uuid,
    ip: &str,
) -> CoreResult<bool> {
    let known: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM login_addresses WHERE user_id = $1)")
            .bind(user)
            .fetch_one(pool)
            .await?;
    let inserted = sqlx::query(
        "INSERT INTO login_addresses (tenant_id, user_id, ip) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, ip) DO UPDATE SET last_seen = now()
         RETURNING (xmax = 0) AS inserted",
    )
    .bind(tenant)
    .bind(user)
    .bind(ip)
    .fetch_one(pool)
    .await
    .map(|row| sqlx::Row::get::<bool, _>(&row, "inserted"))?;
    Ok(known && inserted)
}

// ----------------------------------------------------------------------------
// Rules
// ----------------------------------------------------------------------------

/// The rule of an event type (stored or the catalog default).
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Rule {
    pub event_type: String,
    pub enabled: bool,
    pub min_severity: String,
    pub recipient_users: Vec<Uuid>,
    pub recipient_roles: Vec<Uuid>,
    pub recipient_emails: Vec<String>,
    pub delay_secs: i32,
    pub send_recovery: bool,
    pub digest_secs: i32,
    pub rate_limit_per_hour: i32,
    /// True if the rule is stored (changed by an admin).
    pub custom: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct RuleInput {
    pub enabled: bool,
    #[serde(default = "info")]
    pub min_severity: String,
    #[serde(default)]
    pub recipient_users: Vec<Uuid>,
    #[serde(default)]
    pub recipient_roles: Vec<Uuid>,
    #[serde(default)]
    pub recipient_emails: Vec<String>,
    #[serde(default)]
    pub delay_secs: i32,
    #[serde(default = "yes")]
    pub send_recovery: bool,
    #[serde(default)]
    pub digest_secs: i32,
    #[serde(default = "twenty")]
    pub rate_limit_per_hour: i32,
}

fn info() -> String {
    "info".into()
}
fn yes() -> bool {
    true
}
fn twenty() -> i32 {
    20
}

pub fn default_rule(d: &EventDef, admin_role: Option<Uuid>) -> Rule {
    Rule {
        event_type: d.key.to_owned(),
        enabled: d.enabled,
        min_severity: "info".into(),
        recipient_users: Vec::new(),
        recipient_roles: admin_role.into_iter().collect(),
        recipient_emails: Vec::new(),
        delay_secs: d.delay_secs,
        send_recovery: true,
        digest_secs: d.digest_secs,
        rate_limit_per_hour: 20,
        custom: false,
    }
}

const RULE_COLUMNS: &str = "event_type, enabled, min_severity, recipient_users, recipient_roles, \
     recipient_emails, delay_secs, send_recovery, digest_secs, rate_limit_per_hour, true AS custom";

async fn admin_role(pool: &PgPool, tenant: TenantId) -> CoreResult<Option<Uuid>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM roles WHERE tenant_id = $1 AND builtin = 'admin'")
            .bind(tenant)
            .fetch_optional(pool)
            .await?,
    )
}

/// Rules of all configurable event types, in catalog order.
pub async fn rules(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<Rule>> {
    let sql = format!("SELECT {RULE_COLUMNS} FROM notification_rules WHERE tenant_id = $1");
    let stored: Vec<Rule> = sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?;
    let admin = admin_role(pool, tenant).await?;
    Ok(CATALOG
        .iter()
        .filter(|d| !d.is_recovery)
        .map(|d| {
            stored
                .iter()
                .find(|r| r.event_type == d.key)
                .cloned()
                .unwrap_or_else(|| default_rule(d, admin))
        })
        .collect())
}

pub async fn rule(pool: &PgPool, tenant: TenantId, d: &EventDef) -> CoreResult<Rule> {
    let sql = format!(
        "SELECT {RULE_COLUMNS} FROM notification_rules WHERE tenant_id = $1 AND event_type = $2"
    );
    match sqlx::query_as(&sql)
        .bind(tenant)
        .bind(d.key)
        .fetch_optional(pool)
        .await?
    {
        Some(r) => Ok(r),
        None => Ok(default_rule(d, admin_role(pool, tenant).await?)),
    }
}

fn valid_email(e: &str) -> bool {
    let Some((local, domain)) = e.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && e.len() <= 254
        && !e
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "<>,;\"".contains(c))
}

pub async fn save_rule(
    pool: &PgPool,
    tenant: TenantId,
    d: &EventDef,
    input: &RuleInput,
) -> CoreResult<Rule> {
    if d.is_recovery {
        return Err(CoreError::Validation(
            "recovery events follow the rule of their problem event".into(),
        ));
    }
    if Severity::parse(&input.min_severity).is_none() {
        return Err(CoreError::Validation("invalid minimum severity".into()));
    }
    let mut emails: Vec<String> = input
        .recipient_emails
        .iter()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty())
        .collect();
    emails.sort();
    emails.dedup();
    if let Some(bad) = emails.iter().find(|e| !valid_email(e)) {
        return Err(CoreError::Validation(format!(
            "invalid e-mail address: {bad}"
        )));
    }
    if emails.len() > 50 || input.recipient_users.len() > 200 || input.recipient_roles.len() > 50 {
        return Err(CoreError::Validation("too many recipients".into()));
    }
    if !(0..=86_400).contains(&input.delay_secs) || !(0..=86_400).contains(&input.digest_secs) {
        return Err(CoreError::Validation(
            "delay and digest time: 0 to 86400 seconds".into(),
        ));
    }
    if !(1..=1000).contains(&input.rate_limit_per_hour) {
        return Err(CoreError::Validation(
            "rate limit: 1 to 1000 mails per hour".into(),
        ));
    }
    let foreign: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM unnest($2::uuid[]) u(id)
                 WHERE NOT EXISTS (SELECT 1 FROM users WHERE tenant_id = $1 AND id = u.id))
              + (SELECT count(*) FROM unnest($3::uuid[]) r(id)
                 WHERE NOT EXISTS (SELECT 1 FROM roles WHERE tenant_id = $1 AND id = r.id))",
    )
    .bind(tenant)
    .bind(&input.recipient_users)
    .bind(&input.recipient_roles)
    .fetch_one(pool)
    .await?;
    if foreign > 0 {
        return Err(CoreError::Validation("unknown user or role".into()));
    }
    let sql = format!(
        "INSERT INTO notification_rules (tenant_id, event_type, enabled, min_severity,
             recipient_users, recipient_roles, recipient_emails, delay_secs, send_recovery,
             digest_secs, rate_limit_per_hour)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (tenant_id, event_type) DO UPDATE SET enabled = $3, min_severity = $4,
             recipient_users = $5, recipient_roles = $6, recipient_emails = $7, delay_secs = $8,
             send_recovery = $9, digest_secs = $10, rate_limit_per_hour = $11, updated_at = now()
         RETURNING {RULE_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(d.key)
        .bind(input.enabled)
        .bind(&input.min_severity)
        .bind(&input.recipient_users)
        .bind(&input.recipient_roles)
        .bind(&emails)
        .bind(input.delay_secs)
        .bind(input.send_recovery)
        .bind(input.digest_secs)
        .bind(input.rate_limit_per_hour)
        .fetch_one(pool)
        .await?)
}

pub async fn reset_rule(pool: &PgPool, tenant: TenantId, d: &EventDef) -> CoreResult<Rule> {
    sqlx::query("DELETE FROM notification_rules WHERE tenant_id = $1 AND event_type = $2")
        .bind(tenant)
        .bind(d.key)
        .execute(pool)
        .await?;
    rule(pool, tenant, d).await
}

/// E-mail addresses of a rule: enabled users and members of the roles with
/// an address, plus the free addresses.
pub async fn recipients(pool: &PgPool, tenant: TenantId, rule: &Rule) -> CoreResult<Vec<String>> {
    let mut list: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT lower(u.email) FROM users u
         WHERE u.tenant_id = $1 AND u.enabled AND coalesce(u.email, '') <> ''
           AND (u.id = ANY($2) OR u.role_id = ANY($3)
                OR (u.role_id IS NULL AND EXISTS (
                    SELECT 1 FROM roles r WHERE r.tenant_id = $1 AND r.id = ANY($3)
                      AND r.builtin = u.role)))",
    )
    .bind(tenant)
    .bind(&rule.recipient_users)
    .bind(&rule.recipient_roles)
    .fetch_all(pool)
    .await?;
    list.extend(rule.recipient_emails.iter().map(|e| e.to_lowercase()));
    list.sort();
    list.dedup();
    Ok(list)
}

// ----------------------------------------------------------------------------
// Decisions (pure)
// ----------------------------------------------------------------------------

/// What to do with a (non-recovery) event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Rule off, severity too low, or the problem is already open.
    Ignore,
    SendNow,
    /// Problem event: wait, then send unless a recovery comes first.
    Schedule {
        delay_secs: i32,
    },
    /// Add to the digest; send `digest_secs` after the first event.
    Digest {
        digest_secs: i32,
    },
}

/// Open alert of a problem event and source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertState {
    Pending,
    Alerted,
}

pub fn decide(d: &EventDef, rule: &Rule, severity: Severity, open: Option<AlertState>) -> Decision {
    let min = Severity::parse(&rule.min_severity).unwrap_or(Severity::Info);
    if !rule.enabled || severity < min {
        return Decision::Ignore;
    }
    if d.recovery.is_some() {
        if open.is_some() {
            return Decision::Ignore;
        }
        if rule.delay_secs > 0 {
            return Decision::Schedule {
                delay_secs: rule.delay_secs,
            };
        }
        return Decision::SendNow;
    }
    if rule.digest_secs > 0 {
        return Decision::Digest {
            digest_secs: rule.digest_secs,
        };
    }
    Decision::SendNow
}

/// What a recovery event does with an open alert of its problem event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// The problem ended within the delay: no mail at all.
    Cancel,
    /// Close the alert and send a recovery mail.
    SendRecovery,
    /// Close the alert without a mail.
    Close,
}

pub fn on_recovery(state: AlertState, rule: &Rule) -> RecoveryAction {
    match state {
        AlertState::Pending => RecoveryAction::Cancel,
        AlertState::Alerted if rule.enabled && rule.send_recovery => RecoveryAction::SendRecovery,
        AlertState::Alerted => RecoveryAction::Close,
    }
}

// ----------------------------------------------------------------------------
// Worker storage
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Event {
    pub id: i64,
    #[serde(skip)]
    pub tenant_id: TenantId,
    pub event_type: String,
    pub source: String,
    pub severity: String,
    #[schema(value_type = Object)]
    pub params: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

const EVENT_COLUMNS: &str = "id, tenant_id, event_type, source, severity, params, created_at";

/// Takes up to `limit` new events and marks them as processed.
pub async fn claim_events(pool: &PgPool, limit: i64) -> CoreResult<Vec<Event>> {
    let sql = format!(
        "UPDATE notification_events SET processed_at = now()
         WHERE id IN (SELECT id FROM notification_events WHERE processed_at IS NULL
                      ORDER BY id LIMIT $1 FOR UPDATE SKIP LOCKED)
         RETURNING {EVENT_COLUMNS}"
    );
    let mut events: Vec<Event> = sqlx::query_as(&sql).bind(limit).fetch_all(pool).await?;
    events.sort_by_key(|e| e.id);
    Ok(events)
}

pub async fn events_by_ids(pool: &PgPool, ids: &[i64]) -> CoreResult<Vec<Event>> {
    let sql =
        format!("SELECT {EVENT_COLUMNS} FROM notification_events WHERE id = ANY($1) ORDER BY id");
    Ok(sqlx::query_as(&sql).bind(ids).fetch_all(pool).await?)
}

/// Recent events for the settings page.
pub async fn recent_events(pool: &PgPool, tenant: TenantId, limit: i64) -> CoreResult<Vec<Event>> {
    let sql = format!(
        "SELECT {EVENT_COLUMNS} FROM notification_events WHERE tenant_id = $1
         ORDER BY created_at DESC, id DESC LIMIT $2"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(limit)
        .fetch_all(pool)
        .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct Alert {
    pub id: i64,
    pub tenant_id: TenantId,
    pub event_type: String,
    pub source: String,
    pub kind: String,
    pub event_ids: Vec<i64>,
}

pub async fn open_alert(
    pool: &PgPool,
    tenant: TenantId,
    event_type: &str,
    source: &str,
) -> CoreResult<Option<Alert>> {
    Ok(sqlx::query_as(
        "SELECT id, tenant_id, event_type, source, kind, event_ids FROM notification_alerts
         WHERE tenant_id = $1 AND event_type = $2 AND source = $3",
    )
    .bind(tenant)
    .bind(event_type)
    .bind(source)
    .fetch_optional(pool)
    .await?)
}

impl Alert {
    pub fn state(&self) -> Option<AlertState> {
        match self.kind.as_str() {
            "pending" => Some(AlertState::Pending),
            "alerted" => Some(AlertState::Alerted),
            _ => None,
        }
    }
}

/// Creates a pending (`due_secs` > 0) or alerted (`due_secs` = 0) alert.
pub async fn open_problem(pool: &PgPool, event: &Event, due_secs: i32) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO notification_alerts (tenant_id, event_type, source, kind, event_ids, due_at)
         VALUES ($1, $2, $3, CASE WHEN $5 > 0 THEN 'pending' ELSE 'alerted' END, ARRAY[$4::bigint],
                 CASE WHEN $5 > 0 THEN now() + make_interval(secs => $5) END)
         ON CONFLICT (tenant_id, event_type, source) DO NOTHING",
    )
    .bind(event.tenant_id)
    .bind(&event.event_type)
    .bind(&event.source)
    .bind(event.id)
    .bind(f64::from(due_secs))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_alerted(pool: &PgPool, id: i64) -> CoreResult<()> {
    sqlx::query("UPDATE notification_alerts SET kind = 'alerted', due_at = NULL WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn close_alert(pool: &PgPool, id: i64) -> CoreResult<()> {
    sqlx::query("DELETE FROM notification_alerts WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Adds an event to the digest of its type; the first event sets the time.
pub async fn add_to_digest(pool: &PgPool, event: &Event, digest_secs: i32) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO notification_alerts (tenant_id, event_type, source, kind, event_ids, due_at)
         VALUES ($1, $2, '', 'digest', ARRAY[$3::bigint], now() + make_interval(secs => $4))
         ON CONFLICT (tenant_id, event_type, source) DO UPDATE
             SET event_ids = notification_alerts.event_ids || $3::bigint",
    )
    .bind(event.tenant_id)
    .bind(&event.event_type)
    .bind(event.id)
    .bind(f64::from(digest_secs))
    .execute(pool)
    .await?;
    Ok(())
}

/// Pending alerts and digests whose time has come.
pub async fn due_alerts(pool: &PgPool) -> CoreResult<Vec<Alert>> {
    Ok(sqlx::query_as(
        "SELECT id, tenant_id, event_type, source, kind, event_ids FROM notification_alerts
         WHERE kind IN ('pending', 'digest') AND due_at <= now() ORDER BY due_at LIMIT 100",
    )
    .fetch_all(pool)
    .await?)
}

/// Mails sent by the rule of `event_type` in the last hour.
pub async fn sent_last_hour(pool: &PgPool, tenant: TenantId, event_type: &str) -> CoreResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM notification_deliveries
         WHERE tenant_id = $1 AND event_type = $2 AND status = 'sent' AND kind <> 'test'
           AND created_at > now() - interval '1 hour'",
    )
    .bind(tenant)
    .bind(event_type)
    .fetch_one(pool)
    .await?)
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Delivery {
    pub id: i64,
    pub event_type: String,
    pub kind: String,
    pub subject: String,
    pub recipients: Vec<String>,
    pub status: String,
    pub error: Option<String>,
    pub event_count: i32,
    pub created_at: DateTime<Utc>,
}

/// A new entry of the delivery log.
pub struct NewDelivery<'a> {
    pub event_type: &'a str,
    pub kind: &'a str,
    pub subject: &'a str,
    pub recipients: &'a [String],
    pub status: &'a str,
    pub error: Option<&'a str>,
    pub event_count: i32,
}

pub async fn log_delivery(pool: &PgPool, tenant: TenantId, d: &NewDelivery<'_>) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO notification_deliveries (tenant_id, event_type, kind, subject, recipients,
             status, error, event_count)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(tenant)
    .bind(d.event_type)
    .bind(d.kind)
    .bind(d.subject.chars().take(300).collect::<String>())
    .bind(d.recipients)
    .bind(d.status)
    .bind(d.error.map(|e| e.chars().take(500).collect::<String>()))
    .bind(d.event_count)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn deliveries(pool: &PgPool, tenant: TenantId, limit: i64) -> CoreResult<Vec<Delivery>> {
    Ok(sqlx::query_as(
        "SELECT id, event_type, kind, subject, recipients, status, error, event_count, created_at
         FROM notification_deliveries WHERE tenant_id = $1
         ORDER BY created_at DESC, id DESC LIMIT $2",
    )
    .bind(tenant)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

/// Deletes processed events, closed state and log entries older than `days`.
pub async fn purge(pool: &PgPool, days: i32) -> CoreResult<u64> {
    let a = sqlx::query(
        "DELETE FROM notification_events e WHERE processed_at IS NOT NULL
           AND created_at < now() - make_interval(days => $1)
           AND NOT EXISTS (SELECT 1 FROM notification_alerts a WHERE e.id = ANY(a.event_ids))",
    )
    .bind(days)
    .execute(pool)
    .await?
    .rows_affected();
    let b = sqlx::query(
        "DELETE FROM notification_deliveries WHERE created_at < now() - make_interval(days => $1)",
    )
    .bind(days)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(a + b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(d: &EventDef) -> Rule {
        let mut r = default_rule(d, None);
        r.enabled = true;
        r
    }

    #[test]
    fn catalog_is_consistent() {
        let mut keys = std::collections::HashSet::new();
        for d in CATALOG {
            assert!(keys.insert(d.key), "duplicate {}", d.key);
            assert!(CATEGORIES.contains(&d.category), "{}", d.key);
            if let Some(r) = d.recovery {
                let rec = def(r).unwrap_or_else(|| panic!("missing {r}"));
                assert!(rec.is_recovery, "{r}");
            }
            if d.is_recovery {
                assert!(problems_of(d.key).count() > 0, "{} recovers nothing", d.key);
            }
            assert!(
                d.implemented || !d.enabled,
                "{} enabled but not implemented",
                d.key
            );
        }
        assert_eq!(problems_of("trunk.up").count(), 2);
    }

    #[test]
    fn decisions_with_delay_and_recovery() {
        let down = def("trunk.down").unwrap();
        let mut r = rule(down);
        assert_eq!(
            decide(down, &r, Severity::Warning, None),
            Decision::Schedule { delay_secs: 120 }
        );
        // A second down event while the first is open changes nothing.
        assert_eq!(
            decide(down, &r, Severity::Warning, Some(AlertState::Pending)),
            Decision::Ignore
        );
        assert_eq!(
            decide(down, &r, Severity::Warning, Some(AlertState::Alerted)),
            Decision::Ignore
        );
        r.delay_secs = 0;
        assert_eq!(decide(down, &r, Severity::Warning, None), Decision::SendNow);
        r.min_severity = "critical".into();
        assert_eq!(decide(down, &r, Severity::Warning, None), Decision::Ignore);
        r.min_severity = "info".into();
        r.enabled = false;
        assert_eq!(decide(down, &r, Severity::Warning, None), Decision::Ignore);

        // Recovery inside the delay: no mail; after the alert: recovery mail.
        let r = rule(down);
        assert_eq!(on_recovery(AlertState::Pending, &r), RecoveryAction::Cancel);
        assert_eq!(
            on_recovery(AlertState::Alerted, &r),
            RecoveryAction::SendRecovery
        );
        let quiet = Rule {
            send_recovery: false,
            ..rule(down)
        };
        assert_eq!(
            on_recovery(AlertState::Alerted, &quiet),
            RecoveryAction::Close
        );
    }

    #[test]
    fn digest_for_config_changes() {
        let d = def("config.system").unwrap();
        assert_eq!(
            decide(d, &rule(d), Severity::Info, None),
            Decision::Digest { digest_secs: 300 }
        );
        let no_digest = Rule {
            digest_secs: 0,
            ..rule(d)
        };
        assert_eq!(
            decide(d, &no_digest, Severity::Info, None),
            Decision::SendNow
        );
    }

    #[test]
    fn titles_and_details() {
        let params = json!({"name": "Telekom", "state": "FAIL_WAIT", "error": null, "n": 3});
        assert_eq!(
            format_title("Trunk {name}: lost ({state}) {error} {missing}", &params),
            "Trunk Telekom: lost (FAIL_WAIT) – –"
        );
        assert_eq!(
            details_text(&params),
            "n: 3\nname: Telekom\nstate: FAIL_WAIT"
        );
    }

    #[test]
    fn audit_mapping() {
        assert_eq!(
            audit_event("user", "enable_totp"),
            Some("security.mfa_changed")
        );
        assert_eq!(
            audit_event("disable_totp", "user"),
            Some("security.mfa_changed")
        );
        assert_eq!(
            audit_event("user", "set_password"),
            Some("security.password_changed")
        );
        assert_eq!(
            audit_event("door_station", "new_token"),
            Some("security.api_token")
        );
        assert_eq!(
            audit_event("device", "view_credentials"),
            Some("security.secret_viewed")
        );
        assert_eq!(audit_event("trunk", "update"), Some("trunk.changed"));
        assert_eq!(audit_event("update", "trunk"), Some("trunk.changed"));
        assert_eq!(audit_event("extension", "create"), Some("config.devices"));
        assert_eq!(audit_event("number", "delete"), Some("config.routing"));
        assert_eq!(audit_event("smtp", "update"), Some("config.system"));
        assert_eq!(audit_event("backup", "restore"), Some("config.restore"));
        assert_eq!(
            audit_event("diagnostics", "restart"),
            Some("config.restart")
        );
        assert_eq!(audit_event("door_station", "open"), None);
        assert_eq!(audit_event("smtp", "test"), None);
        assert_eq!(audit_event("recording", "transcribe"), None);
        assert_eq!(audit_event("voicemail", "delete"), None);
        for key in [
            "security.mfa_changed",
            "config.devices",
            "trunk.changed",
            "config.restart",
        ] {
            assert!(def(key).is_some());
        }
    }

    #[test]
    fn emails_are_checked() {
        assert!(valid_email("ops@example.com"));
        assert!(!valid_email("ops@localhost"));
        assert!(!valid_email("a b@example.com"));
        assert!(!valid_email("x@y.z>,evil@example.com"));
    }
}
