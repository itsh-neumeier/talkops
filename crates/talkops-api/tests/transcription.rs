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

    // Two passes: quick local first, API second.
    let (_, mut s) = admin.get("/api/v1/settings").await;
    assert_eq!(s["transcription_refine"], "");
    s["transcription_quality"] = json!("fast");
    s["transcription_refine"] = json!("api");
    let (status, body) = admin.put("/api/v1/settings", s.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["transcription_refine"], "api");
    s["transcription_refine"] = json!("huge");
    let (status, _) = admin.put("/api/v1/settings", s.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    s["transcription_refine"] = json!("");
    s["transcription_quality"] = json!("api");
    let (status, _) = admin.put("/api/v1/settings", s).await;
    assert_eq!(status, StatusCode::OK);

    // API settings: admin only, key never returned, stored encrypted.
    let (status, _) = ben.get("/api/v1/settings/transcription-api").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, a) = admin.get("/api/v1/settings/transcription-api").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        a,
        json!({"url": "https://api.openai.com/v1", "model": "whisper-1", "has_key": false})
    );
    for bad in [
        json!({"url": "ftp://x", "model": "whisper-1"}),
        json!({"url": "https://x/v1", "model": "a b"}),
        json!({"url": "https://x/v1", "model": "m", "key": "with space"}),
    ] {
        let (status, _) = admin.put("/api/v1/settings/transcription-api", bad).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let url = fake_api().await;
    let (status, a) = admin
        .put(
            "/api/v1/settings/transcription-api",
            json!({"url": format!("{url}/"), "model": "whisper-1", "key": "sk-good"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{a}");
    assert_eq!(
        a,
        json!({"url": url, "model": "whisper-1", "has_key": true})
    );
    let stored: Option<String> =
        sqlx::query_scalar("SELECT transcription_api_key_enc FROM tenant_settings")
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(!stored.unwrap().contains("sk-good"));
    let cfg = transcription_api::config(&db, TenantId::DEFAULT, &SecretBox::from_hex(KEY).unwrap())
        .await
        .unwrap();
    assert_eq!(cfg.key.as_deref(), Some("sk-good"));
    assert_eq!(cfg.endpoint(), format!("{url}/audio/transcriptions"));

    // Test: lists the server's speech models.
    let (status, t) = admin
        .post("/api/v1/settings/transcription-api/test", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{t}");
    assert_eq!(t["model_found"], true);
    assert_eq!(
        t["speech_models"],
        json!(["gpt-4o-transcribe", "whisper-1"])
    );

    // `null` keeps the key, a wrong key fails the test, "" removes it.
    let (_, a) = admin
        .put(
            "/api/v1/settings/transcription-api",
            json!({"url": url, "model": "whisper-1"}),
        )
        .await;
    assert_eq!(a["has_key"], true);
    admin
        .put(
            "/api/v1/settings/transcription-api",
            json!({"url": url, "model": "whisper-1", "key": "sk-bad"}),
        )
        .await;
    let (status, t) = admin
        .post("/api/v1/settings/transcription-api/test", json!({}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{t}");
    let (_, a) = admin
        .put(
            "/api/v1/settings/transcription-api",
            json!({"url": url, "model": "whisper-1", "key": ""}),
        )
        .await;
    assert_eq!(a["has_key"], false);
}
