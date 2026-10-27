//! Mail notifications: rules, test mail and log (ADR 0035).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::{notifications as n, tenant::TenantId};
use uuid::Uuid;

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn emergency_rule_and_authenticated_attempts_with_durable_deduplication(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (_, view) = admin.get("/api/v1/settings/notifications").await;
    assert_eq!(view["smtp_configured"], false);
    let default = view["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["event_type"] == n::EMERGENCY_DIALED)
        .unwrap();
    assert_eq!(default["enabled"], false);
    let path = format!(
        "/api/v1/settings/notifications/rules/{}",
        n::EMERGENCY_DIALED
    );
    assert_eq!(
        admin.put(&path, json!({"enabled":true})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        admin
            .put(
                &path,
                json!({"enabled":true,"recipient_roles":[Uuid::new_v4()]})
            )
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    sqlx::query("UPDATE users SET email = 'ops@example.com' WHERE username = 'admin'")
        .execute(&db)
        .await
        .unwrap();
    let (status,saved)=admin.put(&path,json!({"enabled":true,"recipient_roles":default["recipient_roles"],"digest_secs":300,"delay_secs":300,"send_recovery":true})).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["digest_secs"], 0);
    assert_eq!(saved["delay_secs"], 0);
    assert_eq!(saved["send_recovery"], false);
    let mut extensions = Vec::new();
    for number in ["201", "202"] {
        let (_, ext) = admin
            .post(
                "/api/v1/extensions",
                json!({"number":number,"display_name":"Emergency fixture"}),
            )
            .await;
        extensions.push(ext["id"].as_str().unwrap().to_owned());
    }
    let tenant = TenantId::DEFAULT.to_string();
    let first = Uuid::new_v4().to_string();
    let second = Uuid::new_v4().to_string();
    for (extension, uuid, destination, context) in [
        (&extensions[0], &first, "112", "internal"),
        (&extensions[0], &first, "112", "internal"),
        (&extensions[1], &second, "110", "internal"),
        (&extensions[0], &first, "1120", "internal"),
        (&extensions[0], &first, "112", "public"),
    ] {
        let (status, xml) = fs_post(
            &router,
            "/fs/xml",
            &[
                ("section", "dialplan"),
                ("Caller-Context", context),
                ("Caller-Destination-Number", destination),
                ("variable_talkops_tenant_id", &tenant),
                ("variable_talkops_extension_id", extension),
                ("Unique-ID", uuid),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        if context == "internal" {
            assert!(xml.contains("503 Service Unavailable"), "{xml}");
        }
    }
    let events:Vec<(String,serde_json::Value)>=sqlx::query_as("SELECT source, params FROM notification_events WHERE event_type='calls.emergency_dialed' ORDER BY id").fetch_all(&db).await.unwrap();
    assert_eq!(
        events.len(),
        2,
        "repeated lookup, ordinary numbers and inbound calls add no events"
    );
    assert_eq!(events[0].0, first);
    assert_eq!(events[0].1["extension"], "201");
    assert_eq!(events[1].1["number"], "110");
    // Concurrent insertion and a fresh database handle still use the same key.
    let id = Uuid::parse_str(&first).unwrap();
    let restarted = db.clone();
    let (a, b) = tokio::join!(
        n::emergency_dialed(&db, TenantId::DEFAULT, id, "201", "Office", "112"),
        n::emergency_dialed(&restarted, TenantId::DEFAULT, id, "201", "Office", "112")
    );
    a.unwrap();
    b.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_events WHERE event_type='calls.emergency_dialed'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(count, 2);
    let (status, test) = admin.post(&format!("{path}/test"), json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(test["status"], "no_smtp");
    assert_eq!(test["recipients"], json!(["ops@example.com"]));

    // The tenant list, not hard-coded German numbers, determines the event.
    let (_, mut settings) = admin.get("/api/v1/settings").await;
    settings["emergency_numbers"] = json!(["911"]);
    assert_eq!(
        admin.put("/api/v1/settings", settings).await.0,
        StatusCode::OK
    );
    for destination in ["911", "112"] {
        let call = Uuid::new_v4().to_string();
        fs_post(
            &router,
            "/fs/xml",
            &[
                ("section", "dialplan"),
                ("Caller-Context", "internal"),
                ("Caller-Destination-Number", destination),
                ("variable_talkops_tenant_id", &tenant),
                ("variable_talkops_extension_id", &extensions[0]),
                ("variable_uuid", &call),
            ],
        )
        .await;
    }
    let (_, log) = admin.get("/api/v1/settings/notifications/log").await;
    let events: Vec<_> = log["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["event_type"] == n::EMERGENCY_DIALED)
        .collect();
    assert_eq!(events.len(), 3);
    assert!(events.iter().any(|e| e["params"]["number"] == "911"));
}

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
