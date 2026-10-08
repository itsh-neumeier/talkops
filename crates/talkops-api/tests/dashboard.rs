//! Dashboard: call statistics and calls in progress.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::*;

async fn cdr(db: &PgPool, direction: &str, minutes_ago: i64, answered_secs: Option<i32>) {
    sqlx::query(
        "INSERT INTO cdr (tenant_id, call_uuid, direction, started_at, answered_at, ended_at,
                          billsec)
         VALUES ('00000000-0000-4000-8000-000000000001', gen_random_uuid()::text,
                 $1::call_direction, now() - $2 * interval '1 minute',
                 CASE WHEN $3::int IS NULL THEN NULL ELSE now() - $2 * interval '1 minute' END,
                 now() - $2 * interval '1 minute' + interval '1 minute', COALESCE($3, 0))",
    )
    .bind(direction)
    .bind(minutes_ago)
    .bind(answered_secs)
    .execute(db)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn call_statistics(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    cdr(&db, "inbound", 2, Some(60)).await;
    cdr(&db, "inbound", 3, None).await;
    cdr(&db, "inbound", 30, Some(120)).await;
    cdr(&db, "outbound", 10, Some(30)).await;
    cdr(&db, "internal", 90, Some(30)).await;
    cdr(&db, "internal", 60 * 24 * 3, Some(30)).await;

    let (status, s) = admin.get("/api/v1/stats/calls?range=1h").await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["buckets"].as_array().unwrap().len(), 12);
    assert_eq!(
        (s["inbound"].clone(), s["outbound"].clone()),
        (json!(3), json!(1))
    );
    assert_eq!(s["internal"], 0, "90 minutes ago is outside the hour");
    assert_eq!(s["missed"], 1);
    assert_eq!(s["answer_rate"], 66.7);
    assert_eq!(s["avg_talk_secs"], 70);

    let (_, s) = admin.get("/api/v1/stats/calls").await;
    assert_eq!(s["range"], "1d");
    assert_eq!(s["buckets"].as_array().unwrap().len(), 24);
    assert_eq!(s["total"], 5);
    let (_, s) = admin.get("/api/v1/stats/calls?range=1w").await;
    assert_eq!(s["buckets"].as_array().unwrap().len(), 7);
    assert_eq!(s["total"], 6);
    let (status, _) = admin.get("/api/v1/stats/calls?range=1y").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Without FreeSWITCH there are no calls in progress.
    let (status, calls) = admin.get("/api/v1/telephony/calls").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls, json!([]));

    // Users see neither.
    admin
        .post(
            "/api/v1/users",
            json!({"username": "ben", "display_name": "Ben", "password": "ben-password-1",
                   "role": "user"}),
        )
        .await;
    let ben = login(&router, "ben", "ben-password-1").await.unwrap();
    assert_eq!(
        ben.get("/api/v1/stats/calls").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        ben.get("/api/v1/telephony/calls").await.0,
        StatusCode::FORBIDDEN
    );
}
