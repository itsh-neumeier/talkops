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

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn desk_model_catalog_and_extended_common_configs(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (status, models) = admin.get("/api/v1/phone-models").await;
    assert_eq!(status, StatusCode::OK);
    let desk: Vec<_> = models
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["family"] == "desk" && m["layout"].is_object())
        .collect();
    assert_eq!(desk.len(), 21);
    for (i, model) in desk.iter().enumerate() {
        assert!(model["layout"]["pages"].is_array(), "{model}");
        let keys = model["line_keys"].as_u64().unwrap();
        let body = json!({"mac": format!("805ec0dd{i:04x}"), "model": model["id"], "name": "Desk",
            "line_keys": [{"key": keys, "type": "speed_dial", "value": "**103", "label": "Workshop"}]});
        let (status, phone) = admin.post("/api/v1/phones", body.clone()).await;
        assert_eq!(status, StatusCode::OK, "{model}: {phone}");
        let path = format!("/api/v1/phones/{}", phone["id"].as_str().unwrap());
        let (_, detail) = admin.get(&path).await;
        assert_eq!(detail["line_keys"], body["line_keys"]);
        let response = raw(
            &router,
            Request::get(format!("{path}/config"))
                .header(header::COOKIE, &admin.cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let config = body_string(response).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            config.contains(&format!("linekey.{keys}.type = 13")),
            "{model}"
        );
        assert!(
            config.contains(&format!("linekey.{keys}.value = **103")),
            "{model}"
        );
        let mut invalid = body.clone();
        invalid["line_keys"][0]["key"] = json!(keys + 1);
        assert_eq!(
            admin.put(&path, invalid).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let mut invalid = body.clone();
        invalid["line_keys"][0]["key"] = json!(model["line_key_limit"].as_u64().unwrap() + 1);
        invalid["line_keys"][0]["type"] = json!("line");
        assert_eq!(
            admin.put(&path, invalid).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let credentials = Some((
        info["username"].as_str().unwrap(),
        info["password"].as_str().unwrap(),
    ));
    for name in [
        "y000000000036.cfg",
        "y0000000000183.cfg",
        "y0000000000196.cfg",
        "y0000000000197.cfg",
    ] {
        let path = format!("/provisioning/{name}");
        assert_eq!(
            prov_get(&router, &path, None).await.0,
            StatusCode::UNAUTHORIZED
        );
        let (status, body) = prov_get(&router, &path, credentials).await;
        assert_eq!(status, StatusCode::OK, "{name}");
        assert!(body.starts_with("#!version:1.0.0.1\n"));
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn catalog_corrections_preserve_unchanged_legacy_overrides(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    for (i, model, key) in [(1, "t46u", 40), (2, "vp59", 29), (3, "t41p", 7)] {
        let mut input = json!({"mac": format!("805ec0ee{i:04x}"), "model": model, "name": "Legacy", "line_keys": []});
        let (status, phone) = admin.post("/api/v1/phones", input.clone()).await;
        assert_eq!(status, StatusCode::OK);
        let id = uuid::Uuid::parse_str(phone["id"].as_str().unwrap()).unwrap();
        let old = json!([{"key": key, "type": if model == "t41p" { "line" } else { "speed_dial" }, "value": "103", "label": "Keep me", "account": 1}]);
        sqlx::query("UPDATE phones SET line_keys = $1 WHERE id = $2 AND tenant_id = $3")
            .bind(&old)
            .bind(id)
            .bind(talkops_core::tenant::TenantId::DEFAULT)
            .execute(&db)
            .await
            .unwrap();
        let path = format!("/api/v1/phones/{id}");
        input["line_keys"] = old.clone();
        input["name"] = json!("Renamed");
        let (status, saved) = admin.put(&path, input.clone()).await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        assert_eq!(saved["line_keys"], old);
        input["line_keys"][0]["value"] = json!("104");
        assert_eq!(
            admin.put(&path, input.clone()).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        input["line_keys"] = json!([]);
        assert_eq!(admin.put(&path, input).await.0, StatusCode::OK);
    }
}

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
            json!({"number": "200", "display_name": "Office"}),
        )
        .await;
    let (_, _ext21) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "210", "display_name": "Lab"}),
        )
        .await;
    let ext_id = ext20["id"].as_str().unwrap();

    // Validation: unknown model, key outside the model, BLF without number.
    for (model, keys) in [
        ("t99", json!([])),
        ("t54w", json!([{"key": 99, "type": "blf", "value": "210"}])),
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
                   "line_keys": [{"key": 3, "type": "blf", "value": "210", "label": "Lab"}]}),
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
    assert_eq!(detail["accounts"][0]["extension_number"], "200");
    // The list shows the extensions on each phone.
    let (_, list) = admin.get("/api/v1/phones").await;
    let listed = list
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == json!(phone_id))
        .unwrap();
    assert_eq!(listed["extensions"][0]["number"], "200", "{listed}");
    assert_eq!(listed["mac"], detail["mac"]);

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
    let sip_user = cred["sip_username"].as_str().unwrap();
    assert!(
        cfg.contains(&format!("account.1.user_name = {sip_user}")),
        "{cfg}"
    );
    assert!(cfg.contains(&format!(
        "account.1.password = {}",
        cred["sip_password"].as_str().unwrap()
    )));
    assert!(cfg.contains("linekey.3.type = 16"), "{cfg}");
    assert!(cfg.contains("linekey.3.pickup_value = *8*210"), "{cfg}");
    // Dial now: emergency numbers, fixed feature codes, internal 200/210.
    assert!(cfg.contains("dialplan.dialnow.rule.1 = 11[0,2]"), "{cfg}");
    assert!(cfg.contains("= *9[7,8]"), "{cfg}");
    assert!(cfg.contains("= 200\n") && cfg.contains("= 210\n"), "{cfg}");
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
    assert!(xml.contains("<Name>Office</Name>") && xml.contains(">**200</Telephone>"));
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
            json!({"number": "300", "display_name": "Anna", "user_id": user["id"]}),
        )
        .await;
    let (_, other) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "310", "display_name": "Ben"}),
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
            json!({"dnd": false, "forward_all": "300"}),
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

/// Multipart body with text fields and one file.
fn multipart_fields(fields: &[(&str, &str)], filename: &str, data: &[u8]) -> (String, Vec<u8>) {
    let boundary = "talkopsboundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// A WAV file with `samples` silent samples.
fn wav(rate: u32, channels: u16, samples: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::new(&mut out, spec).unwrap();
    for _ in 0..samples * u32::from(channels) {
        w.write_sample(0i16).unwrap();
    }
    w.finalize().unwrap();
    out.into_inner()
}

async fn upload_media(
    router: &axum::Router,
    admin: &Client,
    kind: &str,
    name: &str,
    data: &[u8],
) -> (StatusCode, Value) {
    let (ct, body) = multipart_fields(&[("kind", kind), ("name", name)], "upload", data);
    let res = raw(
        router,
        Request::post("/api/v1/phone-media")
            .header(header::COOKIE, &admin.cookie)
            .header("x-requested-with", "TalkOps")
            .header("x-csrf-token", &admin.csrf)
            .header(header::CONTENT_TYPE, ct)
            .body(Body::from(body))
            .unwrap(),
    )
    .await;
    let status = res.status();
    (status, body_json(res).await)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn ringtones_wallpapers_labels_and_phonebook_sections(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let user = info["username"].as_str().unwrap().to_owned();
    let pass = info["password"].as_str().unwrap().to_owned();

    // Uploads are checked: ringtones 8 kHz mono WAV, wallpapers JPEG/PNG.
    let (status, _) = upload_media(&router, &admin, "ringtone", "Stereo", &wav(8000, 2, 800)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = upload_media(&router, &admin, "ringtone", "16k", &wav(16000, 1, 800)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = upload_media(&router, &admin, "wallpaper", "Gif", b"GIF89a....").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let big = wav(8000, 1, 80_000); // 160 KB: too large for a T53W
    let (status, ring) = upload_media(&router, &admin, "ringtone", "Gong", &big).await;
    assert_eq!(status, StatusCode::OK, "{ring}");
    let ring_id = ring["id"].as_str().unwrap();
    let ring_file = ring["filename"].as_str().unwrap().to_owned();
    assert!(ring_file.starts_with("talkops-") && ring_file.ends_with(".wav"));
    let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4];
    let (status, wall) = upload_media(&router, &admin, "wallpaper", "Foto", &jpeg).await;
    assert_eq!(status, StatusCode::OK, "{wall}");
    let wall_id = wall["id"].as_str().unwrap();
    let wall_file = wall["filename"].as_str().unwrap().to_owned();
    assert!(wall_file.ends_with(".jpg"));
    let (_, list) = admin.get("/api/v1/phone-media").await;
    assert_eq!(list.as_array().unwrap().len(), 2);

    // Sections; a contact in a section is not in the global phone book.
    let (status, family) = admin
        .post("/api/v1/phonebook-sections", json!({"name": "Familie"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{family}");
    let family_id = family["id"].as_str().unwrap();
    admin
        .post(
            "/api/v1/contacts",
            json!({"name": "Oma", "phone_work": "0301234", "section_id": family_id}),
        )
        .await;
    admin
        .post(
            "/api/v1/contacts",
            json!({"name": "Pizza", "phone_work": "0305678"}),
        )
        .await;

    // Models without custom ringtones/wallpapers, or a too large ringtone.
    for (model, body) in [
        ("w70b", json!({"ringtone_id": ring_id})),
        ("t42u", json!({"wallpaper_id": wall_id})),
        ("t53w", json!({"ringtone_id": ring_id})),
    ] {
        let mut phone = json!({"mac": "805ec0000002", "model": model, "name": "X"});
        phone
            .as_object_mut()
            .unwrap()
            .extend(body.as_object().unwrap().clone());
        let (status, err) = admin.post("/api/v1/phones", phone).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{model}: {err}");
    }
    // AX Wi-Fi handsets: 16 line keys and custom media, but no speed dials.
    let speed_dial = json!([{"key": 2, "type": "speed_dial", "value": "0301234"}]);
    let (status, err) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "249ad8000001", "model": "ax86r", "name": "WLAN",
                   "line_keys": speed_dial}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
    let (status, ax) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "249ad8000001", "model": "ax86r", "name": "WLAN",
                   "ringtone_id": ring_id, "wallpaper_id": wall_id,
                   "line_keys": [{"key": 16, "type": "blf", "value": "210"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ax}");
    let ax_path = format!("/api/v1/phones/{}", ax["id"].as_str().unwrap());
    admin.call("DELETE", &ax_path, None).await;
    let (status, phone) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "805ec0000001", "model": "t54w", "name": "Küche",
                   "ringtone_id": ring_id, "wallpaper_id": wall_id,
                   "phonebook_sections": [family_id]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let phone_id = phone["id"].as_str().unwrap();

    // A Wi-Fi handset account with its own label and display name.
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "400", "display_name": "Küche"}),
        )
        .await;
    let (status, dev) = admin
        .post(
            &format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap()),
            json!({"name": "Handset", "kind": "wifi", "phone_id": phone_id,
                   "phone_label": "Küche 400"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{dev}");
    let (_, detail) = admin.get(&format!("/api/v1/phones/{phone_id}")).await;
    assert_eq!(detail["accounts"][0]["phone_label"], "Küche 400");
    let device_id = detail["accounts"][0]["device_id"].as_str().unwrap();
    let (status, _) = admin
        .put(
            &format!("/api/v1/phones/{phone_id}/accounts/{device_id}"),
            json!({"phone_label": "x".repeat(33)}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .put(
            &format!("/api/v1/phones/{phone_id}/accounts/{device_id}"),
            json!({"phone_label": " Küche 400 ", "phone_display_name": "Familie M."}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, cfg) = prov_get(
        &router,
        "/provisioning/805ec0000001.cfg",
        Some((&user, &pass)),
    )
    .await;
    let base = format!("http://{user}:{pass}@localhost/provisioning");
    for line in [
        "account.1.label = Küche 400".to_owned(),
        "account.1.display_name = Familie M.".to_owned(),
        format!("ringtone.url = {base}/media/{ring_id}/{ring_file}"),
        format!("phone_setting.ring_type = {ring_file}"),
        format!("wallpaper_upload.url = {base}/media/{wall_id}/{wall_file}"),
        format!("phone_setting.backgrounds = {wall_file}"),
        "remote_phonebook.data.3.name = Familie".to_owned(),
        format!("remote_phonebook.data.3.url = {base}/phonebook/section/{family_id}.xml"),
    ] {
        assert!(cfg.contains(&line), "missing `{line}` in\n{cfg}");
    }

    // The phone downloads the files and the section phone book.
    let res = raw(
        &router,
        Request::get(format!("/provisioning/media/{wall_id}/{wall_file}"))
            .header(
                header::AUTHORIZATION,
                format!("Basic {}", STANDARD.encode(format!("{user}:{pass}"))),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/jpeg");
    let data = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&data[..], &jpeg[..]);
    let (status, _) = prov_get(
        &router,
        &format!("/provisioning/media/{wall_id}/other.jpg"),
        Some((&user, &pass)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, xml) = prov_get(
        &router,
        &format!("/provisioning/phonebook/section/{family_id}.xml"),
        Some((&user, &pass)),
    )
    .await;
    assert!(
        xml.contains("<Title>Familie</Title>") && xml.contains("<Name>Oma</Name>"),
        "{xml}"
    );
    assert!(!xml.contains("Pizza"));
    let (_, xml) = prov_get(
        &router,
        "/provisioning/phonebook/contacts.xml",
        Some((&user, &pass)),
    )
    .await;
    assert!(xml.contains("Pizza") && !xml.contains("Oma"), "{xml}");

    // Deleting media and the section clears them from the phone.
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/phone-media/{ring_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = admin
        .call(
            "DELETE",
            &format!("/api/v1/phonebook-sections/{family_id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, cfg) = prov_get(
        &router,
        "/provisioning/805ec0000001.cfg",
        Some((&user, &pass)),
    )
    .await;
    assert!(!cfg.contains("ringtone.url"));
    assert!(cfg.contains("remote_phonebook.data.3.url = \n"), "{cfg}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn internal_phonebook_and_contacts_preserve_explicit_dialing(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    for (number, enabled) in [("101", true), ("102", false)] {
        let (status, ext) = admin
            .post(
                "/api/v1/extensions",
                json!({
                    "number": number, "display_name": format!("Office {number}"), "enabled": enabled
                }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{ext}");
    }
    let (_, mut settings) = admin.get("/api/v1/settings").await;
    settings["area_code"] = json!("9543");
    let (status, _) = admin.put("/api/v1/settings", settings).await;
    assert_eq!(status, StatusCode::OK);
    let (status, credentials) = admin.get("/api/v1/provisioning").await;
    assert_eq!(status, StatusCode::OK);
    let creds = Some((
        credentials["username"].as_str().unwrap(),
        credentials["password"].as_str().unwrap(),
    ));
    for path in [
        "/provisioning/phonebook/internal.xml",
        "/provisioning/phonebook/internal.xml?search=101",
        "/provisioning/phonebook/internal.xml?search=%2A%2A101",
    ] {
        let (status, xml) = prov_get(&router, path, creds).await;
        assert_eq!(status, StatusCode::OK, "{xml}");
        assert!(xml.contains(">**101</Telephone>"), "{xml}");
        assert!(
            !xml.contains("**102") && !xml.contains("+499543101"),
            "{xml}"
        );
    }
    let (status, contact) = admin
        .post(
            "/api/v1/contacts",
            json!({
                "name": "Reception", "phone_work": " ** 101 ", "phone_mobile": "+49 171 2345678"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{contact}");
    assert_eq!(contact["phone_work"], "**101");
    assert_eq!(contact["phone_mobile"], "+491712345678");
    let contact_path = format!("/api/v1/contacts/{}", contact["id"].as_str().unwrap());
    let (status, contact) = admin.put(&contact_path, json!({
        "name": "Reception", "phone_work": "**101", "phone_mobile": "+491712345678", "phone_other": "**12345678"
    })).await;
    assert_eq!(status, StatusCode::OK, "{contact}");
    assert_eq!(contact["phone_other"], "**12345678");
    let (status, report) = admin
        .post(
            "/api/v1/contacts/import",
            json!({
                "csv": "Name,Work,Section\nInternal office,**101,Office\nBroken,***101,\n"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["created"], 1, "{report}");
    assert_eq!(report["failed"], 1, "{report}");
    let (_, sections) = admin.get("/api/v1/phonebook-sections").await;
    let section_id = sections[0]["id"].as_str().unwrap();
    for path in [
        "/provisioning/phonebook/contacts.xml".to_owned(),
        format!("/provisioning/phonebook/section/{section_id}.xml"),
    ] {
        let (status, xml) = prov_get(&router, &path, creds).await;
        assert_eq!(status, StatusCode::OK, "{xml}");
        assert!(xml.contains(">**101</Telephone>"), "{xml}");
        assert!(!xml.contains("+499543101"), "{xml}");
        if path.ends_with("contacts.xml") {
            assert!(xml.contains(">01712345678</Telephone>"), "{xml}");
        }
    }
    for number in [
        "**",
        "**1",
        "**0101",
        "***101",
        "**+101",
        "**123456789",
        "*8*101",
        "*78",
    ] {
        let (status, _) = admin
            .post(
                "/api/v1/contacts",
                json!({"name": "Invalid", "phone_work": number}),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{number}");
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn phonebook_csv_import(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let sample = include_str!("../../../web/static/telefonbuch-beispiel.csv");

    // Dry run: report only, nothing stored.
    let (status, report) = admin
        .post(
            "/api/v1/contacts/import",
            json!({"csv": sample, "dry_run": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["created"], 6, "{report}");
    assert_eq!(report["failed"], 0, "{report}");
    assert_eq!(report["sections_created"], json!(["Familie", "Büro"]));
    let (_, contacts) = admin.get("/api/v1/contacts").await;
    assert_eq!(contacts.as_array().unwrap().len(), 0);
    let (_, sections) = admin.get("/api/v1/phonebook-sections").await;
    assert_eq!(sections.as_array().unwrap().len(), 0);

    // Import for real; numbers are cleaned, sections created.
    let (status, report) = admin
        .post("/api/v1/contacts/import", json!({"csv": sample}))
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["created"], 6);
    let (_, contacts) = admin.get("/api/v1/contacts").await;
    let contacts = contacts.as_array().unwrap();
    assert_eq!(contacts.len(), 6);
    let grandma = contacts
        .iter()
        .find(|c| c["name"] == "Oma und Opa")
        .unwrap();
    assert_eq!(grandma["phone_other"], "+498999998103");
    assert!(grandma["section_id"].is_string());
    let tax = contacts
        .iter()
        .find(|c| c["name"] == "Steuerbüro Beispiel")
        .unwrap();
    assert_eq!(tax["company"], "Schmidt; Partner");

    // Again: identical rows are unchanged; changed rows only with update_existing.
    let changed = "Name,Mobile\nPizzeria Bella,0171 3920009\n,0171 1\nNiemand,\n";
    let (_, report) = admin
        .post("/api/v1/contacts/import", json!({"csv": sample}))
        .await;
    assert_eq!(
        (report["created"].as_u64(), report["unchanged"].as_u64()),
        (Some(0), Some(6))
    );
    let (_, report) = admin
        .post("/api/v1/contacts/import", json!({"csv": changed}))
        .await;
    assert_eq!(report["skipped"], 1, "{report}");
    assert_eq!(report["failed"], 2, "{report}");
    assert_eq!(report["errors"][0]["line"], 3);
    let (_, report) = admin
        .post(
            "/api/v1/contacts/import",
            json!({"csv": changed, "update_existing": true}),
        )
        .await;
    assert_eq!(report["updated"], 1, "{report}");
    let (_, contacts) = admin.get("/api/v1/contacts").await;
    let pizza = contacts
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Pizzeria Bella")
        .unwrap()
        .clone();
    assert_eq!(pizza["phone_mobile"], "01713920009");
    assert_eq!(pizza["phone_work"], "", "update replaces all numbers");

    // A file without usable columns is rejected as a whole.
    let (status, _) = admin
        .post("/api/v1/contacts/import", json!({"csv": "Foo;Bar\n1;2\n"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn comfort_settings(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;

    // Catalog with the AX handset settings; nothing set yet.
    let (status, view) = admin.get("/api/v1/phone-settings").await;
    assert_eq!(status, StatusCode::OK, "{view}");
    let key_tone = view["catalog"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "key_tone")
        .cloned()
        .unwrap();
    assert_eq!(key_tone["param"], "features.send_key_tone");
    assert_eq!(key_tone["families"], json!(["wifi"]));
    assert_eq!(view["values"], json!({}));

    // Unknown keys and values outside the catalog are refused.
    for values in [
        json!({"nope": "1"}),
        json!({"backlight_time": "45"}),
        json!({"key_tone": 0}),
    ] {
        let (status, err) = admin
            .put("/api/v1/phone-settings", json!({"values": values}))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{values}: {err}");
    }
    let (status, view) = admin
        .put(
            "/api/v1/phone-settings",
            json!({"values": {"key_tone": "0", "backlight_time": "60"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["values"]["key_tone"], "0");

    // A handset overrides one value; a desk phone does not take them.
    let (status, err) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "249ad8000099", "model": "ax83h", "name": "WLAN",
                   "settings": {"vibrate": "maybe"}}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
    let (status, ax) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "249ad8000099", "model": "ax83h", "name": "WLAN",
                   "settings": {"backlight_time": "15", "off_cradle_answer": "0"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ax}");
    assert_eq!(ax["settings"]["backlight_time"], "15");
    let (status, desk) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "805ec0000099", "model": "t54w", "name": "Desk"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{desk}");

    let (_, info) = admin.get("/api/v1/provisioning").await;
    let creds = (
        info["username"].as_str().unwrap(),
        info["password"].as_str().unwrap(),
    );
    // A group without a number must not break the dial-now rules.
    let (_, member) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "700", "display_name": "Member"}),
        )
        .await;
    let (status, group) = admin
        .post(
            "/api/v1/ring-groups",
            json!({"name": "No number", "members": [member["id"]]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{group}");
    let (status, cfg) = prov_get(&router, "/provisioning/249ad8000099.cfg", Some(creds)).await;
    assert_eq!(status, StatusCode::OK, "{cfg}");
    assert!(cfg.contains("features.send_key_tone = 0\n"), "{cfg}");
    assert!(cfg.contains("phone_setting.backlight_time = 15\n"), "{cfg}");
    assert!(
        cfg.contains("phone_setting.off_cradle_auto_answer.enable = 0\n"),
        "{cfg}"
    );
    assert!(!cfg.contains("features.charging_tone"), "unset: {cfg}");
    let (_, cfg) = prov_get(&router, "/provisioning/805ec0000099.cfg", Some(creds)).await;
    assert!(!cfg.contains("features.send_key_tone"), "{cfg}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn codec_profiles_set_the_phone_codecs(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "300", "display_name": "Lager"}),
        )
        .await;
    let ext = ext["id"].as_str().unwrap();
    let mut phones = Vec::new();
    for (mac, model) in [("249ad8000077", "ax86r"), ("805ec0000077", "t54w")] {
        let (status, phone) = admin
            .post(
                "/api/v1/phones",
                json!({"mac": mac, "model": model, "name": model}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{phone}");
        phones.push(phone["id"].as_str().unwrap().to_owned());
    }
    let mut devices = Vec::new();
    for (name, kind, phone) in [
        ("Handset", "wifi", &phones[0]),
        ("Desk", "desk", &phones[1]),
    ] {
        let (status, d) = admin
            .post(
                &format!("/api/v1/extensions/{ext}/devices"),
                json!({"name": name, "kind": kind, "phone_id": phone}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{d}");
        devices.push(d["device_id"].as_str().unwrap().to_owned());
    }
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let creds = (
        info["username"].as_str().unwrap(),
        info["password"].as_str().unwrap(),
    );

    // The Wi-Fi handset (automatic: HD) gets G.722, iLBC and G.711.
    let (status, cfg) = prov_get(&router, "/provisioning/249ad8000077.cfg", Some(creds)).await;
    assert_eq!(status, StatusCode::OK, "{cfg}");
    for line in [
        "account.1.codec.g722.enable = 1\n",
        "account.1.codec.g722.priority = 1\n",
        "account.1.codec.ilbc.enable = 1\n",
        "account.1.codec.ilbc_15_2kpbs.enable = 1\n",
        "account.1.codec.ilbc_13_33kpbs.enable = 1\n",
        "account.1.codec.pcma.enable = 1\n",
        "account.1.codec.pcmu.enable = 1\n",
        "account.1.codec.g729.enable = 0\n",
    ] {
        assert!(cfg.contains(line), "{line}: {cfg}");
    }
    // The desk phone (standard) keeps its own codec settings.
    let (_, cfg) = prov_get(&router, "/provisioning/805ec0000077.cfg", Some(creds)).await;
    assert!(!cfg.contains(".codec."), "{cfg}");

    // Robust or HD: the phone has the same codecs, TalkOps selects per call.
    // A desk phone set to a profile gets them too.
    let (status, d) = admin
        .put(
            &format!("/api/v1/devices/{}", devices[1]),
            json!({"name": "Desk", "kind": "desk", "phone_id": phones[1], "codec_profile": "robust"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{d}");
    let (_, cfg) = prov_get(&router, "/provisioning/805ec0000077.cfg", Some(creds)).await;
    assert!(cfg.contains("account.1.codec.ilbc.enable = 1\n"), "{cfg}");

    // Standard on the handset: the phone's codec settings stay unchanged.
    admin
        .put(
            &format!("/api/v1/devices/{}", devices[0]),
            json!({"name": "Handset", "kind": "wifi", "phone_id": phones[0], "codec_profile": "standard"}),
        )
        .await;
    let (_, cfg) = prov_get(&router, "/provisioning/249ad8000077.cfg", Some(creds)).await;
    assert!(!cfg.contains(".codec."), "{cfg}");
}

/// GET without credentials, as a Cisco Enterprise phone requests its directory.
async fn plain_get(router: &axum::Router, path: &str) -> (StatusCode, String) {
    let res = raw(
        router,
        Request::get(path)
            .header(header::HOST, "192.0.2.1:8080")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = res.status();
    (status, body_string(res).await)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn cisco_enterprise_directory_resync_and_ringtones(db: PgPool) {
    let state = state(db);
    let router = talkops_api::app(state.clone(), None);
    let admin = setup_admin(&router).await;
    let (status, ring) = upload_media(
        &router,
        &admin,
        "ringtone",
        "Bells & Co",
        &wav(8000, 1, 800),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ring}");
    let (_, other) = upload_media(&router, &admin, "ringtone", "Chime", &wav(8000, 1, 400)).await;
    let input = json!({"mac":"00451d6be327","name":"Cisco desk","model":"cisco-7861-enterprise",
        "ringtone_id": ring["id"],
        "provisioning":{"sip_host":"192.0.2.1","endpoint":"http://192.0.2.1:8080/provisioning",
            "vendor_settings":{"cisco_enterprise_tftp_ip":"127.0.0.2"}}});
    let (status, phone) = admin.post("/api/v1/phones", input).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let id = phone["id"].as_str().unwrap();
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"999","display_name":"Cisco & Co"}),
        )
        .await;
    let (status, device) = admin
        .post(
            &format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap()),
            json!({"name":"Cisco","kind":"desk","phone_id":id,"account_index":1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{device}");
    for i in 0..40 {
        admin
            .post(
                "/api/v1/contacts",
                json!({"name": format!("Contact {i:02}"), "phone_work": format!("0301{i:04}")}),
            )
            .await;
    }

    // Enterprise profiles have resync and TFTP delivery; files have no HTTP URL.
    let (_, models) = admin.get("/api/v1/phone-models").await;
    let model = models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "cisco-7861-enterprise")
        .unwrap();
    let caps = &model["provisioning_capabilities"];
    assert_eq!(
        (caps["resync"].as_bool(), caps["reboot"].as_bool()),
        (Some(true), Some(true))
    );
    assert_eq!(caps["delivery"], "tftp");
    assert_eq!(model["ringtone_max_kb"], 160);
    let (_, detail) = admin.get(&format!("/api/v1/phones/{id}")).await;
    assert_eq!(detail["delivery"], "tftp");
    assert!(detail["provisioning_base"].is_null());
    let files = detail["provisioning_files"].as_array().unwrap();
    assert_eq!(files[0]["name"], "SEP00451D6BE327.cnf.xml");
    assert!(files.iter().all(|f| f["url"] == ""));
    // Without a registration nothing is sent, but the request is accepted and audited.
    let (status, result) = admin
        .post(
            &format!("/api/v1/phones/{id}/resync"),
            json!({"reboot": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["notified"], 0);

    // The export carries the ring list, ringtones and the directory URL.
    let root = tempfile::tempdir().unwrap();
    let stats = talkops_api::cisco_tftp::refresh(&state, root.path())
        .await
        .unwrap();
    assert_eq!((stats.phones, stats.files), (1, 5));
    let dir = root.path().join("active/127.0.0.2");
    let ringlist = std::fs::read_to_string(dir.join("Ringlist-wb.xml")).unwrap();
    let selected = format!(
        "talkops-{}.rwb",
        &ring["id"].as_str().unwrap().replace('-', "")[..8]
    );
    let second = format!(
        "talkops-{}.rwb",
        &other["id"].as_str().unwrap().replace('-', "")[..8]
    );
    assert!(
        ringlist.find(&selected).unwrap() < ringlist.find(&second).unwrap(),
        "selected ringtone first: {ringlist}"
    );
    assert!(ringlist.contains("<DisplayName>Bells &amp; Co</DisplayName>"));
    assert_eq!(std::fs::read(dir.join(&selected)).unwrap().len(), 1600 * 2);
    let sep = std::fs::read_to_string(dir.join("SEP00451D6BE327.cnf.xml")).unwrap();
    let start = sep.find("<directoryURL>").unwrap() + "<directoryURL>".len();
    let url = &sep[start..start + sep[start..].find("</directoryURL>").unwrap()];
    let path = url.strip_prefix("http://192.0.2.1:8080").expect(url);
    assert!(path.starts_with(&format!("/provisioning/cisco/directory/{id}/")));
    assert!(sep.contains("<versionStamp>talkops-"));

    // The directory needs the token, not the provisioning credentials.
    let (status, page) = plain_get(&router, path).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("<CiscoIPPhoneDirectory>"));
    assert!(page.contains("<Name>Cisco &amp; Co</Name><Telephone>**999</Telephone>"));
    assert!(page.contains("<Prompt>1/2</Prompt>"));
    assert!(!page.contains(device["sip_password"].as_str().unwrap()));
    let (_, page2) = plain_get(&router, &format!("{path}?page=2")).await;
    assert!(page2.contains("<Name>Contact 39</Name>"));
    let (_, search) = plain_get(&router, &format!("{path}?search=Contact+07")).await;
    assert_eq!(search.matches("<DirectoryEntry>").count(), 1);
    let (_, form) = plain_get(&router, &format!("{path}?search=")).await;
    assert!(form.contains("<CiscoIPPhoneInput>"));
    let wrong = format!("/provisioning/cisco/directory/{id}/{}", "0".repeat(32));
    assert_eq!(plain_get(&router, &wrong).await.0, StatusCode::NOT_FOUND);
    // Regenerated provisioning credentials revoke the old directory URL.
    let (status, _) = admin
        .post("/api/v1/provisioning/regenerate", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(plain_get(&router, path).await.0, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn cisco_enterprise_resync_exports_the_saved_state_first(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let state = state(db).with_cisco_tftp(root.path().to_owned());
    let router = talkops_api::app(state.clone(), None);
    let admin = setup_admin(&router).await;
    let input = json!({"mac":"00451d6be327","name":"Cisco desk","model":"cisco-7861-enterprise",
        "provisioning":{"sip_host":"192.0.2.1","endpoint":"http://192.0.2.1:8080/provisioning",
            "vendor_settings":{"cisco_enterprise_tftp_ip":"127.0.0.2"}}});
    let (status, phone) = admin.post("/api/v1/phones", input).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let id = phone["id"].as_str().unwrap();
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"999","display_name":"Before"}),
        )
        .await;
    let ext_id = ext["id"].as_str().unwrap();
    let (status, device) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name":"Cisco","kind":"desk","phone_id":id,"account_index":1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{device}");
    let sep = root.path().join("active/127.0.0.2/SEP00451D6BE327.cnf.xml");
    let resync = format!("/api/v1/phones/{id}/resync");
    assert_eq!(admin.post(&resync, json!({})).await.0, StatusCode::OK);
    assert!(std::fs::read_to_string(&sep).unwrap().contains("Before"));

    // The timer runs every 30 s; a resync right after a save must not
    // deliver the previous file.
    let (status, saved) = admin
        .put(
            &format!("/api/v1/extensions/{ext_id}"),
            json!({"number":"999","display_name":"After"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(admin.post(&resync, json!({})).await.0, StatusCode::OK);
    let body = std::fs::read_to_string(&sep).unwrap();
    assert!(body.contains("After") && !body.contains("Before"), "{body}");
}
