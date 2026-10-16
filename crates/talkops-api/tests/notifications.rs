//! Mail notifications: rules, test mail and log (ADR 0035).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn notification_rules_roundtrip(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    admin
        .post(
            "/api/v1/users",
            json!({"username": "ben", "display_name": "Ben", "password": "ben-password-1", "role": "operator"}),
        )
        .await;
    let ben = login(&router, "ben", "ben-password-1").await.unwrap();
    let (status, _) = ben.get("/api/v1/settings/notifications").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, v) = admin.get("/api/v1/settings/notifications").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let types = v["event_types"].as_array().unwrap();
    assert!(
        types
            .iter()
            .any(|t| t["key"] == "trunk.down" && t["recovery"] == "trunk.up")
    );
    assert!(!types.iter().any(|t| t["key"] == "trunk.up"));
    assert_eq!(v["rules"].as_array().unwrap().len(), types.len());

    // Save with validation, then reset.
    let path = "/api/v1/settings/notifications/rules/backup.failed";
    let (status, _) = admin
        .put(path, json!({"enabled": true, "recipient_emails": ["nope"]}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, r) = admin
        .put(
            path,
            json!({"enabled": true, "recipient_emails": ["ops@example.com"], "digest_secs": 60}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(r["custom"], true);
    assert_eq!(r["digest_secs"], 60);
    let (status, _) = admin
        .put(
            "/api/v1/settings/notifications/rules/trunk.up",
            json!({"enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Test mail without SMTP: logged as no_smtp.
    let (status, t) = admin.post(&format!("{path}/test"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{t}");
    assert_eq!(t["status"], "no_smtp");
    assert_eq!(t["recipients"], json!(["ops@example.com"]));

    let (status, r) = admin.call("DELETE", path, None).await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(r["custom"], false);

    // The rule change itself is a configuration event in the outbox.
    let (status, log) = admin.get("/api/v1/settings/notifications/log").await;
    assert_eq!(status, StatusCode::OK, "{log}");
    assert_eq!(log["deliveries"][0]["kind"], "test");
    assert_eq!(log["deliveries"][0]["status"], "no_smtp");
    assert!(log["events"].as_array().unwrap().iter().any(
        |e| e["event_type"] == "config.system" && e["params"]["entity"] == "notification_rule"
    ));
}
