//! Wi-Fi voice survey (ADR 0041): a test call to a Wi-Fi handset while an
//! administrator walks the site. The API stores uplink media values, the
//! Wi-Fi state from UniFi and markers each second; the media worker rates
//! each roam ([`analysis`]).
//!
//! Data exists only for started surveys. The retention loop deletes it
//! with the survey recording after `retention_days`.

pub mod analysis;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::extensions::{self, DeviceKind};
use crate::jobs::{self, NewJob};
use crate::tenant::TenantId;

pub use analysis::{Analysis, Rating, Roam, Summary};

/// Job kind: analyse a finished survey (media worker).
pub const JOB_ANALYSE: &str = "wifi_survey.analyse";
/// How long a self-dial survey waits for the call.
pub const ARM_MINUTES: i64 = 10;
/// Feature code for a self-dial survey.
pub const SELF_DIAL_CODE: &str = "*39";
/// Longest marker note.
pub const MAX_NOTE: usize = 200;

/// Thresholds and limits of a tenant.
#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Thresholds {
    /// A gap shorter than this is clean.
    pub clean_ms: i32,
    /// A gap longer than this is bad.
    pub bad_ms: i32,
    /// Loss in the ±3 s window above this percentage is a loss block (bad).
    pub loss_block_pct: i32,
    pub sticky_rssi: i32,
    pub sticky_secs: i32,
    pub pingpong_count: i32,
    pub pingpong_secs: i32,
    pub weak_rssi: i32,
    pub weak_secs: i32,
    pub retention_days: i32,
    pub max_minutes: i32,
    pub max_parallel: i32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            clean_ms: 50,
            bad_ms: 150,
            loss_block_pct: 5,
            sticky_rssi: -75,
            sticky_secs: 10,
            pingpong_count: 3,
            pingpong_secs: 60,
            weak_rssi: -80,
            weak_secs: 5,
            retention_days: 30,
            max_minutes: 60,
            max_parallel: 3,
        }
    }
}

impl Thresholds {
    pub fn validate(&self) -> CoreResult<()> {
        let ok = (10..=1000).contains(&self.clean_ms)
            && (20..=5000).contains(&self.bad_ms)
            && self.clean_ms < self.bad_ms
            && (1..=100).contains(&self.loss_block_pct)
            && (-100..=-30).contains(&self.sticky_rssi)
            && (2..=600).contains(&self.sticky_secs)
            && (2..=20).contains(&self.pingpong_count)
            && (5..=600).contains(&self.pingpong_secs)
            && (-100..=-30).contains(&self.weak_rssi)
            && (1..=600).contains(&self.weak_secs)
            && (1..=365).contains(&self.retention_days)
            && (1..=240).contains(&self.max_minutes)
            && (1..=20).contains(&self.max_parallel);
        if ok {
            Ok(())
        } else {
            Err(CoreError::Validation(
                "a survey setting is out of range".into(),
            ))
        }
    }
}

const SETTINGS_COLUMNS: &str = "clean_ms, bad_ms, loss_block_pct, sticky_rssi, sticky_secs, \
     pingpong_count, pingpong_secs, weak_rssi, weak_secs, retention_days, max_minutes, max_parallel";

pub async fn settings<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Thresholds> {
    let sql = format!("SELECT {SETTINGS_COLUMNS} FROM wifi_survey_settings WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .fetch_optional(db)
        .await?
        .unwrap_or_default())
}

pub async fn save_settings(pool: &PgPool, tenant: TenantId, t: &Thresholds) -> CoreResult<()> {
    t.validate()?;
    sqlx::query(
        "INSERT INTO wifi_survey_settings (tenant_id, clean_ms, bad_ms, loss_block_pct,
             sticky_rssi, sticky_secs, pingpong_count, pingpong_secs, weak_rssi, weak_secs,
             retention_days, max_minutes, max_parallel)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         ON CONFLICT (tenant_id) DO UPDATE SET clean_ms = $2, bad_ms = $3, loss_block_pct = $4,
             sticky_rssi = $5, sticky_secs = $6, pingpong_count = $7, pingpong_secs = $8,
             weak_rssi = $9, weak_secs = $10, retention_days = $11, max_minutes = $12,
             max_parallel = $13, updated_at = now()",
    )
    .bind(tenant)
    .bind(t.clean_ms)
    .bind(t.bad_ms)
    .bind(t.loss_block_pct)
    .bind(t.sticky_rssi)
    .bind(t.sticky_secs)
    .bind(t.pingpong_count)
    .bind(t.pingpong_secs)
    .bind(t.weak_rssi)
    .bind(t.weak_secs)
    .bind(t.retention_days)
    .bind(t.max_minutes)
    .bind(t.max_parallel)
    .execute(pool)
    .await?;
    Ok(())
}

/// States of a survey.
pub mod state {
    pub const ARMED: &str = "armed";
    pub const CALLING: &str = "calling";
    pub const RUNNING: &str = "running";
    pub const ANALYSING: &str = "analysing";
    pub const ANALYSED: &str = "analysed";
    pub const FAILED: &str = "failed";
    pub const CANCELLED: &str = "cancelled";
    /// States with a call that can still produce data.
    pub const ACTIVE: &[&str] = &[ARMED, CALLING, RUNNING];
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Survey {
    pub id: Uuid,
    pub device_id: Option<Uuid>,
    pub extension_id: Option<Uuid>,
    pub phone_id: Option<Uuid>,
    pub device_label: String,
    pub mac: Option<String>,
    pub label: String,
    /// `originate` or `self_dial`.
    pub mode: String,
    pub state: String,
    pub call_uuid: Option<Uuid>,
    pub ptime_ms: i32,
    pub codec: Option<String>,
    #[serde(skip)]
    pub recording_file: Option<String>,
    pub recording_secs: Option<i32>,
    pub started_by: Option<Uuid>,
    pub armed_until: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub answered_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub hangup_cause: Option<String>,
    #[schema(value_type = Object)]
    pub thresholds: serde_json::Value,
    /// UniFi state at the start (client found, AP, capabilities).
    #[schema(value_type = Object)]
    pub unifi: serde_json::Value,
    #[schema(value_type = Option<Object>)]
    pub summary: Option<serde_json::Value>,
    pub error: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl Survey {
    pub fn is_active(&self) -> bool {
        state::ACTIVE.contains(&self.state.as_str())
    }

    /// Thresholds stored with the survey (the settings at its start).
    pub fn thresholds(&self) -> Thresholds {
        serde_json::from_value(self.thresholds.clone()).unwrap_or_default()
    }

    /// Milliseconds since the call was answered.
    pub fn offset_ms(&self, at: DateTime<Utc>) -> i32 {
        self.answered_at.map_or(0, |a| {
            (at - a).num_milliseconds().clamp(0, i32::MAX as i64) as i32
        })
    }
}

const COLUMNS: &str = "id, device_id, extension_id, phone_id, device_label, mac, label, mode, \
     state, call_uuid, ptime_ms, codec, recording_file, recording_secs, started_by, armed_until, \
     started_at, answered_at, ended_at, hangup_cause, thresholds, unifi, summary, error, \
     expires_at, created_at";

/// The device a survey runs on.
#[derive(Debug, Clone)]
pub struct Target {
    pub device: extensions::Device,
    pub extension: extensions::Extension,
    /// MAC of the provisioned phone (lowercase hex), if any.
    pub mac: Option<String>,
}

/// Loads an enabled Wi-Fi device with its extension and phone MAC.
pub async fn target(pool: &PgPool, tenant: TenantId, device: Uuid) -> CoreResult<Target> {
    let device = extensions::get_device(pool, tenant, device).await?;
    if device.kind != DeviceKind::Wifi {
        return Err(CoreError::Validation(
            "a survey needs a Wi-Fi device".into(),
        ));
    }
    if !device.enabled {
        return Err(CoreError::Validation("the device is disabled".into()));
    }
    let extension = extensions::get(pool, tenant, device.extension_id).await?;
    let mac = match device.phone_id {
        Some(phone) => Some(crate::phones::get(pool, tenant, phone).await?.mac),
        None => None,
    };
    Ok(Target {
        device,
        extension,
        mac,
    })
}

/// Input to start a survey.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct NewSurvey {
    pub device_id: Uuid,
    /// `originate` (TalkOps calls the handset) or `self_dial` (the tester
    /// dials `*39` within 10 minutes).
    pub mode: String,
    #[serde(default)]
    pub label: String,
}

/// Creates a survey in state `calling` (originate) or `armed` (self-dial).
/// Checks the device kind, the one-active-survey rule and the parallel limit.
pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    input: &NewSurvey,
    target: &Target,
    started_by: Option<Uuid>,
    unifi: serde_json::Value,
) -> CoreResult<Survey> {
    if !["originate", "self_dial"].contains(&input.mode.as_str()) {
        return Err(CoreError::Validation("unknown survey mode".into()));
    }
    let label = input.label.trim();
    if label.chars().count() > 120 || label.chars().any(char::is_control) {
        return Err(CoreError::Validation("invalid label".into()));
    }
    let t = settings(pool, tenant).await?;
    let mut tx = pool.begin().await?;
    // Serializes the parallel limit for the tenant.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('wifi_survey:' || $1::text))")
        .bind(tenant)
        .execute(&mut *tx)
        .await?;
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM wifi_surveys WHERE tenant_id = $1 AND state = ANY($2)",
    )
    .bind(tenant)
    .bind(state::ACTIVE)
    .fetch_one(&mut *tx)
    .await?;
    if active >= t.max_parallel as i64 {
        return Err(CoreError::Conflict(
            "too many surveys run at the same time".into(),
        ));
    }
    let now = Utc::now();
    let (st, armed_until) = if input.mode == "self_dial" {
        (state::ARMED, Some(now + Duration::minutes(ARM_MINUTES)))
    } else {
        (state::CALLING, None)
    };
    let device_label = format!("{} · {}", target.extension.number, target.device.name);
    let sql = format!(
        "INSERT INTO wifi_surveys (tenant_id, device_id, extension_id, phone_id, device_label,
             mac, label, mode, state, started_by, armed_until, started_at, thresholds, unifi,
             expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                 now() + make_interval(days => $15))
         RETURNING {COLUMNS}"
    );
    let survey = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(target.device.id)
        .bind(target.extension.id)
        .bind(target.device.phone_id)
        .bind(device_label)
        .bind(&target.mac)
        .bind(label)
        .bind(&input.mode)
        .bind(st)
        .bind(started_by)
        .bind(armed_until)
        .bind((input.mode == "originate").then_some(now))
        .bind(serde_json::to_value(&t).unwrap_or_default())
        .bind(unifi)
        .bind(t.retention_days)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match CoreError::from(e) {
            CoreError::Conflict(_) => {
                CoreError::Conflict("a survey already runs on this device".into())
            }
            other => other,
        })?;
    tx.commit().await?;
    Ok(survey)
}

pub async fn get<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<Survey> {
    let sql = format!("SELECT {COLUMNS} FROM wifi_surveys WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

/// Any tenant: the survey of a call.
/// True while a survey of the tenant is armed, calling or running. No
/// UniFi write may run then (ADR 0042).
pub async fn any_active(pool: &PgPool, tenant: TenantId) -> CoreResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM wifi_surveys WHERE tenant_id = $1 AND state = ANY($2))",
    )
    .bind(tenant)
    .bind(state::ACTIVE)
    .fetch_one(pool)
    .await?)
}

pub async fn by_call(pool: &PgPool, call: Uuid) -> CoreResult<Option<(TenantId, Survey)>> {
    let sql = format!("SELECT tenant_id, {COLUMNS} FROM wifi_surveys WHERE call_uuid = $1");
    let row: Option<SurveyRow> = sqlx::query_as(&sql).bind(call).fetch_optional(pool).await?;
    Ok(row.map(|r| (r.tenant_id, r.survey)))
}

#[derive(FromRow)]
struct SurveyRow {
    tenant_id: TenantId,
    #[sqlx(flatten)]
    survey: Survey,
}

/// Surveys, newest first; `owner` limits the list to surveys a user started.
pub async fn list(pool: &PgPool, tenant: TenantId, owner: Option<Uuid>) -> CoreResult<Vec<Survey>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM wifi_surveys WHERE tenant_id = $1
           AND ($2::uuid IS NULL OR started_by = $2)
         ORDER BY created_at DESC LIMIT 500"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(owner)
        .fetch_all(pool)
        .await?)
}

/// Surveys with a call that still runs, in all tenants.
pub async fn active(pool: &PgPool) -> CoreResult<Vec<(TenantId, Survey)>> {
    let sql = format!("SELECT tenant_id, {COLUMNS} FROM wifi_surveys WHERE state = ANY($1)");
    let rows: Vec<SurveyRow> = sqlx::query_as(&sql)
        .bind(state::ACTIVE)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| (r.tenant_id, r.survey)).collect())
}

/// The armed survey of a device for a self-dial call. Moves it to `calling`
/// with the call UUID; `None` if no survey waits.
pub async fn claim_armed(
    pool: &PgPool,
    tenant: TenantId,
    device: Uuid,
    call: Uuid,
) -> CoreResult<Option<Survey>> {
    let sql = format!(
        "UPDATE wifi_surveys SET state = 'calling', call_uuid = $3, started_at = now(),
             armed_until = NULL
         WHERE tenant_id = $1 AND device_id = $2 AND state = 'armed' AND armed_until > now()
         RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(device)
        .bind(call)
        .fetch_optional(pool)
        .await?)
}

/// Stores the call UUID of an originated survey.
pub async fn set_call(pool: &PgPool, tenant: TenantId, id: Uuid, call: Uuid) -> CoreResult<()> {
    sqlx::query("UPDATE wifi_surveys SET call_uuid = $3 WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .bind(call)
        .execute(pool)
        .await?;
    Ok(())
}

/// The handset answered: the measurement starts.
pub async fn mark_answered(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    at: DateTime<Utc>,
    recording_file: Option<&str>,
) -> CoreResult<bool> {
    let n = sqlx::query(
        "UPDATE wifi_surveys SET state = 'running', answered_at = $3, recording_file = $4
         WHERE tenant_id = $1 AND id = $2 AND state = 'calling'",
    )
    .bind(tenant)
    .bind(id)
    .bind(at)
    .bind(recording_file)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n == 1)
}

/// Codec and packet time of the answered call.
pub async fn set_media(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    codec: &str,
    ptime_ms: i32,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE wifi_surveys SET codec = $3, ptime_ms = $4 WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(codec)
    .bind(ptime_ms.clamp(10, 120))
    .execute(pool)
    .await?;
    Ok(())
}

/// Ends a survey whose call did not start or was stopped before answer.
pub async fn fail(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    new_state: &str,
    error: &str,
) -> CoreResult<bool> {
    let n = sqlx::query(
        "UPDATE wifi_surveys SET state = $3, error = $4, ended_at = COALESCE(ended_at, now())
         WHERE tenant_id = $1 AND id = $2 AND state = ANY($5)",
    )
    .bind(tenant)
    .bind(id)
    .bind(new_state)
    .bind(error.chars().take(300).collect::<String>())
    .bind(state::ACTIVE)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n == 1)
}

/// The call ended after answer: queue the analysis. Returns false if the
/// survey was not running (already finished).
pub async fn finish(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    hangup_cause: &str,
    recording_secs: Option<i32>,
) -> CoreResult<bool> {
    let mut tx = pool.begin().await?;
    let n = sqlx::query(
        "UPDATE wifi_surveys SET state = 'analysing', ended_at = COALESCE(ended_at, now()),
             hangup_cause = $3, recording_secs = COALESCE($4, recording_secs)
         WHERE tenant_id = $1 AND id = $2 AND state = 'running'",
    )
    .bind(tenant)
    .bind(id)
    .bind(hangup_cause.chars().take(64).collect::<String>())
    .bind(recording_secs)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 1 {
        let mut job = NewJob::new(JOB_ANALYSE, serde_json::json!({ "survey_id": id }));
        job.tenant_id = tenant;
        jobs::enqueue(&mut *tx, job).await?;
    }
    tx.commit().await?;
    Ok(n == 1)
}

/// Queues the analysis again (new thresholds).
pub async fn reanalyse(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let t = settings(pool, tenant).await?;
    let mut tx = pool.begin().await?;
    let n = sqlx::query(
        "UPDATE wifi_surveys SET state = 'analysing', thresholds = $3
         WHERE tenant_id = $1 AND id = $2 AND state IN ('analysed', 'analysing')
           AND answered_at IS NOT NULL",
    )
    .bind(tenant)
    .bind(id)
    .bind(serde_json::to_value(&t).unwrap_or_default())
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n != 1 {
        return Err(CoreError::Conflict(
            "only a finished survey can be analysed again".into(),
        ));
    }
    let mut job = NewJob::new(JOB_ANALYSE, serde_json::json!({ "survey_id": id }));
    job.tenant_id = tenant;
    jobs::enqueue(&mut *tx, job).await?;
    tx.commit().await?;
    Ok(())
}

/// Uplink media values for one poll.
#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MediaSample {
    pub offset_ms: i32,
    pub interval_ms: i32,
    /// Packets received in the interval.
    pub received: i32,
    /// Packets expected in the interval (interval / packet time).
    pub expected: i32,
    /// New flaws (lost or late packets) in the interval.
    pub flaws: i32,
    /// Estimated MOS of FreeSWITCH since the call start.
    pub mos: Option<f32>,
    /// Largest jitter variance of FreeSWITCH since the call start.
    pub jitter_max_ms: Option<f32>,
    /// Downlink loss (percent) of the latest RTCP report from the handset.
    pub rtcp_fraction_lost: Option<f32>,
    /// Downlink jitter (ms) of the latest RTCP report from the handset.
    pub rtcp_jitter_ms: Option<f32>,
}

/// Wi-Fi state of the handset for one poll.
#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RadioSample {
    pub offset_ms: i32,
    pub ap_mac: Option<String>,
    pub ap_name: Option<String>,
    pub bssid: Option<String>,
    /// UniFi radio: `ng` (2.4 GHz), `na` (5 GHz), `6e` (6 GHz).
    pub radio: Option<String>,
    pub channel: Option<i32>,
    pub rssi: Option<i32>,
    pub tx_rate_kbps: Option<i32>,
    pub rx_rate_kbps: Option<i32>,
    /// Poll error class; the other values are empty then.
    pub error: Option<String>,
}

/// A marker, an error period of FreeSWITCH or a state change.
#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Event {
    pub id: Uuid,
    pub offset_ms: i32,
    /// `marker`, `error_period` or `state`.
    pub kind: String,
    /// `disturbance`, `location` or `note`.
    pub marker: Option<String>,
    /// `dtmf` or `ui`.
    pub source: Option<String>,
    pub note: String,
    #[schema(value_type = Object)]
    pub data: serde_json::Value,
}

pub async fn add_media(
    pool: &PgPool,
    tenant: TenantId,
    survey: Uuid,
    at: DateTime<Utc>,
    s: &MediaSample,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO wifi_survey_media_samples (survey_id, tenant_id, offset_ms, at, interval_ms,
             received, expected, flaws, mos, jitter_max_ms, rtcp_fraction_lost, rtcp_jitter_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT DO NOTHING",
    )
    .bind(survey)
    .bind(tenant)
    .bind(s.offset_ms)
    .bind(at)
    .bind(s.interval_ms)
    .bind(s.received)
    .bind(s.expected)
    .bind(s.flaws)
    .bind(s.mos)
    .bind(s.jitter_max_ms)
    .bind(s.rtcp_fraction_lost)
    .bind(s.rtcp_jitter_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_radio(
    pool: &PgPool,
    tenant: TenantId,
    survey: Uuid,
    at: DateTime<Utc>,
    s: &RadioSample,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO wifi_survey_radio_samples (survey_id, tenant_id, offset_ms, at, ap_mac,
             ap_name, bssid, radio, channel, rssi, tx_rate_kbps, rx_rate_kbps, error)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         ON CONFLICT DO NOTHING",
    )
    .bind(survey)
    .bind(tenant)
    .bind(s.offset_ms)
    .bind(at)
    .bind(&s.ap_mac)
    .bind(&s.ap_name)
    .bind(&s.bssid)
    .bind(&s.radio)
    .bind(s.channel)
    .bind(s.rssi)
    .bind(s.tx_rate_kbps)
    .bind(s.rx_rate_kbps)
    .bind(&s.error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Maps a DTMF digit to a marker; `#` ends the test and is not a marker.
pub fn dtmf_marker(digit: char) -> Option<&'static str> {
    match digit {
        '1' => Some("disturbance"),
        '2' => Some("location"),
        '3' => Some("note"),
        _ => None,
    }
}

/// Adds a marker from DTMF or the UI.
pub async fn add_marker(
    pool: &PgPool,
    tenant: TenantId,
    survey: &Survey,
    marker: &str,
    source: &str,
    note: &str,
    at: DateTime<Utc>,
) -> CoreResult<Event> {
    if !["disturbance", "location", "note"].contains(&marker) {
        return Err(CoreError::Validation("unknown marker".into()));
    }
    let note = clean_note(note)?;
    Ok(sqlx::query_as(
        "INSERT INTO wifi_survey_events (survey_id, tenant_id, offset_ms, at, kind, marker,
             source, note)
         VALUES ($1, $2, $3, $4, 'marker', $5, $6, $7)
         RETURNING id, offset_ms, kind, marker, source, note, data",
    )
    .bind(survey.id)
    .bind(tenant)
    .bind(survey.offset_ms(at))
    .bind(at)
    .bind(marker)
    .bind(source)
    .bind(note)
    .fetch_one(pool)
    .await?)
}

fn clean_note(note: &str) -> CoreResult<String> {
    let note = note.trim();
    if note.chars().count() > MAX_NOTE || note.chars().any(char::is_control) {
        return Err(CoreError::Validation("invalid note".into()));
    }
    Ok(note.to_owned())
}

/// Changes the note of a marker.
pub async fn set_note(
    pool: &PgPool,
    tenant: TenantId,
    survey: Uuid,
    event: Uuid,
    note: &str,
) -> CoreResult<()> {
    let note = clean_note(note)?;
    let n = sqlx::query(
        "UPDATE wifi_survey_events SET note = $4
         WHERE tenant_id = $1 AND survey_id = $2 AND id = $3 AND kind = 'marker'",
    )
    .bind(tenant)
    .bind(survey)
    .bind(event)
    .bind(note)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Adds an error period from the call record (uplink loss period).
pub async fn add_error_period(
    pool: &PgPool,
    tenant: TenantId,
    survey: &Survey,
    start: DateTime<Utc>,
    stop: DateTime<Utc>,
    flaws: i64,
) -> CoreResult<()> {
    let start_ms = survey.offset_ms(start);
    sqlx::query(
        "INSERT INTO wifi_survey_events (survey_id, tenant_id, offset_ms, at, kind, data)
         SELECT $1, $2, $3, $4, 'error_period', $5
         WHERE NOT EXISTS (SELECT 1 FROM wifi_survey_events
                           WHERE survey_id = $1 AND kind = 'error_period' AND offset_ms = $3)",
    )
    .bind(survey.id)
    .bind(tenant)
    .bind(start_ms)
    .bind(start)
    .bind(serde_json::json!({
        "end_ms": survey.offset_ms(stop),
        "flaws": flaws,
    }))
    .execute(pool)
    .await?;
    Ok(())
}

/// All stored values of a survey.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct Timeline {
    pub media: Vec<MediaSample>,
    pub radio: Vec<RadioSample>,
    pub events: Vec<Event>,
}

/// Values after `since_ms` (all with `-1`).
pub async fn timeline(
    pool: &PgPool,
    tenant: TenantId,
    survey: Uuid,
    since_ms: i32,
) -> CoreResult<Timeline> {
    let media = sqlx::query_as(
        "SELECT offset_ms, interval_ms, received, expected, flaws, mos, jitter_max_ms,
                rtcp_fraction_lost, rtcp_jitter_ms
         FROM wifi_survey_media_samples WHERE tenant_id = $1 AND survey_id = $2 AND offset_ms > $3
         ORDER BY offset_ms",
    )
    .bind(tenant)
    .bind(survey)
    .bind(since_ms)
    .fetch_all(pool)
    .await?;
    let radio = sqlx::query_as(
        "SELECT offset_ms, ap_mac, ap_name, bssid, radio, channel, rssi, tx_rate_kbps,
                rx_rate_kbps, error
         FROM wifi_survey_radio_samples WHERE tenant_id = $1 AND survey_id = $2 AND offset_ms > $3
         ORDER BY offset_ms",
    )
    .bind(tenant)
    .bind(survey)
    .bind(since_ms)
    .fetch_all(pool)
    .await?;
    let events = sqlx::query_as(
        "SELECT id, offset_ms, kind, marker, source, note, data
         FROM wifi_survey_events WHERE tenant_id = $1 AND survey_id = $2 AND offset_ms > $3
         ORDER BY offset_ms, at",
    )
    .bind(tenant)
    .bind(survey)
    .bind(since_ms)
    .fetch_all(pool)
    .await?;
    Ok(Timeline {
        media,
        radio,
        events,
    })
}

/// Stored roams of an analysed survey.
pub async fn roams(pool: &PgPool, tenant: TenantId, survey: Uuid) -> CoreResult<Vec<Roam>> {
    #[derive(FromRow)]
    struct Row {
        offset_ms: i32,
        from_ap_mac: Option<String>,
        from_ap_name: Option<String>,
        to_ap_mac: Option<String>,
        to_ap_name: Option<String>,
        from_radio: Option<String>,
        to_radio: Option<String>,
        from_channel: Option<i32>,
        to_channel: Option<i32>,
        rssi_before: Option<i32>,
        rssi_after: Option<i32>,
        gap_ms: i32,
        loss_pct: f32,
        jitter_ms: Option<f32>,
        rating: String,
        flags: Vec<String>,
        markers: i32,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT offset_ms, from_ap_mac, from_ap_name, to_ap_mac, to_ap_name, from_radio,
                to_radio, from_channel, to_channel, rssi_before, rssi_after, gap_ms, loss_pct,
                jitter_ms, rating, flags, markers
         FROM wifi_survey_roams WHERE tenant_id = $1 AND survey_id = $2 ORDER BY offset_ms",
    )
    .bind(tenant)
    .bind(survey)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Roam {
            offset_ms: r.offset_ms,
            from_ap_mac: r.from_ap_mac,
            from_ap_name: r.from_ap_name,
            to_ap_mac: r.to_ap_mac,
            to_ap_name: r.to_ap_name,
            from_radio: r.from_radio,
            to_radio: r.to_radio,
            from_channel: r.from_channel,
            to_channel: r.to_channel,
            rssi_before: r.rssi_before,
            rssi_after: r.rssi_after,
            gap_ms: r.gap_ms,
            loss_pct: r.loss_pct,
            jitter_ms: r.jitter_ms,
            rating: Rating::parse(&r.rating).unwrap_or(Rating::Bad),
            flags: r.flags,
            markers: r.markers,
        })
        .collect())
}

/// Runs the analysis of a survey and stores roams and summary.
pub async fn analyse_and_store(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Analysis> {
    let survey = get(pool, tenant, id).await?;
    let tl = timeline(pool, tenant, id, -1).await?;
    let result = analysis::analyse(
        &tl.media,
        &tl.radio,
        &tl.events,
        survey.ptime_ms,
        &survey.thresholds(),
    );
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM wifi_survey_roams WHERE tenant_id = $1 AND survey_id = $2")
        .bind(tenant)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for r in &result.roams {
        sqlx::query(
            "INSERT INTO wifi_survey_roams (survey_id, tenant_id, offset_ms, from_ap_mac,
                 from_ap_name, to_ap_mac, to_ap_name, from_radio, to_radio, from_channel,
                 to_channel, rssi_before, rssi_after, gap_ms, loss_pct, jitter_ms, rating, flags,
                 markers)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                     $18, $19)",
        )
        .bind(id)
        .bind(tenant)
        .bind(r.offset_ms)
        .bind(&r.from_ap_mac)
        .bind(&r.from_ap_name)
        .bind(&r.to_ap_mac)
        .bind(&r.to_ap_name)
        .bind(&r.from_radio)
        .bind(&r.to_radio)
        .bind(r.from_channel)
        .bind(r.to_channel)
        .bind(r.rssi_before)
        .bind(r.rssi_after)
        .bind(r.gap_ms)
        .bind(r.loss_pct)
        .bind(r.jitter_ms)
        .bind(r.rating.as_str())
        .bind(&r.flags)
        .bind(r.markers)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "UPDATE wifi_surveys SET summary = $3,
             state = CASE WHEN state = 'analysing' THEN 'analysed' ELSE state END
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(serde_json::to_value(&result.summary).unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(result)
}

/// Deletes a survey with all values. Returns the recording file to remove.
pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Option<String>> {
    let file: Option<Option<String>> = sqlx::query_scalar(
        "DELETE FROM wifi_surveys WHERE tenant_id = $1 AND id = $2 AND NOT (state = ANY($3))
         RETURNING recording_file",
    )
    .bind(tenant)
    .bind(id)
    .bind(state::ACTIVE)
    .fetch_optional(pool)
    .await?;
    match file {
        Some(f) => Ok(f),
        None => {
            // Exists but active, or does not exist.
            get(pool, tenant, id).await?;
            Err(CoreError::Conflict("stop the survey first".into()))
        }
    }
}

/// Deletes expired surveys (all tenants). Returns their recording files.
pub async fn purge_expired(pool: &PgPool) -> CoreResult<Vec<String>> {
    let files: Vec<Option<String>> = sqlx::query_scalar(
        "DELETE FROM wifi_surveys WHERE expires_at < now() AND NOT (state = ANY($1))
         RETURNING recording_file",
    )
    .bind(state::ACTIVE)
    .fetch_all(pool)
    .await?;
    Ok(files.into_iter().flatten().collect())
}

/// Ends armed surveys whose time ran out.
pub async fn expire_armed(pool: &PgPool) -> CoreResult<u64> {
    Ok(sqlx::query(
        "UPDATE wifi_surveys SET state = 'cancelled', error = 'self-dial time expired',
             ended_at = now()
         WHERE state = 'armed' AND armed_until <= now()",
    )
    .execute(pool)
    .await?
    .rows_affected())
}

/// Recording file of a survey, relative to the recordings volume.
pub fn recording_file(tenant: TenantId, survey: Uuid) -> String {
    format!("wifi-surveys/{tenant}/{survey}.wav")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_and_thresholds() {
        assert_eq!(dtmf_marker('1'), Some("disturbance"));
        assert_eq!(dtmf_marker('2'), Some("location"));
        assert_eq!(dtmf_marker('#'), None);
        assert!(Thresholds::default().validate().is_ok());
        let bad = Thresholds {
            clean_ms: 200,
            bad_ms: 150,
            ..Thresholds::default()
        };
        assert!(bad.validate().is_err());
        assert!(clean_note("a\nb").is_err());
        assert_eq!(clean_note("  Hall  ").unwrap(), "Hall");
        assert_eq!(
            recording_file(TenantId::DEFAULT, Uuid::nil()),
            format!("wifi-surveys/{}/{}.wav", TenantId::DEFAULT, Uuid::nil())
        );
    }
}
