//! Voicemail and SMTP API.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use sqlx::PgPool;
use talkops_core::tenant::TenantId;
use talkops_core::voicemail::{self, NewMessage};
use uuid::Uuid;

use common::*;

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn voicemail_api(db: PgPool) {
    let state = state(db.clone());
    let media = state.media.clone();
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;
    let (_, anna) = admin
        .post(
            "/api/v1/users",
            json!({"username": "anna", "display_name": "Anna", "password": "anna-password-1",
                   "role": "user", "email": "anna@example.com"}),
        )
        .await;
    admin
        .post(
            "/api/v1/users",
            json!({"username": "ben", "display_name": "Ben", "password": "ben-password-1", "role": "user"}),
        )
        .await;
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "30", "display_name": "Anna", "user_id": anna["id"]}),
        )
        .await;
    let ext_id = ext["id"].as_str().unwrap();
    let anna = login(&router, "anna", "anna-password-1").await.unwrap();
    let ben = login(&router, "ben", "ben-password-1").await.unwrap();

    // Box settings: owner only (and admins).
    let (status, b) = anna
        .get(&format!("/api/v1/extensions/{ext_id}/voicemail"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(b["enabled"], false);
    assert_eq!(b["has_pin"], false);
    let (status, _) = ben
        .get(&format!("/api/v1/extensions/{ext_id}/voicemail"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, b) = anna
        .put(
            &format!("/api/v1/extensions/{ext_id}/voicemail"),
            json!({"enabled": true, "pin": "2468", "email_notify": true, "greeting": "tts",
                   "greeting_text": "Hallo, hier ist Anna."}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    assert_eq!(b["has_pin"], true);
    assert_eq!(b["greeting_status"], "pending");
    assert!(b.get("pin_hash").is_none());
    let (status, _) = anna
        .put(
            &format!("/api/v1/extensions/{ext_id}/voicemail"),
            json!({"enabled": true, "pin": "12"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // The user decides how long the phone rings before voicemail answers.
    assert_eq!(b["ring_timeout_secs"], 30);
    let (status, b) = anna
        .put(
            &format!("/api/v1/extensions/{ext_id}/voicemail"),
            json!({"enabled": true, "ring_timeout_secs": 15}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    assert_eq!(b["ring_timeout_secs"], 15);
    let (_, e) = admin.get(&format!("/api/v1/extensions/{ext_id}")).await;
    assert_eq!(e["ring_timeout_secs"], 15);
    let (status, _) = anna
        .put(
            &format!("/api/v1/extensions/{ext_id}/voicemail"),
            json!({"enabled": true, "ring_timeout_secs": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // A message as the voicemail flow would store it.
    let ext_uuid: Uuid = ext_id.parse().unwrap();
    let msg = voicemail::create_message(
        &db,
        TenantId::DEFAULT,
        &NewMessage {
            id: Uuid::new_v4(),
            extension_id: ext_uuid,
            caller_number: "+4930123456".into(),
            caller_name: String::new(),
            duration_secs: 5,
            call_uuid: None,
        },
    )
    .await
    .unwrap();
    let file = media.voicemail.join(&msg.file);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"RIFF-fake-wav").unwrap();

    let (status, list) = anna
        .get(&format!("/api/v1/voicemail/messages?extension_id={ext_id}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["status"], "new");
    let (_, b) = anna
        .get(&format!("/api/v1/extensions/{ext_id}/voicemail"))
        .await;
    assert_eq!(b["new_messages"], 1);
    let (status, _) = ben
        .get(&format!("/api/v1/voicemail/messages?extension_id={ext_id}"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let res = raw(
        &router,
        Request::get(format!("/api/v1/voicemail/messages/{}/audio", msg.id))
            .header(header::COOKIE, &anna.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "audio/wav");
    assert_eq!(body_string(res).await, "RIFF-fake-wav");

    let (status, m) = anna
        .put(
            &format!("/api/v1/voicemail/messages/{}", msg.id),
            json!({"heard": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(m["status"], "saved");

    // An admin may listen too, which is audited.
    let (status, _) = admin
        .get(&format!("/api/v1/voicemail/messages/{}/audio", msg.id))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, log) = admin.get("/api/v1/audit").await;
    assert!(
        log.as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "listen")
    );

    let (status, _) = anna
        .call(
            "DELETE",
            &format!("/api/v1/voicemail/messages/{}", msg.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!file.exists());

    // SMTP: admin only, password never returned.
    let (status, _) = anna.get("/api/v1/settings/smtp").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = admin
        .post("/api/v1/settings/smtp/test", json!({"to": "a@example.com"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, s) = admin
        .put(
            "/api/v1/settings/smtp",
            json!({"host": "smtp.example.com", "port": 465, "security": "tls", "username": "pbx",
                   "password": "secret", "from": "TalkOps <pbx@example.com>"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["has_password"], true);
    assert!(s.get("password").is_none());
}
