//! Diagnostics API without a running FreeSWITCH.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use sqlx::PgPool;

use common::*;

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn diagnostics_api(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    admin
        .post(
            "/api/v1/users",
            json!({"username": "ben", "display_name": "Ben", "password": "ben-password-1", "role": "operator"}),
        )
        .await;
    let ben = login(&router, "ben", "ben-password-1").await.unwrap();

    let (status, d) = admin.get("/api/v1/diagnostics").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(d["capture"]["running"], false);
    assert!(d["freeswitch"].is_null());
    assert_eq!(d["channels"], json!([]));
    assert!(d["server_started_at"].is_string());
    let (status, _) = ben.get("/api/v1/diagnostics").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Validation before touching FreeSWITCH.
    let (status, _) = admin
        .post("/api/v1/diagnostics/capture", json!({"level": "trace"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .post("/api/v1/diagnostics/capture", json!({"minutes": 0}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // No event socket configured in tests.
    let (status, _) = admin
        .post("/api/v1/diagnostics/capture", json!({"sip_trace": true}))
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (status, _) = ben.post("/api/v1/diagnostics/capture", json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, page) = admin.get("/api/v1/diagnostics/log?after=0").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["lines"], json!([]));
    let (status, _) = admin
        .call("DELETE", "/api/v1/diagnostics/capture", None)
        .await;
    assert_eq!(status, StatusCode::OK);

    let res = raw(
        &router,
        Request::get("/api/v1/diagnostics/log.txt")
            .header(header::COOKIE, &admin.cookie)
            .header("x-requested-with", "TalkOps")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment; filename=\"talkops-freeswitch-")
    );

    let (status, _) = admin
        .post("/api/v1/diagnostics/restart", json!({"target": "router"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // FreeSWITCH not connected: nothing is restarted (and the test process
    // keeps running).
    let (status, _) = admin
        .post(
            "/api/v1/diagnostics/restart",
            json!({"target": "freeswitch"}),
        )
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (status, _) = ben
        .post("/api/v1/diagnostics/restart", json!({"target": "all"}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
