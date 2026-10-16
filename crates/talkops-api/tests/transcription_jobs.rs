//! Manual transcriptions, the job list with its permissions, job actions
//! and worker reports.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::tenant::TenantId;
use talkops_core::transcription_jobs::{self as tj, WorkerStatus};

use common::*;

async fn recording(pool: &PgPool, call: &str) -> String {
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO recordings (tenant_id, call_uuid, file, duration_secs)
         VALUES ($1, $2, 'x.wav', 120) RETURNING id",
    )
    .bind(TenantId::DEFAULT)
    .bind(call)
    .fetch_one(pool)
    .await
    .unwrap();
    id.to_string()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn manual_transcriptions_and_job_control(pool: PgPool) {
    let router = router(pool.clone());
    let admin = setup_admin(&router).await;
    sqlx::query("UPDATE tenant_settings SET transcription_enabled = true")
        .execute(&pool)
        .await
        .unwrap();
    let rec = recording(&pool, "call-1").await;

    // Options: local models with estimates; the default first pass is marked.
    let (status, options) = admin
        .get(&format!("/api/v1/recordings/{rec}/transcribe"))
        .await;
    assert_eq!(status, StatusCode::OK, "{options}");
    assert_eq!(options["audio_secs"], 120);
    let list = options["options"].as_array().unwrap();
    assert_eq!(list.len(), 5, "local models without cloud connections");
    let fast = list
        .iter()
        .find(|o| o["choice"]["model"] == "fast")
        .unwrap();
    assert_eq!(fast["default"], true);
    assert_eq!(fast["measured"], false);
    assert_eq!(fast["eta_secs"].as_f64().unwrap(), 120.0);

    // A measured speed replaces the default estimate.
    tj::record_speed(&pool, "local:accurate", 60.0, 90.0)
        .await
        .unwrap();
    let (_, options) = admin
        .get(&format!("/api/v1/recordings/{rec}/transcribe"))
        .await;
    let accurate = options["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["choice"]["model"] == "accurate")
        .unwrap()
        .clone();
    assert_eq!(accurate["measured"], true);
    assert_eq!(accurate["eta_secs"].as_f64().unwrap(), 180.0);

    // Start, then a second request for the same recording is refused.
    let body = json!({"engine": {"kind": "local", "model": "accurate"}});
    let (status, started) = admin
        .post(
            &format!("/api/v1/recordings/{rec}/transcribe"),
            body.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let job = started["job_id"].as_str().unwrap().to_owned();
    let (status, _) = admin
        .post(&format!("/api/v1/recordings/{rec}/transcribe"), body)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = admin
        .post(
            &format!("/api/v1/recordings/{rec}/transcribe"),
            json!({"engine": {"kind": "local", "model": "huge"}}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, jobs) = admin.get("/api/v1/transcription-jobs").await;
    let row = &jobs.as_array().unwrap()[0];
    assert_eq!(row["id"], job.as_str());
    assert_eq!(row["status"], "queued");
    assert_eq!(row["engine_key"], "local:accurate");
    assert_eq!(row["requested_by"], "Admin");
    assert_eq!(row["eta_secs"].as_f64().unwrap(), 180.0);

    // Pause and resume a queued job, then stop it.
    let act = |action: &str| json!({ "action": action });
    let path = format!("/api/v1/transcription-jobs/{job}");
    assert_eq!(
        admin.post(&path, act("pause")).await.0,
        StatusCode::NO_CONTENT
    );
    let (_, jobs) = admin.get("/api/v1/transcription-jobs").await;
    assert_eq!(jobs[0]["status"], "paused");
    assert_eq!(jobs[0]["eta_secs"], serde_json::Value::Null);
    assert_eq!(
        admin.post(&path, act("pause")).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        admin.post(&path, act("resume")).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        admin.post(&path, act("prioritize")).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        admin.post(&path, act("stop")).await.0,
        StatusCode::NO_CONTENT
    );
    let (_, jobs) = admin.get("/api/v1/transcription-jobs").await;
    assert!(jobs.as_array().unwrap().is_empty());
    let (_, history) = admin.get("/api/v1/transcription-jobs?history=true").await;
    assert_eq!(history[0]["status"], "cancelled");

    // Workers that reported in the last minute.
    tj::report_worker(
        &pool,
        &WorkerStatus {
            worker_id: "media-1".into(),
            cpu_percent: 350.0,
            cpus: 4,
            rss_bytes: 2_000_000_000,
            mem_total_bytes: 8_000_000_000,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let (status, workers) = admin.get("/api/v1/transcription-workers").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(workers[0]["worker_id"], "media-1");
    assert_eq!(workers[0]["cpus"], 4);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn permissions_limit_the_jobs_a_user_sees(pool: PgPool) {
    let router = router(pool.clone());
    let admin = setup_admin(&router).await;
    sqlx::query("UPDATE tenant_settings SET transcription_enabled = true")
        .execute(&pool)
        .await
        .unwrap();
    let rec = recording(&pool, "call-1").await;
    let (_, user) = admin
        .post(
            "/api/v1/users",
            json!({"username": "bob", "display_name": "Bob", "role": "user", "password": "correct-horse"}),
        )
        .await;
    let bob_id = user["id"].as_str().unwrap().to_owned();
    let bob = login(&router, "bob", "correct-horse").await.unwrap();
    // Without permissions: no page, no manual transcription.
    assert_eq!(
        bob.get("/api/v1/transcription-jobs").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        bob.get(&format!("/api/v1/recordings/{rec}/transcribe"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    // The admin starts a job; the default user role gets view and
    // re-transcribe permissions.
    let (_, started) = admin
        .post(
            &format!("/api/v1/recordings/{rec}/transcribe"),
            json!({"engine": {"kind": "local", "model": "fast"}}),
        )
        .await;
    let admin_job = started["job_id"].as_str().unwrap().to_owned();
    let (_, overview) = admin.get("/api/v1/roles").await;
    let user_role = overview["roles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["builtin"] == "user")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    admin
        .put(
            &format!("/api/v1/roles/{user_role}"),
            json!({"name": "Default user", "permissions": ["transcriptions.view", "recordings.retranscribe"]}),
        )
        .await;
    let (status, jobs) = bob.get("/api/v1/transcription-jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(jobs.as_array().unwrap().is_empty(), "not Bob's job: {jobs}");
    assert_eq!(
        bob.post(
            &format!("/api/v1/transcription-jobs/{admin_job}"),
            json!({"action": "stop"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // Bob cannot reach a recording of other extensions.
    assert_eq!(
        bob.post(
            &format!("/api/v1/recordings/{rec}/transcribe"),
            json!({"engine": {"kind": "local", "model": "base"}})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // A job that Bob started is his.
    sqlx::query("UPDATE jobs SET requested_by = $1")
        .bind(uuid::Uuid::parse_str(&bob_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (_, jobs) = bob.get("/api/v1/transcription-jobs").await;
    assert_eq!(jobs.as_array().unwrap().len(), 1);
    assert_eq!(
        bob.get("/api/v1/transcription-workers").await.0,
        StatusCode::FORBIDDEN,
        "resources need transcriptions.manage"
    );
}
