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
        Some(&SecretBox::from_hex(KEY).unwrap()),
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

/// A fake coral-stt service: `/health` with one USB TPU, `/v1/models`.
async fn fake_coral() -> String {
    let app = Router::new()
        .route(
            "/health",
            get(|| async {
                axum::Json(json!({"status": "ok", "backend": "tpu", "version": "1",
                    "tpus": [{"id": "usb:0", "kind": "usb", "state": "ready", "load": 0.1, "busy": false,
                              "inferences": 3, "models": ["coral-ctc-de", "coral-ctc-en", "langid"], "node": null, "error": null}],
                    "queue": {"length": 0, "capacity": 16, "sessions": 0, "max_sessions": 4},
                    "models": ["coral-ctc-de", "coral-ctc-en", "coral-auto"]}))
            }),
        )
        .route(
            "/v1/models",
            get(|| async {
                axum::Json(json!({"object": "list", "data": [
                    {"id": "coral-ctc-de"}, {"id": "coral-ctc-en"}, {"id": "coral-auto"}
                ]}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://127.0.0.1:{port}/v1")
}

/// A Coral connection (ADR 0046): loopback only, tested with /health and
/// /v1/models; an unreachable service fails the test.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn coral_connection(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (status, body) = admin
        .post(
            "/api/v1/settings/transcription/providers",
            json!({"name": "Coral", "kind": "coral", "url": "http://10.0.0.5:8095/v1"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let url = fake_coral().await;
    let (status, p) = admin
        .post(
            "/api/v1/settings/transcription/providers",
            json!({"name": "Coral", "kind": "coral", "url": url}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{p}");
    assert_eq!(p["kind"], "coral");
    let id = p["id"].as_str().unwrap().to_owned();
    let (status, body) = admin
        .put(
            "/api/v1/settings/transcription/profiles",
            json!([{"purpose": "live", "engine": "cloud", "local_model": "base", "provider_id": id,
                    "model": "coral-auto", "realtime": true},
                   {"purpose": "recording", "engine": "cloud", "local_model": "fast", "provider_id": id,
                    "model": "coral-auto"},
                   {"purpose": "recording_refine", "engine": "local", "local_model": "accurate", "provider_id": null, "model": ""}]),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body[1]["provider_kind"], "coral");
    let path = format!("/api/v1/settings/transcription/providers/{id}/test");
    let (status, t) = admin.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{t}");
    assert_eq!(t["missing_models"], json!([]));
    assert_eq!(t["coral"]["tpus"][0]["kind"], "usb");
    assert_eq!(t["coral"]["tpus"][0]["models"][0], "coral-ctc-de");
    assert!(
        t["speech_models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "coral-auto")
    );
    // The service is down: the test says so.
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead = closed.local_addr().unwrap().port();
    drop(closed);
    let (status, _) = admin
        .put(
            &format!("/api/v1/settings/transcription/providers/{id}"),
            json!({"name": "Coral", "kind": "coral", "url": format!("http://127.0.0.1:{dead}/v1")}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, t) = admin.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        t["message"]
            .as_str()
            .unwrap()
            .contains("speech service not reachable"),
        "{t}"
    );
}

/// A fake cuda-stt service: `/health` with the GPU, `/v1/models` and the
/// model library routes. The upload answer echoes the bytes it received.
async fn fake_cuda() -> String {
    use axum::routing::post;
    let app = Router::new()
        .route(
            "/health",
            get(|| async {
                axum::Json(json!({"status": "ok", "backend": "cuda", "version": "1",
                    "gpu": {"name": "NVIDIA T600", "memory_total_mib": 4096},
                    "device": {"state": "ready", "models": [{"slot": "de", "state": "ready", "vram_mib": 700}]},
                    "queue": {"length": 0, "capacity": 32, "sessions": 0, "max_sessions": 8},
                    "models": ["cuda-de", "cuda-auto"]}))
            }),
        )
        .route(
            "/v1/models",
            get(|| async { axum::Json(json!({"data": [{"id": "cuda-de"}, {"id": "cuda-auto"}]})) }),
        )
        .route(
            "/v1/admin/models",
            get(|| async { axum::Json(json!({"backend": "cuda", "slots": [], "library": []})) }),
        )
        .route(
            "/v1/admin/library",
            post(|headers: HeaderMap, body: axum::body::Bytes| async move {
                let multipart = headers["content-type"].to_str().unwrap().starts_with("multipart/form-data");
                axum::Json(json!({"id": "0123456789ab", "slots": ["de"], "received": body.len(),
                    "multipart": multipart, "manifest": {"name": "FC", "family": "nemo_transducer",
                    "archive_sha256": "ab", "license": "CC-BY-4.0", "source": "hf"}}))
            }),
        )
        .route(
            "/v1/admin/library/download",
            post(|axum::Json(v): axum::Json<serde_json::Value>| async move {
                axum::Json(json!({"id": "0123456789ac", "slots": [], "manifest": {"name": v["name"],
                    "archive_sha256": v["sha256"], "license": v["license"], "source": v["source"]}}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://127.0.0.1:{port}/v1")
}

/// A CUDA connection (ADR 0049): loopback only, health in the test, and
/// the model library through `/api/v1/speech-services/{id}`.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn cuda_connection_and_model_library(db: PgPool) {
    use axum::body::Body;
    use axum::http::{Request, header};
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (status, body) = admin
        .post(
            "/api/v1/settings/transcription/providers",
            json!({"name": "GPU", "kind": "cuda", "url": "http://10.0.0.5:8096/v1"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let url = fake_cuda().await;
    let (status, p) = admin
        .post(
            "/api/v1/settings/transcription/providers",
            json!({"name": "GPU", "kind": "cuda", "url": url}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{p}");
    let id = p["id"].as_str().unwrap().to_owned();
    let (status, t) = admin
        .post(
            &format!("/api/v1/settings/transcription/providers/{id}/test"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{t}");
    assert_eq!(t["coral"]["gpu"]["name"], "NVIDIA T600");
    assert!(
        t["speech_models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "cuda-auto")
    );

    let (status, m) = admin
        .get(&format!("/api/v1/speech-services/{id}/models"))
        .await;
    assert_eq!(status, StatusCode::OK, "{m}");
    assert_eq!(m["backend"], "cuda");

    // The upload body reaches the service unchanged.
    let boundary = "b";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"archive\"; filename=\"m.tar\"\r\n\r\n{}\r\n--{boundary}--\r\n",
        "x".repeat(100_000)
    );
    let length = body.len();
    let req = Request::post(format!("/api/v1/speech-services/{id}/library"))
        .header(header::COOKIE, &admin.cookie)
        .header("x-requested-with", "TalkOps")
        .header("x-csrf-token", &admin.csrf)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let res = raw(&router, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let added = body_json(res).await;
    assert_eq!(added["received"], length);
    assert_eq!(added["multipart"], true);

    let (status, d) = admin
        .post(
            &format!("/api/v1/speech-services/{id}/library/download"),
            json!({"url": "https://example.org/m.tar.bz2", "sha256": "ab", "name": "FC", "family": "nemo_transducer",
                   "languages": "de", "license": "CC-BY-4.0", "source": "hf", "slot": "xx"}),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "unknown slot: {d}"
    );
    let (status, d) = admin
        .post(
            &format!("/api/v1/speech-services/{id}/library/download"),
            json!({"url": "https://example.org/m.tar.bz2", "sha256": "ab", "name": "FC", "family": "nemo_transducer",
                   "languages": "de", "license": "CC-BY-4.0", "source": "hf"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{d}");
    assert_eq!(d["id"], "0123456789ac");
    let details: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT details FROM audit_log WHERE action IN ('add_speech_model', 'download_speech_model') ORDER BY id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(details.len(), 2);
    assert_eq!(details[0]["sha256"], "ab");
    assert_eq!(details[0]["license"], "CC-BY-4.0");
    assert_eq!(details[1]["url"], "https://example.org/m.tar.bz2");
}
