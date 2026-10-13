//! Transcription settings: passes, API settings with encrypted key, test.

mod common;

use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::tenant::TenantId;
use talkops_core::transcription_api;

use common::*;

/// A fake OpenAI-compatible server: `/v1/models` for key `sk-good`.
async fn fake_api() -> String {
    let app = Router::new().route(
        "/v1/models",
        get(|headers: HeaderMap| async move {
            if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("Bearer sk-good")
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(json!({"error": {"message": "bad key"}})),
                );
            }
            (
                StatusCode::OK,
                axum::Json(json!({"data": [
                    {"id": "gpt-4o-mini"}, {"id": "whisper-1"}, {"id": "gpt-4o-transcribe"}
                ]})),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://127.0.0.1:{port}/v1")
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn transcription_settings(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    admin
        .post(
            "/api/v1/users",
            json!({"username": "ben", "display_name": "Ben", "password": "ben-password-1", "role": "operator"}),
        )
        .await;
    let ben = login(&router, "ben", "ben-password-1").await.unwrap();

    // Admin only; defaults without connections.
    let (status, _) = ben.get("/api/v1/settings/transcription").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = ben
        .post(
            "/api/v1/settings/transcription/providers",
            json!({"name": "X", "kind": "openai"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, o) = admin.get("/api/v1/settings/transcription").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(o["providers"], json!([]));
    assert_eq!(o["profiles"][0]["purpose"], "live");
    assert_eq!(o["profiles"][0]["local_model"], "base");
    assert_eq!(o["profiles"][2]["engine"], "off");
    assert!(
        o["openai_prices"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["model"] == "gpt-live-transcribe")
    );

    // Connections: validated, key never returned, stored encrypted.
    for bad in [
        json!({"name": "X", "kind": "compatible", "url": "ftp://x"}),
        json!({"name": "", "kind": "openai"}),
        json!({"name": "X", "kind": "openai", "key": "with space"}),
    ] {
        let (status, _) = admin
            .post("/api/v1/settings/transcription/providers", bad)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let url = fake_api().await;
    let (status, p) = admin
        .post(
            "/api/v1/settings/transcription/providers",
            json!({"name": "LAN", "kind": "compatible", "url": format!("{url}/"), "key": "sk-good"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{p}");
    let id = p["id"].as_str().unwrap().to_owned();
    assert_eq!(
        p,
        json!({"id": id, "name": "LAN", "kind": "compatible", "url": url, "has_key": true})
    );
    let stored: String = sqlx::query_scalar("SELECT key_enc FROM transcription_providers")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!stored.contains("sk-good"));
    let uuid = id.parse().unwrap();
    let cfg = transcription_api::config(
        &db,
        TenantId::DEFAULT,
        &SecretBox::from_hex(KEY).unwrap(),
        uuid,
        "whisper-1",
    )
    .await
    .unwrap();
    assert_eq!(cfg.key.as_deref(), Some("sk-good"));
    assert_eq!(cfg.endpoint(), format!("{url}/audio/transcriptions"));

    // Profiles: cloud for live (streaming) and the second recording pass.
    let mut profiles = o["profiles"].clone();
    profiles[0] = json!({"purpose": "live", "engine": "cloud", "local_model": "base",
        "provider_id": id, "model": "gpt-live-transcribe", "realtime": true});
    profiles[2] = json!({"purpose": "recording_refine", "engine": "cloud", "local_model": "fast",
        "provider_id": id, "model": "whisper-1", "price_per_minute": 0.002});
    let (status, saved) = admin
        .put("/api/v1/settings/transcription/profiles", profiles.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved[0]["realtime"], true);
    assert_eq!(saved[2]["price_per_minute"], 0.002);
    // A compatible server has no list price.
    assert_eq!(saved[2]["list_price"], json!(null));
    let mut bad = profiles.clone();
    bad[1]["engine"] = json!("off");
    let (status, _) = admin
        .put("/api/v1/settings/transcription/profiles", bad)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = ben
        .put("/api/v1/settings/transcription/profiles", profiles)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Test: lists the server's speech models and the profile models it lacks.
    let (status, t) = admin
        .post(
            &format!("/api/v1/settings/transcription/providers/{id}/test"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{t}");
    assert_eq!(t["missing_models"], json!(["gpt-live-transcribe"]));
    assert_eq!(
        t["speech_models"],
        json!(["gpt-4o-transcribe", "whisper-1"])
    );

    // `null` keeps the key, a wrong key fails the test, "" removes it.
    let path = format!("/api/v1/settings/transcription/providers/{id}");
    let (_, p) = admin
        .put(
            &path,
            json!({"name": "LAN", "kind": "compatible", "url": url}),
        )
        .await;
    assert_eq!(p["has_key"], true);
    admin
        .put(
            &path,
            json!({"name": "LAN", "kind": "compatible", "url": url, "key": "sk-bad"}),
        )
        .await;
    let (status, t) = admin.post(&format!("{path}/test"), json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{t}");
    let (_, p) = admin
        .put(
            &path,
            json!({"name": "LAN", "kind": "compatible", "url": url, "key": ""}),
        )
        .await;
    assert_eq!(p["has_key"], false);

    // In use: no delete. After switching back to local, delete works.
    let (status, e) = admin.call("DELETE", &path, None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        e["message"],
        "connection is in use for: live, recording_refine"
    );
    let (status, _) = admin
        .put(
            "/api/v1/settings/transcription/profiles",
            json!([o["profiles"][0], o["profiles"][2]]),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin.call("DELETE", &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE entity_type LIKE 'transcription%' AND details::text LIKE '%sk-%'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audit, 0, "no keys in the audit log");
}
