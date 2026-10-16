//! Mail design: branding, logo, templates and preview (ADR 0034).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::mail_render;
use talkops_core::mail_templates;
use talkops_core::tenant::TenantId;

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut data = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[8, 6, 0, 0, 0]);
    data
}

async fn upload_logo(admin: &Client, data: &[u8]) -> StatusCode {
    let boundary = "XBOUNDARYX";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"logo.png\"\r\nContent-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let res = raw(
        &admin.router,
        Request::post("/api/v1/settings/mail/logo")
            .header(header::COOKIE, &admin.cookie)
            .header("x-requested-with", "TalkOps")
            .header("x-csrf-token", &admin.csrf)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap(),
    )
    .await;
    res.status()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn mail_design_roundtrip(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    admin
        .post(
            "/api/v1/users",
            json!({"username": "ben", "display_name": "Ben", "password": "ben-password-1", "role": "operator"}),
        )
        .await;
    let ben = login(&router, "ben", "ben-password-1").await.unwrap();
    let (status, _) = ben.get("/api/v1/settings/mail").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Defaults.
    let (status, v) = admin.get("/api/v1/settings/mail").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["branding"]["accent_color"], "#0d9488");
    assert_eq!(v["branding"]["has_logo"], false);
    assert_eq!(v["instance"]["version"], mail_render::VERSION);
    assert!(
        v["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k["key"] == "voicemail")
    );
    let templates = v["templates"].as_array().unwrap();
    assert_eq!(templates.len(), mail_render::KINDS.len() * 2);
    assert!(templates.iter().all(|t| t["custom"] == false));

    // Branding: validation and save.
    let (status, _) = admin
        .put(
            "/api/v1/settings/mail/branding",
            json!({"accent_color": "teal"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .put(
            "/api/v1/settings/mail/branding",
            json!({"accent_color": "#123456", "footer_text": "{{ caller.name }}"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, b) = admin
        .put(
            "/api/v1/settings/mail/branding",
            json!({"instance_name": "Home", "sender_name": "Phone system", "accent_color": "#ABC",
                   "header_text": "Hello", "footer_text": "Footer of **{{ instance.name }}**"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    assert_eq!(b["accent_color"], "#aabbcc");
    assert_eq!(b["instance_name"], "Home");
    let name: String =
        sqlx::query_scalar("SELECT instance_name FROM tenant_settings WHERE tenant_id = $1")
            .bind(TenantId::DEFAULT)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(name, "Home");

    // Logo: type and size checks.
    assert_eq!(
        upload_logo(&admin, b"<svg/>").await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut big = png(10, 10);
    big.resize(mail_templates::MAX_LOGO_BYTES + 1, 0);
    assert_ne!(upload_logo(&admin, &big).await, StatusCode::OK);
    assert_eq!(upload_logo(&admin, &png(400, 100)).await, StatusCode::OK);
    let (_, v) = admin.get("/api/v1/settings/mail").await;
    assert_eq!(v["branding"]["has_logo"], true);
    assert_eq!(v["branding"]["logo_width"], 400);
    let res = raw(
        &router,
        Request::get("/api/v1/settings/mail/logo")
            .header(header::COOKIE, &admin.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/png");

    // Templates: unknown placeholders are rejected.
    let (status, e) = admin
        .put(
            "/api/v1/settings/mail/templates/voicemail/de",
            json!({"subject": "x", "body": "{{ smtp.password }}"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(e["message"].as_str().unwrap().contains("smtp.password"));
    let (status, _) = admin
        .put(
            "/api/v1/settings/mail/templates/nope/de",
            json!({"subject": "x", "body": "y"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = admin
        .put(
            "/api/v1/settings/mail/templates/voicemail/fr",
            json!({"subject": "x", "body": "y"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, list) = admin
        .put(
            "/api/v1/settings/mail/templates/voicemail/de",
            json!({"subject": "Anruf von {{ caller.name }}", "body": "Hallo **{{ recipient.name }}**"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let vm = |list: &serde_json::Value| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|t| t["kind"] == "voicemail" && t["language"] == "de")
            .cloned()
            .unwrap()
    };
    assert_eq!(vm(&list)["custom"], true);
    let stored = mail_templates::template(&db, TenantId::DEFAULT, &mail_render::VOICEMAIL, "de")
        .await
        .unwrap();
    assert_eq!(stored.subject, "Anruf von {{ caller.name }}");

    // Preview: sample values, escaping, logo as data URL, branding draft.
    let (status, p) = admin
        .post(
            "/api/v1/settings/mail/preview",
            json!({"kind": "voicemail", "language": "en", "subject": "From {{ caller.name }}",
                   "body": "<b>x</b> {{ caller.name }}",
                   "branding": {"instance_name": "Draft", "accent_color": "#ff0000"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["subject"], "[Draft] From John Doe");
    let html = p["html"].as_str().unwrap();
    assert!(html.contains("&lt;b&gt;x&lt;/b&gt; John Doe"));
    assert!(html.contains("src=\"data:image/png;base64,"));
    assert!(html.contains("solid #ff0000"));
    assert!(html.contains("Powered by TalkOps"));
    assert!(p["text"].as_str().unwrap().contains("Powered by TalkOps"));
    let (status, _) = admin
        .post(
            "/api/v1/settings/mail/preview",
            json!({"kind": "voicemail", "language": "en", "subject": "x", "body": "{{ range(3) }}"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Test send needs SMTP.
    let (status, _) = admin
        .post(
            "/api/v1/settings/mail/test",
            json!({"kind": "test", "language": "de", "subject": "x", "body": "y", "to": "a@example.com"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Reset to default; delete the logo.
    let (status, list) = admin
        .call(
            "DELETE",
            "/api/v1/settings/mail/templates/voicemail/de",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(vm(&list)["custom"], false);
    assert_eq!(vm(&list)["subject"], mail_render::VOICEMAIL.subject[0]);
    let (status, b) = admin
        .call("DELETE", "/api/v1/settings/mail/logo", None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(b["has_logo"], false);

    let (_, audit) = admin.get("/api/v1/audit").await;
    let audit = audit.to_string();
    assert!(audit.contains("mail_branding"));
    assert!(audit.contains("mail_template"));
    assert!(audit.contains("mail_logo"));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn design_uses_public_url_and_stored_texts(db: PgPool) {
    sqlx::query("UPDATE identity_settings SET public_url = 'https://pbx.example.com'")
        .execute(&db)
        .await
        .unwrap();
    let (design, logo) = mail_templates::design(&db, TenantId::DEFAULT)
        .await
        .unwrap();
    assert!(logo.is_none());
    assert_eq!(design.instance.url, "https://pbx.example.com");
    assert_eq!(design.accent_color, "#0d9488");
    let text = mail_templates::template(&db, TenantId::DEFAULT, &mail_render::TEST, "en")
        .await
        .unwrap();
    assert_eq!(text.subject, mail_render::TEST.subject[1]);
}
