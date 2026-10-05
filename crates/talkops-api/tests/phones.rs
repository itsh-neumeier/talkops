//! Phone management API and the provisioning endpoints phones fetch.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::*;

const UA: &str = "Yealink SIP-T54W 96.86.0.100 80:5e:c0:aa:bb:cc";

async fn prov_get(
    router: &axum::Router,
    path: &str,
    creds: Option<(&str, &str)>,
) -> (StatusCode, String) {
    let mut req = Request::get(path).header(header::USER_AGENT, UA);
    if let Some((u, p)) = creds {
        req = req.header(
            header::AUTHORIZATION,
            format!("Basic {}", STANDARD.encode(format!("{u}:{p}"))),
        );
    }
    let res = raw(router, req.body(Body::empty()).unwrap()).await;
    let status = res.status();
    (status, body_string(res).await)
}

fn multipart(model: &str, filename: &str, data: &[u8]) -> (String, Vec<u8>) {
    let boundary = "talkopsboundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{model}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn phones_and_provisioning(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;

    let (status, models) = admin.get("/api/v1/phone-models").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        models
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "t54w" && m["family"] == "desk")
    );

    let (_, ext20) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "20", "display_name": "Office"}),
        )
        .await;
    let (_, _ext21) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "21", "display_name": "Lab"}),
        )
        .await;
    let ext_id = ext20["id"].as_str().unwrap();

    // Validation: unknown model, key outside the model, BLF without number.
    for (model, keys) in [
        ("t99", json!([])),
        ("t54w", json!([{"key": 99, "type": "blf", "value": "21"}])),
        ("t54w", json!([{"key": 3, "type": "blf", "value": "${x}"}])),
    ] {
        let (status, _) = admin
            .post(
                "/api/v1/phones",
                json!({"mac": "805ec0aabbcc", "model": model, "name": "Desk", "line_keys": keys}),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{model} {keys}");
    }
    let (status, phone) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "80:5E:C0:AA:BB:CC", "model": "t54w", "name": "Desk",
                   "line_keys": [{"key": 3, "type": "blf", "value": "21", "label": "Lab"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    assert_eq!(phone["mac"], "805ec0aabbcc");
    let phone_id = phone["id"].as_str().unwrap();

    // Devices are placed on the phone; slots beyond the model are refused.
    let (status, _) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "Desk", "kind": "desk", "phone_id": phone_id, "account_index": 17}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, cred) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "Desk", "kind": "desk", "phone_id": phone_id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, detail) = admin.get(&format!("/api/v1/phones/{phone_id}")).await;
    assert_eq!(detail["accounts"][0]["account_index"], 1);
    assert_eq!(detail["accounts"][0]["extension_number"], "20");

    // Provisioning credentials.
    let (status, info) = admin.get("/api/v1/provisioning").await;
    assert_eq!(status, StatusCode::OK);
    let user = info["username"].as_str().unwrap().to_owned();
    let pass = info["password"].as_str().unwrap().to_owned();
    assert!(
        info["url_with_credentials"]
            .as_str()
            .unwrap()
            .contains(&format!("{user}:{pass}@"))
    );

    // Phones ask without credentials first, then with them.
    let (status, _) = prov_get(&router, "/provisioning/y000000000068.cfg", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = prov_get(
        &router,
        "/provisioning/y000000000068.cfg",
        Some((&user, "wrong")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, common) = prov_get(
        &router,
        "/provisioning/y000000000068.cfg",
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(common.starts_with("#!version:1.0.0.1\n"));
    assert!(common.contains("auto_provision.server.url = http://localhost/provisioning"));
    assert!(common.contains(&info["phone_admin_password"].as_str().unwrap().to_owned()));

    let (status, cfg) = prov_get(
        &router,
        "/provisioning/805ec0aabbcc.cfg",
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(cfg.contains("account.1.user_name = 20-1"), "{cfg}");
    assert!(cfg.contains(&format!(
        "account.1.password = {}",
        cred["sip_password"].as_str().unwrap()
    )));
    assert!(cfg.contains("linekey.3.type = 16"), "{cfg}");
    assert!(cfg.contains("linekey.3.pickup_value = **21"), "{cfg}");
    assert!(!cfg.contains("firmware.url"));
    let (_, detail) = admin.get(&format!("/api/v1/phones/{phone_id}")).await;
    assert_eq!(detail["last_firmware"], "96.86.0.100");
    assert!(detail["last_seen_at"].is_string());

    let (status, _) = prov_get(
        &router,
        "/provisioning/001565000000.cfg",
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = prov_get(&router, "/provisioning/../etc/passwd", Some((&user, &pass))).await;
    assert_ne!(status, StatusCode::OK);

    // Phonebooks.
    let (status, xml) = prov_get(
        &router,
        "/provisioning/phonebook/internal.xml",
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(xml.contains("<Name>Office</Name>") && xml.contains(">20</Telephone>"));
    let (status, contact) = admin
        .post(
            "/api/v1/contacts",
            json!({"name": "Plumber", "company": "Müller & Co", "phone_mobile": "+49 171 2345678"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{contact}");
    let (status, xml) = prov_get(
        &router,
        "/provisioning/phonebook/contacts.xml?search=plum",
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(xml.contains("Plumber (Müller &amp; Co)"), "{xml}");
    assert!(xml.contains(">01712345678</Telephone>"), "{xml}");

    // Action URL: DND pressed on the phone.
    let (status, _) = prov_get(
        &router,
        &format!("/provisioning/events?key={pass}&event=dnd_on&mac=805ec0aabbcc"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, ext) = admin.get(&format!("/api/v1/extensions/{ext_id}")).await;
    assert_eq!(ext["dnd"], true);
    let (status, _) = prov_get(
        &router,
        "/provisioning/events?key=nope&event=dnd_off&mac=805ec0aabbcc",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Firmware upload, activation and download.
    let (ct, body) = multipart("t54w", "T54W-96.86.0.120.rom", b"firmware-bytes");
    let res = raw(
        &router,
        Request::post("/api/v1/firmware")
            .header(header::COOKIE, &admin.cookie)
            .header("x-requested-with", "TalkOps")
            .header("x-csrf-token", &admin.csrf)
            .header(header::CONTENT_TYPE, ct)
            .body(Body::from(body))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let fw: Value = body_json(res).await;
    assert_eq!(fw["size_bytes"], 14);
    assert_eq!(fw["active"], false);
    let fw_id = fw["id"].as_str().unwrap();
    let (status, fw) = admin
        .put(
            &format!("/api/v1/firmware/{fw_id}"),
            json!({"active": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fw["active"], true);
    let (_, cfg) = prov_get(
        &router,
        "/provisioning/805ec0aabbcc.cfg",
        Some((&user, &pass)),
    )
    .await;
    let url = format!(
        "firmware.url = http://{user}:{pass}@localhost/provisioning/firmware/{fw_id}/T54W-96.86.0.120.rom"
    );
    assert!(cfg.contains(&url), "{cfg}");
    let (status, data) = prov_get(
        &router,
        &format!("/provisioning/firmware/{fw_id}/T54W-96.86.0.120.rom"),
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data, "firmware-bytes");
    let (status, _) = prov_get(
        &router,
        &format!("/provisioning/firmware/{fw_id}/other.rom"),
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/firmware/{fw_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Config preview for the admin.
    let (status, _) = admin
        .get(&format!("/api/v1/phones/{phone_id}/config"))
        .await;
    assert_eq!(status, StatusCode::OK);

    // Regenerated credentials invalidate the old ones.
    let (_, info2) = admin
        .post("/api/v1/provisioning/regenerate", json!({}))
        .await;
    assert_ne!(info2["password"], info["password"]);
    let (status, _) = prov_get(
        &router,
        "/provisioning/y000000000068.cfg",
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn provisioning_brute_force_is_limited(db: PgPool) {
    let router = router(db);
    let _admin = setup_admin(&router).await;
    let mut last = StatusCode::OK;
    for _ in 0..12 {
        (last, _) = prov_get(
            &router,
            "/provisioning/y000000000068.cfg",
            Some(("provision", "guess")),
        )
        .await;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn users_set_their_own_call_settings(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, user) = admin
        .post(
            "/api/v1/users",
            json!({"username": "anna", "display_name": "Anna", "password": "anna-password-1", "role": "user"}),
        )
        .await;
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "30", "display_name": "Anna", "user_id": user["id"]}),
        )
        .await;
    let (_, other) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "31", "display_name": "Ben"}),
        )
        .await;
    let anna = login(&router, "anna", "anna-password-1").await.unwrap();
    let id = ext["id"].as_str().unwrap();
    let (status, e) = anna
        .put(
            &format!("/api/v1/extensions/{id}/call-settings"),
            json!({"dnd": true, "forward_all": "0171 2345678"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{e}");
    let (status, e) = anna
        .put(
            &format!("/api/v1/extensions/{id}/call-settings"),
            json!({"dnd": true, "forward_all": "01712345678"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{e}");
    assert_eq!(e["dnd"], true);
    assert_eq!(e["forward_all"], "01712345678");
    let (status, _) = anna
        .put(
            &format!("/api/v1/extensions/{id}/call-settings"),
            json!({"dnd": false, "forward_all": "30"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = anna
        .put(
            &format!(
                "/api/v1/extensions/{}/call-settings",
                other["id"].as_str().unwrap()
            ),
            json!({"dnd": true}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
