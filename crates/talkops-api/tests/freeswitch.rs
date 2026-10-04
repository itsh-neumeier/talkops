//! Tests of the FreeSWITCH-facing endpoints (`/fs/xml`, `/fs/cdr`) against a
//! real database: directory, sofia.conf, dialplan routing and CDR ingestion.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;

const TENANT: &str = "00000000-0000-4000-8000-000000000001";

struct Fixture {
    admin: Client,
    ext20: Value,
    ext21: Value,
    number: Value,
}

/// Admin, extensions 20 (with CLIR off) and 21 (no devices), a LEONET line
/// +49891234567 routed to 20, area code 89 and default number set.
async fn fixture(router: &axum::Router) -> Fixture {
    let admin = setup_admin(router).await;
    let (_, ext20) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "20", "display_name": "Office ${x}"}),
        )
        .await;
    let (_, ext21) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "21", "display_name": "Lab"}),
        )
        .await;
    for name in ["Desk", "DECT"] {
        admin
            .post(
                &format!(
                    "/api/v1/extensions/{}/devices",
                    ext20["id"].as_str().unwrap()
                ),
                json!({"name": name, "kind": "desk"}),
            )
            .await;
    }
    let (_, trunk) = admin
        .post(
            "/api/v1/trunks",
            json!({"name": "LEONET", "preset": "leonet"}),
        )
        .await;
    let (_, line) = admin.post(&format!("/api/v1/trunks/{}/lines", trunk["id"].as_str().unwrap()), json!({
        "e164": "+49891234567", "password": "leo-secret", "destination_extension_id": ext20["id"]
    })).await;
    let (_, mut s) = admin.get("/api/v1/settings").await;
    s["area_code"] = json!("89");
    s["default_number_id"] = line["number"]["id"].clone();
    admin.put("/api/v1/settings", s).await;
    Fixture {
        admin,
        ext20,
        ext21,
        number: line["number"].clone(),
    }
}

fn actions(xml: &str) -> Vec<(String, String)> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name("action"))
        .map(|n| {
            (
                n.attribute("application").unwrap().to_owned(),
                n.attribute("data").unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

fn has(actions: &[(String, String)], app: &str, data: &str) -> bool {
    actions.iter().any(|(a, d)| a == app && d == data)
}

async fn internal_call(router: &axum::Router, ext: &Value, dest: &str) -> Vec<(String, String)> {
    let ext_id = ext["id"].as_str().unwrap();
    let (status, xml) = fs_post(
        router,
        "/fs/xml",
        &[
            ("section", "dialplan"),
            ("Caller-Context", "internal"),
            ("Caller-Destination-Number", dest),
            ("Caller-Caller-ID-Number", "20"),
            ("variable_talkops_tenant_id", TENANT),
            ("variable_talkops_extension_id", ext_id),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(xml.contains("<context name=\"internal\">"), "{xml}");
    actions(&xml)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn directory_and_sofia_conf(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let (_, creds) = f
        .admin
        .get(&format!(
            "/api/v1/extensions/{}/devices",
            f.ext20["id"].as_str().unwrap()
        ))
        .await;
    let device_id = creds[0]["id"].as_str().unwrap();
    let (_, creds) = f
        .admin
        .get(&format!("/api/v1/devices/{device_id}/credentials"))
        .await;

    let user = creds["sip_username"].as_str().unwrap();
    let (status, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "directory"),
            ("action", "sip_auth"),
            ("user", user),
            ("domain", "192.168.1.10"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let pw = doc
        .descendants()
        .find(|n| n.attribute("name") == Some("password"))
        .unwrap();
    assert_eq!(pw.attribute("value"), creds["sip_password"].as_str());
    assert!(
        xml.contains("value=\"Office x\""),
        "display name sanitized: {xml}"
    );

    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[("section", "directory"), ("user", "unknown")],
    )
    .await;
    assert!(xml.contains("not found"));
    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "directory"),
            ("purpose", "gateways"),
            ("user", "20-1"),
        ],
    )
    .await;
    assert!(xml.contains("not found"));

    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[("section", "configuration"), ("key_value", "sofia.conf")],
    )
    .await;
    assert!(
        xml.contains("<profile name=\"internal\">") && xml.contains("<profile name=\"external\">")
    );
    assert!(xml.contains("value=\"leo49891234567\""));
    assert!(xml.contains("value=\"leo-secret\""));
    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[("section", "configuration"), ("key_value", "acl.conf")],
    )
    .await;
    assert!(xml.contains("not found"));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn requires_basic_auth(db: PgPool) {
    let router = router(db);
    let res = raw(
        &router,
        axum::http::Request::post("/fs/xml")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(axum::body::Body::from("section=directory&user=20-1"))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = raw(
        &router,
        axum::http::Request::post("/fs/cdr")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(axum::body::Body::from("cdr=x"))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn internal_routing(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;

    // Extension to extension rings all devices.
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(
        has(
            &a,
            "bridge",
            "user/20-1@talkops.local,user/20-2@talkops.local"
        ),
        "{a:?}"
    );
    assert!(has(&a, "set", "talkops_direction=internal"));
    // Extension without devices.
    let a = internal_call(&router, &f.ext20, "21").await;
    assert!(has(&a, "respond", "480 Temporarily Unavailable"));

    let gw = format!(
        "sofia/gateway/gw-{}",
        f.number["account_id"].as_str().unwrap().replace('-', "")
    );
    // National number via LEONET: national format, caller id without '+'.
    let a = internal_call(&router, &f.ext20, "030 123456").await;
    assert!(has(&a, "bridge", &format!("{gw}/030123456")), "{a:?}");
    assert!(has(&a, "set", "effective_caller_id_number=49891234567"));
    assert!(has(&a, "set", "talkops_destination=+4930123456"));
    // Local number gets the area code.
    let a = internal_call(&router, &f.ext20, "555123").await;
    assert!(has(&a, "bridge", &format!("{gw}/089555123")), "{a:?}");
    // Emergency goes out unchanged via the default trunk.
    let a = internal_call(&router, &f.ext20, "112").await;
    assert!(has(&a, "bridge", &format!("{gw}/112")), "{a:?}");
    // Invalid input.
    let a = internal_call(&router, &f.ext20, "0").await;
    assert!(has(&a, "respond", "404 Not Found"));
    // Injection attempt in the dialed number is stripped to digits.
    let a = internal_call(&router, &f.ext20, "030${system(id)}123").await;
    assert!(a.iter().all(|(_, d)| !d.contains("system")), "{a:?}");

    // CLIR is applied for normal calls but never for emergency calls.
    let mut e = f.ext20.clone();
    e["hide_caller_id"] = json!(true);
    let (status, _) = f
        .admin
        .put(
            &format!("/api/v1/extensions/{}", e["id"].as_str().unwrap()),
            e.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(has(
        &internal_call(&router, &f.ext20, "030123456").await,
        "privacy",
        "full"
    ));
    assert!(!has(
        &internal_call(&router, &f.ext20, "112").await,
        "privacy",
        "full"
    ));

    // Unauthenticated internal call is refused.
    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "dialplan"),
            ("Caller-Context", "internal"),
            ("Caller-Destination-Number", "20"),
        ],
    )
    .await;
    assert!(has(&actions(&xml), "respond", "403 Forbidden"));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn outbound_without_default_number_is_rejected(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "20", "display_name": "A"}),
        )
        .await;
    let a = internal_call(&router, &ext, "030123456").await;
    assert!(has(&a, "respond", "503 Service Unavailable"), "{a:?}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn inbound_routing(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let inbound = |dest: &'static str, extra: Vec<(&'static str, String)>| {
        let router = router.clone();
        async move {
            let mut form: Vec<(&str, String)> = vec![
                ("section", "dialplan".into()),
                ("Caller-Context", "public".into()),
                ("Caller-Destination-Number", dest.into()),
                ("Caller-Caller-ID-Number", "+4930999888".into()),
                ("Caller-Caller-ID-Name", "Müller ${x}".into()),
            ];
            form.extend(extra);
            let form: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let (_, xml) = fs_post(&router, "/fs/xml", &form).await;
            actions(&xml)
        }
    };
    for dest in ["+49891234567", "0891234567", "49891234567"] {
        let a = inbound(dest, vec![]).await;
        assert!(
            has(
                &a,
                "bridge",
                "user/20-1@talkops.local,user/20-2@talkops.local"
            ),
            "{dest}: {a:?}"
        );
        assert!(
            has(&a, "set", "effective_caller_id_number=030999888"),
            "{a:?}"
        );
        assert!(has(&a, "set", "effective_caller_id_name=Müller x"), "{a:?}");
        assert!(has(&a, "set", "talkops_direction=inbound"));
    }
    // Per-number registration: RURI carries the username; the account identifies the number.
    let a = inbound(
        "leo49891234567",
        vec![(
            "variable_talkops_account_id",
            f.number["account_id"].as_str().unwrap().to_owned(),
        )],
    )
    .await;
    assert!(has(&a, "set", "talkops_direction=inbound"), "{a:?}");
    // Unknown number.
    let a = inbound("+4940111", vec![]).await;
    assert!(has(&a, "respond", "404 Not Found"));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn cdr_ingestion(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let ext_id = f.ext20["id"].as_str().unwrap();
    let xml = format!(
        r#"<?xml version="1.0"?><cdr><variables>
        <uuid>call-1</uuid><talkops_direction>outbound</talkops_direction>
        <talkops_tenant_id>{TENANT}</talkops_tenant_id><talkops_extension_id>{ext_id}</talkops_extension_id>
        <talkops_caller_number>20</talkops_caller_number><talkops_destination>%2B4930123456</talkops_destination>
        <start_epoch>1791150000</start_epoch><answer_epoch>1791150003</answer_epoch><end_epoch>1791150033</end_epoch>
        <duration>33</duration><billsec>30</billsec><hangup_cause>NORMAL_CLEARING</hangup_cause>
        </variables></cdr>"#
    );
    for _ in 0..2 {
        let (status, _) = fs_post(&router, "/fs/cdr", &[("cdr", &xml)]).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, calls) = f.admin.get("/api/v1/calls").await;
    assert_eq!(status, StatusCode::OK);
    let calls = calls.as_array().unwrap();
    assert_eq!(calls.len(), 1, "duplicate posts are ignored");
    assert_eq!(calls[0]["destination"], "+4930123456");
    assert_eq!(calls[0]["extension_id"], ext_id);
    let _ = &f.ext21;
}
