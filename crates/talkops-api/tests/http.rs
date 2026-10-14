//! HTTP-level tests: health, auth flow, CSRF, roles and CRUD.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn health_endpoints(db: PgPool) {
    let router = router(db);
    for path in [
        "/healthz",
        "/readyz",
        "/api/v1/status",
        "/api/v1/openapi.json",
        "/api/v1/setup",
    ] {
        let res = raw(&router, Request::get(path).body(Body::empty()).unwrap()).await;
        assert_eq!(res.status(), StatusCode::OK, "{path}");
    }
    let res = raw(
        &router,
        Request::get("/api/v1/openapi.json")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
    assert_eq!(res.headers()["x-frame-options"], "DENY");
    assert_eq!(
        res.headers()["content-security-policy"],
        "frame-ancestors 'none'"
    );
    let doc = body_json(res).await;
    assert!(
        doc["paths"]["/api/v1/trunks/{id}/lines"]["post"].is_object(),
        "OpenAPI lists all routes"
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn prometheus_metrics(db: PgPool) {
    let get = |router: Router, auth: Option<&'static str>| async move {
        let mut req = Request::get("/metrics");
        if let Some(a) = auth {
            req = req.header(header::AUTHORIZATION, a);
        }
        raw(&router, req.body(Body::empty()).unwrap()).await
    };
    // Disabled without a token.
    let res = get(router(db.clone()), Some("Bearer x")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let router = talkops_api::app(state(db).with_metrics_token(Some("scrape-me")), None);
    assert_eq!(
        get(router.clone(), None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(router.clone(), Some("Bearer wrong")).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let res = get(router, Some("Bearer scrape-me")).await;
    assert_eq!(res.status(), StatusCode::OK);
    let text = body_string(res).await;
    for metric in [
        "talkops_build_info{version=",
        "talkops_freeswitch_connected 0",
        "talkops_registrations 0",
        "talkops_recordings 0",
        "talkops_voicemail_new 0",
        "talkops_database_up 1",
    ] {
        assert!(text.contains(metric), "{metric} missing in:\n{text}");
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn setup_login_csrf_and_roles(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;

    // Setup only works once.
    let res = raw(
        &router,
        Request::post("/api/v1/setup")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-requested-with", "TalkOps")
            .body(Body::from(
                r#"{"username":"x","display_name":"x","password":"correct-horse"}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);

    let (status, me) = admin.get("/api/v1/auth/me").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["user"]["role"], "admin");
    assert!(
        me["user"].get("password_hash").is_none(),
        "hash never leaves the server"
    );

    // Missing CSRF token / X-Requested-With are rejected.
    let res = raw(
        &router,
        Request::post("/api/v1/users")
            .header(header::COOKIE, &admin.cookie)
            .header("x-requested-with", "TalkOps")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = raw(
        &router,
        Request::post("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"username":"admin","password":"correct-horse"}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Unauthenticated access.
    let res = raw(
        &router,
        Request::get("/api/v1/users").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // A normal user cannot administrate.
    let (status, _) = admin.post("/api/v1/users", json!({"username": "bob", "display_name": "Bob", "role": "user", "password": "bob-password-1"})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(login(&router, "bob", "wrong-password").await.is_none());
    let bob = login(&router, "bob", "bob-password-1").await.unwrap();
    let (status, _) = bob.get("/api/v1/users").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = bob
        .post(
            "/api/v1/extensions",
            json!({"number": "300", "display_name": "x"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The last admin cannot lock themself out.
    let admin_id = me["user"]["id"].as_str().unwrap();
    let (status, _) = admin
        .put(
            &format!("/api/v1/users/{admin_id}"),
            json!({"display_name": "A", "role": "user", "enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Logout invalidates the session.
    let (status, _) = admin.call("POST", "/api/v1/auth/logout", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = admin.get("/api/v1/auth/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, audit) = login(&router, "admin", "correct-horse")
        .await
        .unwrap()
        .get("/api/v1/audit")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(audit.as_array().unwrap().len() >= 2);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn login_is_rate_limited(db: PgPool) {
    let router = router(db);
    setup_admin(&router).await;
    for _ in 0..10 {
        assert!(login(&router, "admin", "nope-nope-nope").await.is_none());
    }
    // Even the right password is refused while limited.
    assert!(login(&router, "admin", "correct-horse").await.is_none());
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn extensions_devices_and_self_service(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, bob) = admin.post("/api/v1/users", json!({"username": "bob", "display_name": "Bob", "role": "user", "password": "bob-password-1"})).await;
    let (status, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "200", "display_name": "Bob", "user_id": bob["id"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "112", "display_name": "x"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "200", "display_name": "dup"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Internal numbers start at 100: *1-*99 are system codes.
    let (status, err) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "20", "display_name": "short"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        err["message"].as_str().unwrap().contains("start at 100"),
        "{err}"
    );

    let ext_id = ext["id"].as_str().unwrap();
    let (status, creds) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "Desk", "kind": "desk"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    // Generated: random, not derived from the extension number.
    let generated = creds["sip_username"].as_str().unwrap().to_owned();
    assert_eq!(generated.len(), 6, "{generated}");
    assert!(!generated.starts_with("200"), "{generated}");
    assert_eq!(creds["sip_password"].as_str().unwrap().len(), 20);

    // Bob sees his phone and may read his credentials; not someone else's.
    let bob_client = login(&router, "bob", "bob-password-1").await.unwrap();
    let (status, phones) = bob_client.get("/api/v1/me/phones").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(phones[0]["devices"][0]["sip_username"], generated.as_str());
    assert!(phones[0]["devices"][0].get("sip_password_enc").is_none());
    let device_id = creds["device_id"].as_str().unwrap();
    let (status, again) = bob_client
        .get(&format!("/api/v1/devices/{device_id}/credentials"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["sip_password"], creds["sip_password"]);

    // A free SIP username, independent of the extension number (Dahua:
    // digits only). Only admins rename; the rename is audited.
    let (status, vth) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "VTH", "kind": "other", "sip_username": "Hall6578"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{vth}");
    assert_eq!(vth["sip_username"], "Hall6578");
    let (status, err) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "X", "kind": "desk", "sip_username": "web-x"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(err["message"].as_str().unwrap().contains("web-"), "{err}");
    let (status, err) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "X", "kind": "desk", "sip_username": "HALL6578"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        err["message"].as_str().unwrap().contains("already in use"),
        "{err}"
    );
    let rename = json!({"name": "VTH", "kind": "other", "sip_username": "123"});
    let vth_id = vth["device_id"].as_str().unwrap();
    let (status, _) = bob_client
        .put(&format!("/api/v1/devices/{vth_id}"), rename.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, renamed) = admin
        .put(&format!("/api/v1/devices/{vth_id}"), rename)
        .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["sip_username"], "123");
    let (_, audit) = admin.get("/api/v1/audit").await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["details"]["previous_sip_username"] == "Hall6578"
                && e["details"]["sip_username"] == "123"),
        "{audit}"
    );
    admin
        .call("DELETE", &format!("/api/v1/devices/{vth_id}"), None)
        .await;

    let (_, other) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "210", "display_name": "Other"}),
        )
        .await;
    let (status, _) = bob_client
        .get(&format!(
            "/api/v1/extensions/{}",
            other["id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn trunk_lines_and_numbers(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (status, presets) = admin.get("/api/v1/presets").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        presets
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "leonet")
    );

    let (status, trunk) = admin
        .post(
            "/api/v1/trunks",
            json!({"name": "LEONET", "preset": "leonet"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let trunk_id = trunk["id"].as_str().unwrap();
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "200", "display_name": "Office"}),
        )
        .await;

    let (status, line) = admin.post(&format!("/api/v1/trunks/{trunk_id}/lines"), json!({
        "e164": "+49891234567", "password": "leo-secret", "destination_extension_id": ext["id"]
    })).await;
    assert_eq!(status, StatusCode::OK, "{line}");
    assert_eq!(line["account"]["username"], "leo49891234567");
    assert!(line["account"].get("password_enc").is_none());

    // Invalid number rolls back the account.
    let (status, _) = admin
        .post(
            &format!("/api/v1/trunks/{trunk_id}/lines"),
            json!({"e164": "089123", "password": "x"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, detail) = admin.get(&format!("/api/v1/trunks/{trunk_id}")).await;
    assert_eq!(detail["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(detail["numbers"][0]["e164"], "+49891234567");

    // Shared-account providers reject the per-number shortcut.
    let (_, sg) = admin
        .post(
            "/api/v1/trunks",
            json!({"name": "sipgate", "preset": "sipgate-trunking"}),
        )
        .await;
    let (status, _) = admin
        .post(
            &format!("/api/v1/trunks/{}/lines", sg["id"].as_str().unwrap()),
            json!({"e164": "+4930123456", "password": "x"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Settings round trip.
    let (_, mut s) = admin.get("/api/v1/settings").await;
    s["area_code"] = json!("89");
    s["default_number_id"] = line["number"]["id"].clone();
    let (status, saved) = admin.put("/api/v1/settings", s.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["area_code"], "89");
    s["external_ip"] = json!("not an ip");
    let (status, _) = admin.put("/api/v1/settings", s).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn two_factor_login(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let now = || chrono::Utc::now().timestamp() as u64;
    let login_raw = |user: &str, pw: &str| {
        Request::post("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-requested-with", "TalkOps")
            .body(Body::from(
                json!({"username": user, "password": pw}).to_string(),
            ))
            .unwrap()
    };

    let (_, st) = admin.get("/api/v1/auth/totp").await;
    assert_eq!(st["enabled"], false);
    assert_eq!(st["available"], true);
    // Enabling needs a valid code of the pending secret.
    let (status, setup) = admin.post("/api/v1/auth/totp/setup", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let secret = setup["secret"].as_str().unwrap().to_owned();
    assert!(setup["qr_svg"].as_str().unwrap().contains("<svg"));
    assert!(
        setup["otpauth_uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/TalkOps:admin?")
    );
    let (status, _) = admin
        .post("/api/v1/auth/totp/enable", json!({"code": "000000"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let code = talkops_core::mfa::code_for(&secret, now()).unwrap();
    let (status, codes) = admin
        .post("/api/v1/auth/totp/enable", json!({"code": code}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let recovery: Vec<String> = codes["recovery_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(recovery.len(), 10);

    // Password alone gives no session, only a challenge.
    let res = raw(&router, login_raw("admin", "correct-horse")).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().get(header::SET_COOKIE).is_none());
    let body = body_json(res).await;
    assert_eq!(body["mfa_required"], true);
    let token = body["mfa_token"].as_str().unwrap().to_owned();
    let second = |token: &str, code: &str| {
        Request::post("/api/v1/auth/login/totp")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-requested-with", "TalkOps")
            .body(Body::from(
                json!({"mfa_token": token, "code": code}).to_string(),
            ))
            .unwrap()
    };
    assert_eq!(
        raw(&router, second(&token, "123456")).await.status(),
        StatusCode::UNAUTHORIZED
    );
    // The code used for enabling cannot be replayed; the next one works.
    assert_eq!(
        raw(&router, second(&token, &code)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let next = talkops_core::mfa::code_for(&secret, now() + 30).unwrap();
    let res = raw(&router, second(&token, &next)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().get(header::SET_COOKIE).is_some());
    let me = body_json(res).await;
    assert_eq!(me["user"]["totp_enabled"], true);
    // The challenge is used up.
    assert_eq!(
        raw(&router, second(&token, &next)).await.status(),
        StatusCode::UNAUTHORIZED
    );

    // A recovery code works once (any case, with or without dash).
    let body = body_json(raw(&router, login_raw("admin", "correct-horse")).await).await;
    let token = body["mfa_token"].as_str().unwrap().to_owned();
    let rc = recovery[0].to_uppercase().replace('-', "");
    assert_eq!(
        raw(&router, second(&token, &rc)).await.status(),
        StatusCode::OK
    );
    let body = body_json(raw(&router, login_raw("admin", "correct-horse")).await).await;
    let token = body["mfa_token"].as_str().unwrap().to_owned();
    assert_eq!(
        raw(&router, second(&token, &recovery[0])).await.status(),
        StatusCode::UNAUTHORIZED
    );
    // Too many wrong codes end the challenge even before a right one.
    for _ in 0..5 {
        raw(&router, second(&token, "111111")).await;
    }
    assert_eq!(
        raw(&router, second(&token, &recovery[1])).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let (_, st) = admin.get("/api/v1/auth/totp").await;
    assert_eq!(st["recovery_codes_left"], 9);

    // Disabling needs the password; an admin can reset another user's 2FA.
    let (status, _) = admin
        .post("/api/v1/auth/totp/disable", json!({"password": "wrong"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, me) = admin.get("/api/v1/auth/me").await;
    let (status, _) = admin
        .post(
            &format!(
                "/api/v1/users/{}/totp/reset",
                me["user"]["id"].as_str().unwrap()
            ),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // The reset ended all sessions; a plain password login works again.
    assert_eq!(
        admin.get("/api/v1/auth/me").await.0,
        StatusCode::UNAUTHORIZED
    );
    let res = raw(&router, login_raw("admin", "correct-horse")).await;
    assert!(res.headers().get(header::SET_COOKIE).is_some());
}

/// SIP password formats: the standard stays 20 characters, `compact15` gives
/// 15. Only admins choose the format; owners reset with the stored one. The
/// audit names the format, never the password.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn sip_password_formats(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    for (name, role) in [("bob", "user"), ("olga", "operator")] {
        admin
            .post(
                "/api/v1/users",
                json!({"username": name, "display_name": name, "role": role,
                       "password": format!("{name}-password-1")}),
            )
            .await;
    }
    let (_, users) = admin.get("/api/v1/users").await;
    let bob = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "bob")
        .unwrap()
        .clone();
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "8001", "display_name": "Hall", "user_id": bob["id"]}),
        )
        .await;
    let devices = format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap());

    // Unchanged default for clients without the field.
    let (status, desk) = admin
        .post(&devices, json!({"name": "Desk", "kind": "desk"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{desk}");
    assert_eq!(desk["sip_password"].as_str().unwrap().len(), 20);

    let (status, vth) = admin
        .post(
            &devices,
            json!({"name": "VTH", "kind": "door", "sip_username": "8001vth",
                   "sip_password_format": "compact15"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{vth}");
    let pw = vth["sip_password"].as_str().unwrap().to_owned();
    assert_eq!(pw.len(), 15);
    assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
    let vth_id = vth["device_id"].as_str().unwrap().to_owned();
    let (_, list) = admin.get(&devices).await;
    let stored = list
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == vth_id.as_str())
        .unwrap();
    assert_eq!(stored["sip_password_format"], "compact15");
    assert!(stored.get("sip_password_enc").is_none());
    let (status, _) = admin
        .post(
            &devices,
            json!({"name": "X", "kind": "desk", "sip_password_format": "short"}),
        )
        .await;
    assert!(status.is_client_error(), "{status}");

    // FreeSWITCH gets the 15-character password.
    let (status, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "directory"),
            ("action", "sip_auth"),
            ("user", "8001vth"),
            ("domain", "VDP"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(xml.contains(&format!("value=\"{pw}\"")), "{xml}");

    // The owner resets with the stored format (no body, as before).
    let bob_client = login(&router, "bob", "bob-password-1").await.unwrap();
    let reset = format!("/api/v1/devices/{vth_id}/reset-password");
    let (status, creds) = bob_client.call("POST", &reset, None).await;
    assert_eq!(status, StatusCode::OK, "{creds}");
    let pw2 = creds["sip_password"].as_str().unwrap().to_owned();
    assert_eq!(pw2.len(), 15);
    assert_ne!(pw2, pw);
    let (status, creds) = bob_client.post(&reset, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(creds["sip_password"].as_str().unwrap().len(), 15);
    // The same format in the body is no change.
    let (status, _) = bob_client
        .post(&reset, json!({"sip_password_format": "compact15"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    // Only an admin changes the format.
    let (status, _) = bob_client
        .post(&reset, json!({"sip_password_format": "standard"}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let olga = login(&router, "olga", "olga-password-1").await.unwrap();
    let (status, _) = olga.call("POST", &reset, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Only an admin edits the device (and its stored format).
    let (status, _) = bob_client
        .put(
            &format!("/api/v1/devices/{vth_id}"),
            json!({"name": "VTH", "kind": "door", "sip_password_format": "standard"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, creds) = admin
        .post(&reset, json!({"sip_password_format": "standard"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let pw3 = creds["sip_password"].as_str().unwrap().to_owned();
    assert_eq!(pw3.len(), 20);

    // Editing the format keeps the current password.
    let (status, dev) = admin
        .put(
            &format!("/api/v1/devices/{vth_id}"),
            json!({"name": "VTH", "kind": "door", "sip_password_format": "compact15"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{dev}");
    assert_eq!(dev["sip_password_format"], "compact15");
    let (_, creds) = admin
        .get(&format!("/api/v1/devices/{vth_id}/credentials"))
        .await;
    assert_eq!(creds["sip_password"], pw3.as_str());

    let (_, audit) = admin.get("/api/v1/audit").await;
    let audit_text = audit.to_string();
    for secret in [&pw, &pw2, &pw3] {
        assert!(!audit_text.contains(secret.as_str()), "password in audit");
    }
    let entries = audit.as_array().unwrap();
    let has = |action: &str, f: &dyn Fn(&serde_json::Value) -> bool| {
        entries
            .iter()
            .any(|e| e["action"] == action && e["entity_id"] == vth_id.as_str() && f(&e["details"]))
    };
    assert!(
        has("create", &|d| d["sip_password_format"] == "compact15"),
        "{audit}"
    );
    assert!(
        has("reset_password", &|d| d["sip_password_format"]
            == "compact15"
            && d.get("previous_sip_password_format").is_none()),
        "{audit}"
    );
    assert!(
        has("reset_password", &|d| d["sip_password_format"]
            == "standard"
            && d["previous_sip_password_format"] == "compact15"),
        "{audit}"
    );
    assert!(
        has("update", &|d| d["sip_password_format"] == "compact15"
            && d["previous_sip_password_format"] == "standard"),
        "{audit}"
    );
}
