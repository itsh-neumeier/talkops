//! Short-lived, bounded preview jobs. These never start a recording.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, types::Json};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::jobs::{self, NewJob};
use crate::recordings::Segment;
use crate::tenant::TenantId;

pub const JOB: &str = "transcribe.live";
pub const MAX_SEGMENTS: usize = 200;

/// Bounded technical state for a tenant's support report. No call identities
/// or transcript contents are selected. Normal preview retention still applies.
pub async fn diagnostic_states(
    pool: &PgPool,
    tenant: TenantId,
) -> CoreResult<Vec<serde_json::Value>> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('preview_id', id, 'processed_secs', processed_secs, 'engine', engine, 'fallback_from', fallback_from, 'stream_failed_at', stream_failed_at) FROM live_transcript_previews WHERE tenant_id = $1 AND requested_at > now() - interval '1 hour' ORDER BY requested_at DESC LIMIT 20")
        .bind(tenant).fetch_all(pool).await?)
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Preview {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub processed_secs: f32,
    #[schema(value_type = Vec<Segment>)]
    pub segments: Json<Vec<Segment>>,
    /// queued, running, done, or failed; text is always provisional.
    pub status: String,
    /// The engine of the latest text, e.g. `api:Coral/coral-auto`.
    pub engine: String,
    /// The selected engine that is not available now, while a local
    /// fallback makes the text (ADR 0046).
    pub fallback_from: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct Work {
    pub file: String,
    pub cursor_frames: i64,
    pub segments: Json<Vec<Segment>>,
}

pub fn valid_file(file: &str, tenant: TenantId, call: Uuid) -> bool {
    let parts: Vec<_> = file.split('/').collect();
    parts.len() == 3
        && parts[0] == tenant.to_string()
        && parts[1].len() == 7
        && parts[1].as_bytes()[4] == b'-'
        && parts[1]
            .bytes()
            .enumerate()
            .all(|(i, c)| i == 4 || c.is_ascii_digit())
        && parts[2] == format!("{call}.wav")
}

/// One pending job per viewer/call. The tenant lock also enforces the limit.
pub async fn request(
    pool: &PgPool,
    tenant: TenantId,
    user: Uuid,
    call: Uuid,
    file: &str,
    started_at: DateTime<Utc>,
) -> CoreResult<(Preview, bool)> {
    if !valid_file(file, tenant, call) {
        return Err(CoreError::Validation("invalid recording identity".into()));
    }
    cleanup(pool).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)")
        .bind(format!("live:{tenant}"))
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM live_transcript_previews WHERE tenant_id = $1 AND requested_at > now() - interval '30 seconds' AND NOT (user_id = $2 AND call_uuid = $3)")
        .bind(tenant).bind(user).bind(call).fetch_one(&mut *tx).await?;
    if count >= 4 {
        return Err(CoreError::Conflict("live preview capacity reached".into()));
    }
    let (id, created): (Uuid, bool) = sqlx::query_as(
        "INSERT INTO live_transcript_previews (tenant_id, user_id, call_uuid, file, started_at)
        SELECT $1, id, $3, $4, $5 FROM users WHERE id = $2 AND tenant_id = $1
        ON CONFLICT (tenant_id, user_id, call_uuid) DO UPDATE SET requested_at = now()
        RETURNING id, (xmax = 0)",
    )
    .bind(tenant)
    .bind(user)
    .bind(call)
    .bind(file)
    .bind(started_at)
    .fetch_one(&mut *tx)
    .await?;
    let previous_status: Option<String> = sqlx::query_scalar("SELECT j.status::text FROM live_transcript_previews p JOIN jobs j ON j.id = p.job_id WHERE p.id = $1")
        .bind(id).fetch_optional(&mut *tx).await?;
    let pending = matches!(previous_status.as_deref(), Some("queued" | "running"));
    if !pending {
        let mut job = NewJob::new(JOB, serde_json::json!({"preview_id": id}));
        job.tenant_id = tenant;
        job.max_attempts = 1;
        let job_id = jobs::enqueue(&mut *tx, job).await?;
        sqlx::query("UPDATE live_transcript_previews SET job_id = $2 WHERE id = $1")
            .bind(id)
            .bind(job_id)
            .execute(&mut *tx)
            .await?;
    }
    let mut preview: Preview = sqlx::query_as("SELECT p.id, p.started_at, p.processed_secs, p.segments, CASE WHEN p.stream_failed_at > now() - interval '30 seconds' AND p.fallback_from IS NULL THEN 'failed' ELSE j.status::text END AS status, p.engine, p.fallback_from FROM live_transcript_previews p JOIN jobs j ON j.id = p.job_id WHERE p.id = $1")
        .bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    if previous_status.as_deref() == Some("failed") && preview.fallback_from.is_none() {
        preview.status = "failed".into();
    }
    Ok((preview, created))
}

pub async fn stop(pool: &PgPool, tenant: TenantId, user: Uuid, call: Uuid) -> CoreResult<()> {
    sqlx::query("DELETE FROM live_transcript_previews WHERE tenant_id = $1 AND user_id = $2 AND call_uuid = $3")
        .bind(tenant).bind(user).bind(call).execute(pool).await?;
    Ok(())
}

pub async fn work(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Option<Work>> {
    Ok(sqlx::query_as("SELECT file, cursor_frames, segments FROM live_transcript_previews WHERE id = $1 AND tenant_id = $2 AND requested_at > now() - interval '30 seconds'")
        .bind(id).bind(tenant).fetch_optional(pool).await?)
}

/// Compare-and-set prevents repeated jobs from appending the same audio twice.
pub async fn save(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    previous: i64,
    cursor: i64,
    seconds: f32,
    segments: Vec<Segment>,
) -> CoreResult<bool> {
    let segments: Vec<_> = segments
        .into_iter()
        .rev()
        .take(MAX_SEGMENTS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Ok(sqlx::query("UPDATE live_transcript_previews SET cursor_frames = $4, processed_secs = $5, segments = $6 WHERE id = $1 AND tenant_id = $2 AND cursor_frames = $3 AND requested_at > now() - interval '30 seconds'")
        .bind(id).bind(tenant).bind(previous).bind(cursor).bind(seconds).bind(Json(segments)).execute(pool).await?.rows_affected() == 1)
}

/// Takes the realtime stream of a preview for a worker. Fails while another
/// worker streams it, the viewer is gone, or it failed in the last 30 s.
pub async fn claim_stream(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    owner: &str,
) -> CoreResult<bool> {
    Ok(sqlx::query("UPDATE live_transcript_previews SET stream_owner = $3, stream_beat = now() WHERE id = $1 AND tenant_id = $2 AND requested_at > now() - interval '30 seconds' AND (stream_beat IS NULL OR stream_beat < now() - interval '20 seconds' OR stream_owner = $3) AND (stream_failed_at IS NULL OR stream_failed_at < now() - interval '30 seconds')")
        .bind(id).bind(tenant).bind(owner).execute(pool).await?.rows_affected() == 1)
}

/// Keeps a stream alive; `false` means stop (viewer gone or lease lost).
pub async fn beat_stream(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    owner: &str,
) -> CoreResult<bool> {
    Ok(sqlx::query("UPDATE live_transcript_previews SET stream_beat = now() WHERE id = $1 AND tenant_id = $2 AND stream_owner = $3 AND requested_at > now() - interval '30 seconds'")
        .bind(id).bind(tenant).bind(owner).execute(pool).await?.rows_affected() == 1)
}

/// Records the engine of the preview text and, for a local fallback, the
/// engine it replaces.
pub async fn set_engine(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    engine: &str,
    fallback_from: Option<&str>,
) -> CoreResult<()> {
    sqlx::query("UPDATE live_transcript_previews SET engine = $3, fallback_from = $4 WHERE id = $1 AND tenant_id = $2")
        .bind(id).bind(tenant).bind(engine).bind(fallback_from).execute(pool).await?;
    Ok(())
}

/// A realtime stream of the preview failed in the last 30 s (the backoff
/// of ADR 0024): a Coral preview uses its local fallback meanwhile.
pub async fn stream_failed_recently(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<bool> {
    Ok(sqlx::query_scalar::<_, Option<bool>>("SELECT stream_failed_at > now() - interval '30 seconds' FROM live_transcript_previews WHERE id = $1 AND tenant_id = $2")
        .bind(id).bind(tenant).fetch_optional(pool).await?.flatten().unwrap_or(false))
}

/// Ends a stream; a failed one is not restarted for 30 s.
pub async fn release_stream(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    owner: &str,
    failed: bool,
) -> CoreResult<()> {
    sqlx::query("UPDATE live_transcript_previews SET stream_owner = NULL, stream_beat = NULL, stream_failed_at = CASE WHEN $4 THEN now() ELSE stream_failed_at END WHERE id = $1 AND tenant_id = $2 AND stream_owner = $3")
        .bind(id).bind(tenant).bind(owner).bind(failed).execute(pool).await?;
    Ok(())
}

pub async fn cleanup(pool: &PgPool) -> CoreResult<()> {
    sqlx::query(
        "DELETE FROM live_transcript_previews WHERE requested_at < now() - interval '5 minutes'",
    )
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM jobs WHERE kind = $1 AND status IN ('done', 'failed') AND finished_at < now() - interval '5 minutes'")
        .bind(JOB).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn user(pool: &PgPool) -> Uuid {
        sqlx::query_scalar("INSERT INTO users (tenant_id, username, display_name) VALUES ($1, $2, 'Test viewer') RETURNING id")
            .bind(TenantId::DEFAULT).bind(Uuid::new_v4().to_string()).fetch_one(pool).await.unwrap()
    }

    fn file(call: Uuid) -> String {
        format!("{}/2026-10/{call}.wav", TenantId::DEFAULT)
    }

    #[test]
    fn accepts_only_generated_recording_paths() {
        let call = Uuid::new_v4();
        assert!(valid_file(&file(call), TenantId::DEFAULT, call));
        for bad in ["../2026-10/a.wav", "C:/a.wav", "tenant/../../a.wav"] {
            assert!(!valid_file(bad, TenantId::DEFAULT, call));
        }
        assert!(!valid_file(&file(call), TenantId(Uuid::new_v4()), call));
        assert!(!valid_file(&file(call), TenantId::DEFAULT, Uuid::new_v4()));
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn preview_jobs_are_bounded_and_isolated(pool: PgPool) {
        let tenant = TenantId::DEFAULT;
        let user = user(&pool).await;
        let call = Uuid::new_v4();
        let (first, created) = request(&pool, tenant, user, call, &file(call), Utc::now())
            .await
            .unwrap();
        assert!(created);
        let (second, created) = request(&pool, tenant, user, call, &file(call), Utc::now())
            .await
            .unwrap();
        assert!(!created);
        assert_eq!(first.id, second.id);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = $1")
            .bind(JOB)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert!(
            work(&pool, TenantId(Uuid::new_v4()), first.id)
                .await
                .unwrap()
                .is_none()
        );
        let job = jobs::claim(&pool, "test", &[JOB]).await.unwrap().unwrap();
        jobs::fail(&pool, job.id, "test failure").await.unwrap();
        assert_eq!(
            request(&pool, tenant, user, call, &file(call), Utc::now())
                .await
                .unwrap()
                .0
                .status,
            "failed"
        );
        for _ in 0..3 {
            let id = Uuid::new_v4();
            request(&pool, tenant, user, id, &file(id), Utc::now())
                .await
                .unwrap();
        }
        let extra = Uuid::new_v4();
        assert!(
            request(&pool, tenant, user, extra, &file(extra), Utc::now())
                .await
                .is_err()
        );
        stop(&pool, tenant, Uuid::new_v4(), call).await.unwrap();
        assert!(work(&pool, tenant, first.id).await.unwrap().is_some());
        stop(&pool, tenant, user, call).await.unwrap();
        assert!(work(&pool, tenant, first.id).await.unwrap().is_none());
        assert!(
            request(&pool, tenant, user, extra, &file(extra), Utc::now())
                .await
                .is_ok()
        );
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn cursor_updates_are_idempotent_and_previews_expire(pool: PgPool) {
        let tenant = TenantId::DEFAULT;
        let user = user(&pool).await;
        let call = Uuid::new_v4();
        let preview = request(&pool, tenant, user, call, &file(call), Utc::now())
            .await
            .unwrap()
            .0;
        let segments = (0..250)
            .map(|i| Segment {
                start: i as f32,
                end: i as f32 + 1.0,
                speaker: "caller".into(),
                text: format!("line {i}"),
                language: None,
            })
            .collect();
        assert!(
            save(&pool, tenant, preview.id, -1, 8000, 1.0, segments)
                .await
                .unwrap()
        );
        assert!(
            !save(&pool, tenant, preview.id, -1, 8000, 1.0, vec![])
                .await
                .unwrap()
        );
        assert_eq!(
            work(&pool, tenant, preview.id)
                .await
                .unwrap()
                .unwrap()
                .segments
                .0
                .len(),
            MAX_SEGMENTS
        );
        sqlx::query(
            "UPDATE live_transcript_previews SET requested_at = now() - interval '31 seconds'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(work(&pool, tenant, preview.id).await.unwrap().is_none());
        assert!(
            !save(&pool, tenant, preview.id, 8000, 16000, 2.0, vec![])
                .await
                .unwrap()
        );
        sqlx::query(
            "UPDATE live_transcript_previews SET requested_at = now() - interval '6 minutes'",
        )
        .execute(&pool)
        .await
        .unwrap();
        cleanup(&pool).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM live_transcript_previews")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let job = jobs::claim(&pool, "test", &[JOB]).await.unwrap().unwrap();
        jobs::complete(&pool, job.id).await.unwrap();
        sqlx::query("UPDATE jobs SET finished_at = now() - interval '6 minutes' WHERE id = $1")
            .bind(job.id)
            .execute(&pool)
            .await
            .unwrap();
        let other = jobs::enqueue(&pool, NewJob::new("unrelated", serde_json::json!({})))
            .await
            .unwrap();
        cleanup(&pool).await.unwrap();
        let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM jobs")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(!ids.contains(&job.id));
        assert!(ids.contains(&other));
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn realtime_streams_have_one_owner(pool: PgPool) {
        let tenant = TenantId::DEFAULT;
        let user = user(&pool).await;
        let call = Uuid::new_v4();
        let preview = request(&pool, tenant, user, call, &file(call), Utc::now())
            .await
            .unwrap()
            .0;
        assert!(claim_stream(&pool, tenant, preview.id, "a").await.unwrap());
        assert!(claim_stream(&pool, tenant, preview.id, "a").await.unwrap());
        assert!(!claim_stream(&pool, tenant, preview.id, "b").await.unwrap());
        assert!(
            !claim_stream(&pool, TenantId(Uuid::new_v4()), preview.id, "a")
                .await
                .unwrap()
        );
        assert!(beat_stream(&pool, tenant, preview.id, "a").await.unwrap());
        assert!(!beat_stream(&pool, tenant, preview.id, "b").await.unwrap());
        // A stale lease goes to another worker.
        sqlx::query(
            "UPDATE live_transcript_previews SET stream_beat = now() - interval '21 seconds'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(claim_stream(&pool, tenant, preview.id, "b").await.unwrap());
        assert!(!beat_stream(&pool, tenant, preview.id, "a").await.unwrap());
        // A failed stream pauses and shows as failed.
        release_stream(&pool, tenant, preview.id, "b", true)
            .await
            .unwrap();
        assert!(!claim_stream(&pool, tenant, preview.id, "a").await.unwrap());
        let job = jobs::claim(&pool, "test", &[JOB]).await.unwrap().unwrap();
        jobs::complete(&pool, job.id).await.unwrap();
        let again = request(&pool, tenant, user, call, &file(call), Utc::now())
            .await
            .unwrap()
            .0;
        assert_eq!(again.status, "failed");
        // The viewer leaves: no new stream.
        sqlx::query("UPDATE live_transcript_previews SET stream_failed_at = NULL, requested_at = now() - interval '31 seconds'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(!claim_stream(&pool, tenant, preview.id, "a").await.unwrap());
    }
}
