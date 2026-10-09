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

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn voicemail_control_settings(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    admin
        .post(
            "/api/v1/users",
            json!({"username": "ben", "display_name": "Ben", "password": "ben-password-1", "role": "operator"}),
        )
        .await;
    let ben = login(&router, "ben", "ben-password-1").await.unwrap();
    let (status, _) = ben.get("/api/v1/settings/voicemail").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, v) = admin.get("/api/v1/settings/voicemail").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["config"]["keys"]["delete"], "7");
    assert_eq!(v["config"]["announce"]["date"], true);
    let menu = v["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == "vm_main_menu")
        .unwrap();
    assert_eq!(menu["placeholders"], json!(["listen", "greeting", "exit"]));
    assert!(
        menu["defaults"]["de"]
            .as_str()
            .unwrap()
            .contains("{listen}")
    );

    // The same key twice in one menu, unknown placeholders: refused.
    let mut c = v["config"].clone();
    c["keys"]["save"] = json!("7");
    let (status, _) = admin.put("/api/v1/settings/voicemail", c).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let mut c = v["config"].clone();
    c["texts"] = json!({"de": {"vm_main_menu": "Drück {foo}"}});
    let (status, _) = admin.put("/api/v1/settings/voicemail", c).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let mut c = v["config"].clone();
    c["keys"]["delete"] = json!("3");
    c["voices"] = json!({"de": 2, "en": 1});
    c["announce"]["date"] = json!(false);
    c["texts"] = json!({"de": {"vm_goodbye": " Tschüss! ", "vm_and": "und"}});
    let (status, v) = admin.put("/api/v1/settings/voicemail", c).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    // Defaults are not stored: voice 1, texts equal to the default.
    assert_eq!(v["config"]["voices"], json!({"de": 2}));
    assert_eq!(
        v["config"]["texts"],
        json!({"de": {"vm_goodbye": "Tschüss!"}})
    );
    assert_eq!(v["config"]["keys"]["delete"], "3");
    let (_, again) = admin.get("/api/v1/settings/voicemail").await;
    assert_eq!(again["config"], v["config"]);
    // The worker renders the changed prompts.
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'tts_prompts' AND status = 'queued'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(queued, 1);
}
