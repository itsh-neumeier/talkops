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

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Preview {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub processed_secs: f32,
    #[schema(value_type = Vec<Segment>)]
    pub segments: Json<Vec<Segment>>,
    /// queued, running, done, or failed; text is always provisional.
    pub status: String,
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
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)")
        .bind(format!("live:{tenant}"))
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM live_transcript_previews WHERE requested_at < now() - interval '5 minutes'",
    )
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
    let mut preview: Preview = sqlx::query_as("SELECT p.id, p.started_at, p.processed_secs, p.segments, j.status::text AS status FROM live_transcript_previews p JOIN jobs j ON j.id = p.job_id WHERE p.id = $1")
        .bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    if previous_status.as_deref() == Some("failed") {
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

pub async fn cleanup(pool: &PgPool) -> CoreResult<()> {
    sqlx::query(
        "DELETE FROM live_transcript_previews WHERE requested_at < now() - interval '5 minutes'",
    )
    .execute(pool)
    .await?;
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
    }
}
