//! Browser softphone account: SIP credentials and TURN servers.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::*;

async fn account(state: talkops_api::AppState) -> (StatusCode, serde_json::Value, String) {
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;
    let (_, me) = admin.get("/api/v1/auth/me").await;
    let (status, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "200", "display_name": "Office", "user_id": me["user"]["id"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ext}");
    let (status, acc) = admin
        .post("/api/v1/me/webrtc", json!({"extension_id": ext["id"]}))
        .await;
    (status, acc, me["user"]["id"].as_str().unwrap().to_owned())
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn softphone_account_without_turn(db: PgPool) {
    let (status, acc, _) = account(state(db)).await;
    assert_eq!(status, StatusCode::OK, "{acc}");
    assert_eq!(acc["ws_path"], "/api/v1/webrtc/ws");
    assert_eq!(acc["ice_servers"], json!([]));
    assert_eq!(acc["relay_only"], false);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn softphone_account_with_turn(db: PgPool) {
    let state = state(db).with_turn(
        vec!["turn:pbx.example.com:3478?transport=udp".into(), " ".into()],
        Some("s3cret".into()),
        true,
    );
    let (status, acc, user_id) = account(state).await;
    assert_eq!(status, StatusCode::OK, "{acc}");
    let server = &acc["ice_servers"][0];
    assert_eq!(
        server["urls"],
        json!(["turn:pbx.example.com:3478?transport=udp"])
    );
    let username = server["username"].as_str().unwrap();
    let (expiry, user) = username.split_once(':').unwrap();
    let expiry: u64 = expiry.parse().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(expiry > now + 23 * 3600 && expiry <= now + 24 * 3600 + 5);
    assert_eq!(user, user_id);
    let (_, expected) =
        talkops_core::turn::credentials("s3cret", user, expiry - 24 * 3600, 24 * 3600);
    assert_eq!(server["credential"], expected);
    assert_eq!(acc["relay_only"], true);

    // Without a secret, TURN stays off.
    let off = state_without_secret();
    assert!(off.turn.is_none());
}

fn state_without_secret() -> talkops_api::AppState {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/none")
        .unwrap();
    state(pool).with_turn(vec!["turn:x:3478".into()], Some(String::new()), false)
}

async fn create_user(admin: &Client, name: &str, role: &str) -> serde_json::Value {
    let (status, user) = admin
        .post(
            "/api/v1/users",
            json!({
                "username": name, "display_name": name, "password": "correct-horse", "role": role
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{user}");
    user
}

async fn create_extension(
    admin: &Client,
    number: &str,
    owner: serde_json::Value,
    enabled: bool,
) -> serde_json::Value {
    let (status, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({
                "number": number, "display_name": number, "user_id": owner, "enabled": enabled
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ext}");
    ext
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn selection_permissions_and_disabled_extensions(db: PgPool) {
    let app = router(db);
    let admin = setup_admin(&app).await;
    let user = create_user(&admin, "alice", "user").await;
    let operator = create_user(&admin, "operator", "operator").await;
    let a = create_extension(&admin, "200", user["id"].clone(), true).await;
    let b = create_extension(&admin, "201", user["id"].clone(), true).await;
    let disabled = create_extension(&admin, "202", user["id"].clone(), false).await;
    let other = create_extension(&admin, "203", serde_json::Value::Null, true).await;
    let op_ext = create_extension(&admin, "204", operator["id"].clone(), true).await;
    let alice = login(&app, "alice", "correct-horse").await.unwrap();
    let op = login(&app, "operator", "correct-horse").await.unwrap();

    let (status, choices) = admin.get("/api/v1/me/webrtc/extensions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(choices.as_array().unwrap().len(), 4);
    assert!(choices[0]["outbound_number"].is_null());
    assert!(choices[0].get("sip_password").is_none());
    assert_eq!(
        admin.post("/api/v1/me/webrtc", json!({})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, choices) = alice.get("/api/v1/me/webrtc/extensions").await;
    assert_eq!(choices.as_array().unwrap().len(), 2);
    let (_, choices) = op.get("/api/v1/me/webrtc/extensions").await;
    assert_eq!(choices.as_array().unwrap().len(), 1);
    assert_eq!(choices[0]["extension_id"], op_ext["id"]);
    assert_eq!(
        alice.post("/api/v1/me/webrtc", json!({})).await.1["extension_id"],
        a["id"]
    );
    assert_eq!(
        alice
            .post("/api/v1/me/webrtc", json!({"extension_id": b["id"]}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        admin
            .post("/api/v1/me/webrtc", json!({"extension_id": other["id"]}))
            .await
            .0,
        StatusCode::OK
    );

    for client in [&admin, &alice, &op] {
        assert_eq!(
            client
                .post("/api/v1/me/webrtc", json!({"extension_id": disabled["id"]}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    for client in [&alice, &op] {
        assert_eq!(
            client
                .post("/api/v1/me/webrtc", json!({"extension_id": other["id"]}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            client
                .get(&format!(
                    "/api/v1/me/webrtc/calls?extension_id={}",
                    other["id"].as_str().unwrap()
                ))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            client
                .post(
                    "/api/v1/me/webrtc/conference",
                    json!({"extension_id": other["id"], "call_id": "test", "number": "200"})
                )
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn separate_accounts_reuse_and_revocation(db: PgPool) {
    use talkops_core::{extensions, tenant::TenantId};
    let app = router(db.clone());
    let admin = setup_admin(&app).await;
    let user = create_user(&admin, "alice", "user").await;
    let ext = create_extension(&admin, "200", user["id"].clone(), true).await;
    let alice = login(&app, "alice", "correct-horse").await.unwrap();
    let request = json!({"extension_id": ext["id"]});
    let (a, b) = tokio::join!(
        alice.post("/api/v1/me/webrtc", request.clone()),
        alice.post("/api/v1/me/webrtc", request.clone())
    );
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(b.0, StatusCode::OK);
    assert_eq!(a.1["sip_username"], b.1["sip_username"]);
    assert_eq!(a.1["sip_password"], b.1["sip_password"]);
    let (status, admin_account) = admin.post("/api/v1/me/webrtc", request.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(a.1["sip_username"], admin_account["sip_username"]);
    assert_ne!(a.1["sip_password"], admin_account["sip_password"]);
    let id = uuid::Uuid::parse_str(ext["id"].as_str().unwrap()).unwrap();
    let username = a.1["sip_username"].as_str().unwrap();
    let devices = extensions::list_devices(&db, TenantId::DEFAULT, id)
        .await
        .unwrap();
    let admin_device = devices
        .iter()
        .find(|d| d.sip_username == admin_account["sip_username"].as_str().unwrap())
        .unwrap();
    let own_device = devices.iter().find(|d| d.sip_username == username).unwrap();
    assert_eq!(
        alice
            .get(&format!("/api/v1/devices/{}/credentials", admin_device.id))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        alice
            .post(
                &format!("/api/v1/devices/{}/reset-password", admin_device.id),
                json!({})
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        alice
            .get(&format!("/api/v1/devices/{}/credentials", own_device.id))
            .await
            .0,
        StatusCode::OK
    );
    assert!(
        extensions::device_auth(&db, username)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(extensions::ring_targets(&db, id).await.unwrap().len(), 2);
    sqlx::query("UPDATE extensions SET user_id = NULL WHERE id = $1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    assert!(
        extensions::device_auth(&db, username)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(extensions::ring_targets(&db, id).await.unwrap().len(), 1);
    assert!(
        extensions::device_auth(&db, admin_account["sip_username"].as_str().unwrap())
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        alice.post("/api/v1/me/webrtc", request.clone()).await.0,
        StatusCode::NOT_FOUND
    );
    let devices = extensions::list_devices(&db, TenantId::DEFAULT, id)
        .await
        .unwrap();
    sqlx::query("UPDATE devices SET enabled = false WHERE id = $1")
        .bind(
            devices
                .iter()
                .find(|d| d.sip_username == admin_account["sip_username"].as_str().unwrap())
                .unwrap()
                .id,
        )
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        admin.post("/api/v1/me/webrtc", request).await.0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn tenant_isolation_and_call_history(db: PgPool) {
    use talkops_core::{cdr, tenant::TenantId};
    let app = router(db.clone());
    let admin = setup_admin(&app).await;
    let user = create_user(&admin, "alice", "user").await;
    let ext = create_extension(&admin, "200", user["id"].clone(), true).await;
    let id = uuid::Uuid::parse_str(ext["id"].as_str().unwrap()).unwrap();
    let tenant = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, 'Other')")
        .bind(tenant)
        .execute(&db)
        .await
        .unwrap();
    let foreign: uuid::Uuid = sqlx::query_scalar("INSERT INTO extensions (tenant_id, number, display_name) VALUES ($1, '300', 'Other') RETURNING id").bind(tenant).fetch_one(&db).await.unwrap();
    assert_eq!(
        admin
            .post("/api/v1/me/webrtc", json!({"extension_id": foreign}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, choices) = admin.get("/api/v1/me/webrtc/extensions").await;
    assert_eq!(choices.as_array().unwrap().len(), 1);
    for (caller, destination, from, to) in [
        ("+49123", "200", None, Some(id)),
        ("200", "+49456", Some(id), None),
    ] {
        cdr::insert(
            &db,
            TenantId::DEFAULT,
            &cdr::Cdr {
                id: uuid::Uuid::nil(),
                call_uuid: uuid::Uuid::new_v4().to_string(),
                direction: if from.is_some() {
                    cdr::Direction::Outbound
                } else {
                    cdr::Direction::Inbound
                },
                caller_number: caller.into(),
                caller_name: String::new(),
                destination: destination.into(),
                extension_id: from,
                dest_extension_id: to,
                trunk_id: None,
                number_id: None,
                started_at: chrono::Utc::now(),
                answered_at: None,
                ended_at: chrono::Utc::now(),
                duration_secs: 0,
                billsec: 0,
                hangup_cause: "NO_ANSWER".into(),
                recording_id: None,
            },
        )
        .await
        .unwrap();
    }
    let alice = login(&app, "alice", "correct-horse").await.unwrap();
    let (status, calls) = alice
        .get(&format!("/api/v1/me/webrtc/calls?extension_id={id}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls.as_array().unwrap().len(), 2);
    assert_eq!(
        alice
            .get(&format!("/api/v1/me/webrtc/calls?extension_id={foreign}"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn choices_and_dialplan_use_the_selected_outbound_number(db: PgPool) {
    let app = router(db);
    let admin = setup_admin(&app).await;
    let first = create_extension(&admin, "200", serde_json::Value::Null, true).await;
    let second = create_extension(&admin, "201", serde_json::Value::Null, true).await;
    let (_, trunk) = admin
        .post(
            "/api/v1/trunks",
            json!({"name": "LEONET", "preset": "leonet"}),
        )
        .await;
    let lines_url = format!("/api/v1/trunks/{}/lines", trunk["id"].as_str().unwrap());
    let (status, line_a) = admin.post(&lines_url, json!({"e164": "+49891234567", "password": "fixture", "destination_extension_id": first["id"]})).await;
    assert_eq!(status, StatusCode::OK, "{line_a}");
    let (status, line_b) = admin.post(&lines_url, json!({"e164": "+49897654321", "password": "fixture", "destination_extension_id": second["id"]})).await;
    assert_eq!(status, StatusCode::OK, "{line_b}");
    let (_, mut settings) = admin.get("/api/v1/settings").await;
    settings["default_number_id"] = line_a["number"]["id"].clone();
    assert_eq!(
        admin.put("/api/v1/settings", settings).await.0,
        StatusCode::OK
    );
    let (_, mut second) = admin
        .get(&format!(
            "/api/v1/extensions/{}",
            second["id"].as_str().unwrap()
        ))
        .await;
    second["outbound_number_id"] = line_b["number"]["id"].clone();
    assert_eq!(
        admin
            .put(
                &format!("/api/v1/extensions/{}", second["id"].as_str().unwrap()),
                second.clone()
            )
            .await
            .0,
        StatusCode::OK
    );
    let (_, choices) = admin.get("/api/v1/me/webrtc/extensions").await;
    assert_eq!(choices[0]["outbound_number"], "+49891234567");
    assert_eq!(choices[0]["uses_default_number"], true);
    assert_eq!(choices[1]["outbound_number"], "+49897654321");
    assert_eq!(choices[1]["uses_default_number"], false);
    for (ext, public) in [(&first, "+49891234567"), (&second, "+49897654321")] {
        let (status, account) = admin
            .post("/api/v1/me/webrtc", json!({"extension_id": ext["id"]}))
            .await;
        assert_eq!(status, StatusCode::OK);
        let (_, directory) = fs_post(
            &app,
            "/fs/xml",
            &[
                ("section", "directory"),
                ("user", account["sip_username"].as_str().unwrap()),
            ],
        )
        .await;
        assert!(
            directory.contains(ext["id"].as_str().unwrap()),
            "{directory}"
        );
        let (_, xml) = fs_post(
            &app,
            "/fs/xml",
            &[
                ("section", "dialplan"),
                ("Caller-Context", "internal"),
                ("Caller-Destination-Number", "+4912345678"),
                (
                    "variable_talkops_tenant_id",
                    "00000000-0000-4000-8000-000000000001",
                ),
                ("variable_talkops_extension_id", ext["id"].as_str().unwrap()),
            ],
        )
        .await;
        // LEONET sends caller IDs in international format without the leading '+'.
        assert!(
            xml.contains(&format!(
                "data=\"effective_caller_id_number={}\"",
                public.trim_start_matches('+')
            )),
            "{xml}"
        );
        let number_id = if ext["id"] == first["id"] {
            &line_a["number"]["id"]
        } else {
            &line_b["number"]["id"]
        };
        assert!(
            xml.contains(&format!(
                "data=\"talkops_number_id={}\"",
                number_id.as_str().unwrap()
            )),
            "{xml}"
        );
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn migration_preserves_existing_owner_credentials(db: PgPool) {
    use talkops_core::{crypto::SecretBox, extensions, tenant::TenantId};
    let app = router(db.clone());
    let admin = setup_admin(&app).await;
    let user = create_user(&admin, "alice", "user").await;
    let ext = create_extension(&admin, "200", user["id"].clone(), true).await;
    let id = uuid::Uuid::parse_str(ext["id"].as_str().unwrap()).unwrap();
    let extension = extensions::get(&db, TenantId::DEFAULT, id).await.unwrap();
    let secrets = SecretBox::from_hex(KEY).unwrap();
    let input = extensions::DeviceInput {
        name: "Browser".into(),
        kind: extensions::DeviceKind::Browser,
        sip_username: None,
        phone_id: None,
        account_index: None,
        phone_label: String::new(),
        phone_display_name: String::new(),
        enabled: true,
    };
    let (legacy, password) =
        extensions::create_device(&db, TenantId::DEFAULT, &secrets, &extension, &input)
            .await
            .unwrap();
    // Reproduce the schema before this migration in this isolated test database.
    sqlx::raw_sql(
        "DROP INDEX devices_browser_user_idx; ALTER TABLE devices DROP COLUMN browser_user_id;",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261026000002_browser_account_owners.sql"
    ))
    .execute(&db)
    .await
    .unwrap();
    let alice = login(&app, "alice", "correct-horse").await.unwrap();
    let (status, account) = alice.post("/api/v1/me/webrtc", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(account["sip_username"], legacy.sip_username);
    assert_eq!(account["sip_password"], password);
    assert_eq!(
        extensions::list_devices(&db, TenantId::DEFAULT, id)
            .await
            .unwrap()
            .len(),
        1
    );
}
