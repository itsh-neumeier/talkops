//! Job queue integration tests. They need a Postgres server; `sqlx::test`
//! creates a fresh database per test from `DATABASE_URL`.

use serde_json::json;
use sqlx::PgPool;
use talkops_core::jobs::{self, JobStatus, NewJob};

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn claim_complete_roundtrip(pool: PgPool) {
    let id = jobs::enqueue(&pool, NewJob::new("transcribe", json!({"recording": 1})))
        .await
        .unwrap();

    assert!(jobs::claim(&pool, "w1", &["tts"]).await.unwrap().is_none());

    let job = jobs::claim(&pool, "w1", &["transcribe"])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.id, id);
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(job.attempts, 1);
    assert_eq!(job.payload, json!({"recording": 1}));

    // A claimed job is invisible to other workers.
    assert!(
        jobs::claim(&pool, "w2", &["transcribe"])
            .await
            .unwrap()
            .is_none()
    );

    jobs::complete(&pool, id).await.unwrap();
    let status: JobStatus = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, JobStatus::Done);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn failures_retry_with_backoff_then_fail(pool: PgPool) {
    let mut job = NewJob::new("tts", json!({}));
    job.max_attempts = 2;
    let id = jobs::enqueue(&pool, job).await.unwrap();

    jobs::claim(&pool, "w", &["tts"]).await.unwrap().unwrap();
    assert_eq!(
        jobs::fail(&pool, id, "boom").await.unwrap(),
        JobStatus::Queued
    );

    // Backoff pushes run_at into the future, so the job is not claimable yet.
    assert!(jobs::claim(&pool, "w", &["tts"]).await.unwrap().is_none());

    sqlx::query("UPDATE jobs SET run_at = now() WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    jobs::claim(&pool, "w", &["tts"]).await.unwrap().unwrap();
    assert_eq!(
        jobs::fail(&pool, id, "boom again").await.unwrap(),
        JobStatus::Failed
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn stale_jobs_are_requeued(pool: PgPool) {
    let id = jobs::enqueue(&pool, NewJob::new("tts", json!({})))
        .await
        .unwrap();
    jobs::claim(&pool, "dead-worker", &["tts"])
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE jobs SET locked_at = now() - interval '1 hour' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(jobs::requeue_stale(&pool, 600).await.unwrap(), 1);
    let job = jobs::claim(&pool, "w", &["tts"]).await.unwrap().unwrap();
    assert_eq!(job.attempts, 2);
}
