//! Transcription jobs as the UI sees them (ADR 0030): the list with progress
//! and estimates, manual transcriptions with a selected engine, pause,
//! resume and stop, the measured speed of each engine and the resource
//! reports of the media workers.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::cdr::Direction;
use crate::error::{CoreError, CoreResult};
use crate::jobs::{self, JobStatus, NewJob};
use crate::recordings::{self, Source};
use crate::settings::TranscriptionQuality;
use crate::tenant::TenantId;
use crate::transcription_api;
use crate::transcription_profiles::{self as profiles, Engine};

/// An engine selected for one manual transcription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EngineChoice {
    /// A local Whisper model: `base`, `fast`, `accurate`, `best`, `german`.
    Local { model: String },
    /// A model of a cloud connection.
    Cloud { provider_id: Uuid, model: String },
}

impl EngineChoice {
    /// Key of the speed statistics, e.g. `local:fast` or `cloud:whisper-1`.
    pub fn key(&self) -> String {
        match self {
            Self::Local { model } => format!("local:{model}"),
            Self::Cloud { model, .. } => format!("cloud:{model}"),
        }
    }

    /// Resolves the choice into an engine; checks model and connection.
    pub async fn engine(&self, pool: &PgPool, tenant: TenantId) -> CoreResult<Engine> {
        match self {
            Self::Local { model } => TranscriptionQuality::parse(model)
                .map(Engine::Local)
                .ok_or_else(|| CoreError::Validation(format!("unknown local model {model}"))),
            Self::Cloud { provider_id, model } => {
                if !transcription_api::valid_model(model.trim()) {
                    return Err(CoreError::Validation("invalid model name".into()));
                }
                let provider = transcription_api::get(pool, tenant, *provider_id).await?;
                let coral = provider.kind == "coral";
                Ok(Engine::Cloud {
                    provider_id: provider.id,
                    model: model.trim().to_owned(),
                    realtime: false,
                    coral_stable_preview: false,
                    price: profiles::list_price(&provider.kind, model.trim()),
                    coral,
                    fallback: coral.then_some(TranscriptionQuality::Fast),
                })
            }
        }
    }
}

/// The speed key of an engine (see [EngineChoice::key]).
pub fn engine_key(engine: &Engine) -> String {
    match engine {
        Engine::Local(q) => format!("local:{}", q.as_str()),
        Engine::Cloud { model, .. } => format!("cloud:{model}"),
    }
}

/// Seconds of processing for each second of a stereo call recording when
/// nothing was measured yet (4 CPU threads, both channels).
pub fn default_speed(key: &str) -> f32 {
    match key {
        "local:base" => 0.3,
        "local:fast" | "local:german" => 1.0,
        "local:accurate" => 3.0,
        "local:best" => 6.0,
        _ => 0.15,
    }
}

/// Measured speeds by engine key.
pub async fn speeds(pool: &PgPool) -> CoreResult<HashMap<String, f32>> {
    let rows: Vec<(String, f32)> =
        sqlx::query_as("SELECT engine, secs_per_audio_sec FROM transcription_speed")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().collect())
}

/// Adds a measurement: an average that follows changes of the hardware.
pub async fn record_speed(
    pool: &PgPool,
    key: &str,
    audio_secs: f32,
    wall_secs: f32,
) -> CoreResult<()> {
    if !(audio_secs >= 5.0 && wall_secs > 0.0 && wall_secs.is_finite()) {
        return Ok(());
    }
    let factor = wall_secs / audio_secs;
    sqlx::query(
        "INSERT INTO transcription_speed (engine, secs_per_audio_sec, samples) VALUES ($1, $2, 1)
         ON CONFLICT (engine) DO UPDATE SET
             secs_per_audio_sec = CASE WHEN transcription_speed.samples < 4
                 THEN (transcription_speed.secs_per_audio_sec * transcription_speed.samples + $2)
                      / (transcription_speed.samples + 1)
                 ELSE transcription_speed.secs_per_audio_sec * 0.75 + $2 * 0.25 END,
             samples = transcription_speed.samples + 1,
             updated_at = now()",
    )
    .bind(key)
    .bind(factor)
    .execute(pool)
    .await?;
    Ok(())
}

/// Payload of a `transcribe` job (shared with the media worker).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Payload {
    pub recording_id: Option<Uuid>,
    pub voicemail_id: Option<Uuid>,
    /// 1 = quick first pass (default), 2 = more accurate second pass.
    #[serde(default = "first_pass")]
    pub pass: u8,
    /// A manual transcription with this engine: one final pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineChoice>,
}

fn first_pass() -> u8 {
    1
}

/// Starts a manual transcription of a recording with the selected engine.
/// The current transcript stays until the new one is complete.
pub async fn enqueue_manual(
    pool: &PgPool,
    tenant: TenantId,
    recording: Uuid,
    choice: &EngineChoice,
    user: Uuid,
) -> CoreResult<Uuid> {
    choice.engine(pool, tenant).await?;
    let rec = recordings::get(pool, tenant, recording).await?;
    let busy: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM jobs WHERE tenant_id = $1 AND kind = $2
             AND status IN ('queued', 'running', 'paused')
             AND payload->>'recording_id' = $3::text AND payload ? 'engine')",
    )
    .bind(tenant)
    .bind(recordings::JOB_TRANSCRIBE)
    .bind(rec.id)
    .fetch_one(pool)
    .await?;
    if busy {
        return Err(CoreError::Conflict(
            "a manual transcription of this recording is already waiting or running".into(),
        ));
    }
    let payload = Payload {
        recording_id: Some(rec.id),
        voicemail_id: None,
        pass: 1,
        engine: Some(choice.clone()),
    };
    let mut job = NewJob::new(
        recordings::JOB_TRANSCRIBE,
        serde_json::to_value(&payload).map_err(|e| CoreError::Validation(e.to_string()))?,
    );
    job.tenant_id = tenant;
    job.max_attempts = 2;
    let id = jobs::enqueue(pool, job).await?;
    sqlx::query("UPDATE jobs SET requested_by = $2 WHERE id = $1")
        .bind(id)
        .bind(user)
        .execute(pool)
        .await?;
    Ok(id)
}

/// A transcription job in the list.
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct JobView {
    pub id: Uuid,
    #[schema(value_type = String)]
    pub status: JobStatus,
    pub priority: i16,
    /// 0 to 1; none for jobs that did not start.
    pub progress: Option<f32>,
    pub pass: i32,
    /// Manual transcription: the selected engine.
    #[schema(value_type = Option<Object>)]
    pub engine: Option<sqlx::types::Json<EngineChoice>>,
    /// Engine key for the estimate (`local:fast`, …).
    pub engine_key: String,
    pub recording_id: Option<Uuid>,
    pub voicemail_id: Option<Uuid>,
    /// Length of the audio in seconds.
    pub audio_secs: i32,
    /// Caller of the recorded call or of the voicemail.
    pub caller_number: Option<String>,
    pub caller_name: Option<String>,
    pub destination: Option<String>,
    /// Direction of the recorded call; none for a voicemail.
    pub direction: Option<Direction>,
    /// Own extension of the call, or the voicemail box.
    #[serde(skip)]
    pub extension_id: Option<Uuid>,
    /// Answering or called extension of the call.
    #[serde(skip)]
    pub dest_extension_id: Option<Uuid>,
    /// Own phone number that the caller dialed.
    #[serde(skip)]
    pub number_id: Option<Uuid>,
    pub call_started_at: Option<DateTime<Utc>>,
    pub requested_by: Option<String>,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub control: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub run_started_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub run_progress_start: f32,
    pub finished_at: Option<DateTime<Utc>>,
    /// Seconds until the end; for queued jobs including the jobs before them.
    #[sqlx(default)]
    pub eta_secs: Option<f32>,
    /// A paused job continues at its resume point.
    pub resumable: bool,
}

/// Which jobs a user sees.
#[derive(Debug, Clone)]
pub enum Scope {
    All,
    /// Jobs that the user started or of recordings and voicemails of their
    /// extensions.
    Own {
        user: Uuid,
        extensions: Vec<Uuid>,
    },
}

const VIEW: &str = "SELECT j.id, j.status, j.priority, j.progress,
        COALESCE((j.payload->>'pass')::int, 1) AS pass,
        j.payload->'engine' AS engine, '' AS engine_key,
        r.id AS recording_id, vm.id AS voicemail_id,
        COALESCE(r.duration_secs, vm.duration_secs, 0) AS audio_secs,
        COALESCE(c.caller_number, vm.caller_number) AS caller_number,
        COALESCE(c.caller_name, vm.caller_name) AS caller_name,
        COALESCE(c.destination, ve.number) AS destination,
        c.direction, COALESCE(c.extension_id, vm.extension_id) AS extension_id,
        c.dest_extension_id, c.number_id,
        COALESCE(c.started_at, vm.created_at) AS call_started_at,
        u.display_name AS requested_by, j.attempts, j.last_error, j.control,
        j.created_at, j.started_at, j.run_started_at, j.run_progress_start, j.finished_at,
        (j.checkpoint IS NOT NULL) AS resumable
     FROM jobs j
     LEFT JOIN recordings r ON r.id = (j.payload->>'recording_id')::uuid
     LEFT JOIN cdr c ON c.call_uuid = r.call_uuid
     LEFT JOIN voicemail_messages vm ON vm.id = (j.payload->>'voicemail_id')::uuid
     LEFT JOIN extensions ve ON ve.id = vm.extension_id
     LEFT JOIN users u ON u.id = j.requested_by
     WHERE j.tenant_id = $1 AND j.kind = $2
       AND ($3::uuid IS NULL OR j.requested_by = $3
            OR c.extension_id = ANY($4) OR c.dest_extension_id = ANY($4)
            OR vm.extension_id = ANY($4))";

/// Maximum number of jobs in the history. The history covers 7 days, so the
/// UI normally has the full list and filters it in the browser.
pub const HISTORY_LIMIT: i64 = 1000;

/// Active jobs (running, paused, queued) or the history of the last days.
pub async fn list(
    pool: &PgPool,
    tenant: TenantId,
    scope: &Scope,
    history: bool,
) -> CoreResult<Vec<JobView>> {
    let (user, extensions) = match scope {
        Scope::All => (None, Vec::new()),
        Scope::Own { user, extensions } => (Some(*user), extensions.clone()),
    };
    let sql = if history {
        format!(
            "{VIEW} AND j.status IN ('done', 'failed', 'cancelled')
               AND j.updated_at > now() - interval '7 days'
             ORDER BY j.updated_at DESC LIMIT {HISTORY_LIMIT}"
        )
    } else {
        format!(
            "{VIEW} AND j.status IN ('running', 'paused', 'queued')
             ORDER BY CASE j.status WHEN 'running' THEN 0 WHEN 'queued' THEN 1 ELSE 2 END,
                      j.priority DESC, j.run_at"
        )
    };
    let mut rows: Vec<JobView> = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(recordings::JOB_TRANSCRIBE)
        .bind(user)
        .bind(&extensions)
        .fetch_all(pool)
        .await?;
    let speeds = speeds(pool).await?;
    let mut keys = HashMap::new();
    for row in &mut rows {
        row.engine_key = match &row.engine {
            Some(choice) => choice.key(),
            None => {
                let voicemail = row.voicemail_id.is_some();
                let purpose = profiles::Purpose::for_pass(voicemail, row.pass as u8);
                match keys.get(purpose.as_str()) {
                    Some(key) => String::clone(key),
                    None => {
                        let engine = profiles::engine(pool, tenant, purpose).await?;
                        let key = engine.as_ref().map(engine_key).unwrap_or_default();
                        keys.insert(purpose.as_str(), key.clone());
                        key
                    }
                }
            }
        };
    }
    if !history {
        estimate(&mut rows, &speeds, Utc::now());
    }
    Ok(rows)
}

/// Remaining time of each job. A running job uses its own progress rate;
/// otherwise the measured speed of its engine. Queued jobs wait for the
/// running and the queued jobs before them (one job at a time).
pub fn estimate(rows: &mut [JobView], speeds: &HashMap<String, f32>, now: DateTime<Utc>) {
    let mut ahead = 0.0f32;
    for row in rows.iter_mut() {
        let speed = speeds
            .get(&row.engine_key)
            .copied()
            .unwrap_or_else(|| default_speed(&row.engine_key));
        let full = row.audio_secs.max(1) as f32 * speed;
        let progress = row.progress.unwrap_or(0.0).clamp(0.0, 1.0);
        let remaining = match (row.status, row.run_started_at) {
            (JobStatus::Running, Some(started)) => {
                let elapsed = (now - started).num_milliseconds() as f32 / 1000.0;
                let done = progress - row.run_progress_start;
                if done > 0.02 && elapsed > 2.0 {
                    elapsed / done * (1.0 - progress)
                } else {
                    (full * (1.0 - progress) - elapsed).max(full * 0.05)
                }
            }
            _ => full * (1.0 - progress),
        };
        row.eta_secs = match row.status {
            JobStatus::Running => Some(remaining),
            JobStatus::Queued => Some(ahead + remaining),
            _ => None,
        };
        if matches!(row.status, JobStatus::Running | JobStatus::Queued) {
            ahead += remaining;
        }
    }
}

/// An action on a job from the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Pause,
    Resume,
    Stop,
    /// Move the job to the front of the queue.
    Prioritize,
}

/// Applies an action. A running job gets a request that the worker
/// carries out within a few seconds; other jobs change at once.
pub async fn act(pool: &PgPool, tenant: TenantId, id: Uuid, action: Action) -> CoreResult<()> {
    let (status, payload): (JobStatus, serde_json::Value) = sqlx::query_as(
        "SELECT status, payload FROM jobs WHERE tenant_id = $1 AND id = $2 AND kind = $3",
    )
    .bind(tenant)
    .bind(id)
    .bind(recordings::JOB_TRANSCRIBE)
    .fetch_optional(pool)
    .await?
    .ok_or(CoreError::NotFound)?;
    let invalid = || {
        Err(CoreError::Conflict(format!(
            "the job is {}",
            serde_json::to_value(status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        )))
    };
    match (action, status) {
        (Action::Pause | Action::Stop, JobStatus::Running) => {
            let control = if action == Action::Pause {
                "pause"
            } else {
                "stop"
            };
            sqlx::query("UPDATE jobs SET control = $2, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(control)
                .execute(pool)
                .await?;
        }
        (Action::Pause, JobStatus::Queued) => {
            sqlx::query("UPDATE jobs SET status = 'paused', updated_at = now() WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
        (Action::Stop, JobStatus::Queued | JobStatus::Paused) => {
            sqlx::query(
                "UPDATE jobs SET status = 'cancelled', checkpoint = NULL, finished_at = now(),
                                 updated_at = now()
                 WHERE id = $1",
            )
            .bind(id)
            .execute(pool)
            .await?;
            after_cancel(pool, tenant, &payload).await?;
        }
        (Action::Resume, JobStatus::Paused) => {
            sqlx::query(
                "WITH resumed AS (
                     UPDATE jobs SET status = 'queued', run_at = now(), updated_at = now()
                     WHERE id = $1 RETURNING kind
                 )
                 SELECT pg_notify($2, kind) FROM resumed",
            )
            .bind(id)
            .bind(jobs::NOTIFY_CHANNEL)
            .execute(pool)
            .await?;
        }
        (Action::Prioritize, JobStatus::Queued | JobStatus::Paused) => {
            sqlx::query(
                "UPDATE jobs SET priority = (SELECT COALESCE(max(priority), 0) + 1 FROM jobs
                                             WHERE tenant_id = $2 AND kind = $3
                                               AND status = 'queued'),
                                 updated_at = now()
                 WHERE id = $1",
            )
            .bind(id)
            .bind(tenant)
            .bind(recordings::JOB_TRANSCRIBE)
            .execute(pool)
            .await?;
        }
        _ => return invalid(),
    }
    Ok(())
}

/// After a stop: an automatic first pass leaves no "pending" transcript
/// behind, and a voicemail e-mail is not held back. A manual or second
/// pass keeps the current transcript.
pub async fn after_cancel(
    pool: &PgPool,
    tenant: TenantId,
    payload: &serde_json::Value,
) -> CoreResult<()> {
    let p: Payload = serde_json::from_value(payload.clone()).unwrap_or_default();
    let source = match (p.recording_id, p.voicemail_id) {
        (Some(id), _) => Source::Recording(id),
        (None, Some(id)) => Source::Voicemail(id),
        _ => return Ok(()),
    };
    if p.engine.is_some() || p.pass >= 2 {
        return recordings::finalize_transcript(pool, source).await;
    }
    recordings::transcription_stopped(pool, source).await?;
    if let Source::Voicemail(id) = source {
        let msg = crate::voicemail::get_message(pool, tenant, id).await?;
        let mut conn = pool.acquire().await?;
        crate::voicemail::enqueue_mail(&mut conn, tenant, &msg).await?;
    }
    Ok(())
}

/// The tenant and payload of a job (for the worker after a stop).
pub async fn payload_of(pool: &PgPool, id: Uuid) -> CoreResult<(TenantId, serde_json::Value)> {
    sqlx::query_as("SELECT tenant_id, payload FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(CoreError::NotFound)
}

/// Resource usage of a media worker.
#[derive(Debug, Clone, Default, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct WorkerStatus {
    pub worker_id: String,
    /// CPU time of the worker and its Whisper processes; 100 = one core.
    pub cpu_percent: f32,
    pub cpus: i32,
    pub load1: f32,
    /// Memory of the worker and its Whisper processes.
    pub rss_bytes: i64,
    /// Memory use and limit of the container (or the host).
    pub mem_used_bytes: i64,
    pub mem_total_bytes: i64,
    /// The transcription job that runs now.
    pub job_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

pub async fn report_worker(pool: &PgPool, s: &WorkerStatus) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO worker_status (worker_id, cpu_percent, cpus, load1, rss_bytes,
                                    mem_used_bytes, mem_total_bytes, job_id, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
         ON CONFLICT (worker_id) DO UPDATE SET cpu_percent = $2, cpus = $3, load1 = $4,
             rss_bytes = $5, mem_used_bytes = $6, mem_total_bytes = $7, job_id = $8,
             updated_at = now()",
    )
    .bind(&s.worker_id)
    .bind(s.cpu_percent)
    .bind(s.cpus)
    .bind(s.load1)
    .bind(s.rss_bytes)
    .bind(s.mem_used_bytes)
    .bind(s.mem_total_bytes)
    .bind(s.job_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Workers that reported in the last minute.
pub async fn workers(pool: &PgPool) -> CoreResult<Vec<WorkerStatus>> {
    sqlx::query("DELETE FROM worker_status WHERE updated_at < now() - interval '1 day'")
        .execute(pool)
        .await?;
    Ok(sqlx::query_as(
        "SELECT * FROM worker_status WHERE updated_at > now() - interval '1 minute'
         ORDER BY worker_id",
    )
    .fetch_all(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: JobStatus, audio: i32, progress: Option<f32>) -> JobView {
        JobView {
            id: Uuid::new_v4(),
            status,
            priority: 0,
            progress,
            pass: 1,
            engine: None,
            engine_key: "local:fast".into(),
            recording_id: None,
            voicemail_id: None,
            audio_secs: audio,
            caller_number: None,
            caller_name: None,
            destination: None,
            direction: None,
            extension_id: None,
            dest_extension_id: None,
            number_id: None,
            call_started_at: None,
            requested_by: None,
            attempts: 1,
            last_error: None,
            control: None,
            created_at: Utc::now(),
            started_at: None,
            run_started_at: None,
            run_progress_start: 0.0,
            finished_at: None,
            eta_secs: None,
            resumable: false,
        }
    }

    #[test]
    fn running_jobs_use_their_rate_and_queued_jobs_wait() {
        let now = Utc::now();
        let mut running = row(JobStatus::Running, 600, Some(0.5));
        running.run_started_at = Some(now - chrono::Duration::seconds(100));
        let mut rows = vec![
            running,
            row(JobStatus::Queued, 60, None),
            row(JobStatus::Paused, 60, Some(0.3)),
        ];
        let speeds = HashMap::from([("local:fast".to_owned(), 2.0)]);
        estimate(&mut rows, &speeds, now);
        // 50 % in 100 s: 100 s left.
        assert!((rows[0].eta_secs.unwrap() - 100.0).abs() < 1.0);
        // Then 60 s × 2.
        assert!((rows[1].eta_secs.unwrap() - 220.0).abs() < 1.0);
        assert_eq!(rows[2].eta_secs, None, "paused jobs do not run");
    }

    #[test]
    fn a_resumed_run_counts_only_its_own_progress() {
        let now = Utc::now();
        let mut r = row(JobStatus::Running, 600, Some(0.6));
        r.run_progress_start = 0.5;
        r.run_started_at = Some(now - chrono::Duration::seconds(10));
        let mut rows = vec![r];
        estimate(&mut rows, &HashMap::new(), now);
        // 10 % in 10 s: 40 s left.
        assert!((rows[0].eta_secs.unwrap() - 40.0).abs() < 1.0);
    }

    #[test]
    fn choices_have_keys_and_defaults() {
        let local = EngineChoice::Local {
            model: "accurate".into(),
        };
        assert_eq!(local.key(), "local:accurate");
        assert_eq!(default_speed("local:accurate"), 3.0);
        assert_eq!(
            engine_key(&Engine::Local(TranscriptionQuality::German)),
            "local:german"
        );
        let json = serde_json::to_value(&local).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "local", "model": "accurate"})
        );
    }
}
