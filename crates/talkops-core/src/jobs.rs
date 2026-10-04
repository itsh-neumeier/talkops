//! Postgres-backed job queue (see ADR 0005).
//!
//! Producers insert rows with [`enqueue`] and send a `NOTIFY` on
//! [`NOTIFY_CHANNEL`]. Workers claim jobs with `FOR UPDATE SKIP LOCKED`, so any
//! number of workers can run concurrently without double-processing. Failed jobs
//! are retried with exponential backoff until `max_attempts` is reached.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::tenant::TenantId;

/// Channel used to wake idle workers when a job is enqueued.
pub const NOTIFY_CHANNEL: &str = "talkops_jobs";

/// Base delay for retry backoff: attempt n waits `RETRY_BASE_SECS * 2^(n-1)`.
const RETRY_BASE_SECS: i64 = 30;
/// Upper bound for the retry delay.
const RETRY_MAX_SECS: i64 = 6 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type)]
#[sqlx(type_name = "job_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub kind: String,
    pub payload: serde_json::Value,
    pub status: JobStatus,
    pub priority: i16,
    pub attempts: i32,
    pub max_attempts: i32,
    pub run_at: DateTime<Utc>,
    pub locked_by: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewJob {
    pub tenant_id: TenantId,
    pub kind: String,
    pub payload: serde_json::Value,
    pub priority: i16,
    pub max_attempts: i32,
    pub run_at: Option<DateTime<Utc>>,
}

impl NewJob {
    pub fn new(kind: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            tenant_id: TenantId::DEFAULT,
            kind: kind.into(),
            payload,
            priority: 0,
            max_attempts: 5,
            run_at: None,
        }
    }
}

const JOB_COLUMNS: &str = "id, tenant_id, kind, payload, status, priority, attempts, max_attempts, run_at, locked_by, last_error";

/// Inserts a job and notifies listening workers. Accepts any executor so that a
/// job can be enqueued inside the transaction that produced it.
pub async fn enqueue<'e>(db: impl PgExecutor<'e>, job: NewJob) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "WITH inserted AS (
             INSERT INTO jobs (tenant_id, kind, payload, priority, max_attempts, run_at)
             VALUES ($1, $2, $3, $4, $5, COALESCE($6, now()))
             RETURNING id, kind
         )
         SELECT id FROM inserted, pg_notify($7, inserted.kind)",
    )
    .bind(job.tenant_id)
    .bind(&job.kind)
    .bind(&job.payload)
    .bind(job.priority)
    .bind(job.max_attempts)
    .bind(job.run_at)
    .bind(NOTIFY_CHANNEL)
    .fetch_one(db)
    .await
}

/// Atomically claims the next due job of one of `kinds`, or returns `None`.
pub async fn claim(
    pool: &PgPool,
    worker_id: &str,
    kinds: &[&str],
) -> Result<Option<Job>, sqlx::Error> {
    let sql = format!(
        "UPDATE jobs SET status = 'running', attempts = attempts + 1, locked_by = $1,
                         locked_at = now(), updated_at = now()
         WHERE id = (
             SELECT id FROM jobs
             WHERE status = 'queued' AND run_at <= now() AND kind = ANY($2)
             ORDER BY priority DESC, run_at
             LIMIT 1
             FOR UPDATE SKIP LOCKED
         )
         RETURNING {JOB_COLUMNS}"
    );
    sqlx::query_as::<_, Job>(&sql)
        .bind(worker_id)
        .bind(kinds)
        .fetch_optional(pool)
        .await
}

/// Marks a running job as successfully finished.
pub async fn complete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET status = 'done', locked_by = NULL, locked_at = NULL,
                         finished_at = now(), updated_at = now()
         WHERE id = $1 AND status = 'running'",
    )
    .bind(id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Records a failure. The job is re-queued with backoff while attempts remain,
/// otherwise it is marked as failed permanently. Returns the new status.
pub async fn fail(pool: &PgPool, id: Uuid, error: &str) -> Result<JobStatus, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE jobs SET
             status = CASE WHEN attempts >= max_attempts THEN 'failed'::job_status ELSE 'queued'::job_status END,
             run_at = CASE WHEN attempts >= max_attempts THEN run_at
                           ELSE now() + make_interval(secs => LEAST($3::bigint, $2::bigint * (1::bigint << LEAST(attempts - 1, 30)))) END,
             finished_at = CASE WHEN attempts >= max_attempts THEN now() ELSE NULL END,
             last_error = $4, locked_by = NULL, locked_at = NULL, updated_at = now()
         WHERE id = $1 AND status = 'running'
         RETURNING status",
    )
    .bind(id)
    .bind(RETRY_BASE_SECS)
    .bind(RETRY_MAX_SECS)
    .bind(error)
    .fetch_one(pool)
    .await
}

/// Re-queues jobs whose worker vanished (e.g. container restart) while holding
/// them for longer than `timeout_secs`. Returns the number of recovered jobs.
pub async fn requeue_stale(pool: &PgPool, timeout_secs: i64) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET status = 'queued', locked_by = NULL, locked_at = NULL,
                         last_error = 'worker lease expired', updated_at = now()
         WHERE status = 'running' AND locked_at < now() - make_interval(secs => $1::bigint)",
    )
    .bind(timeout_secs)
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}
