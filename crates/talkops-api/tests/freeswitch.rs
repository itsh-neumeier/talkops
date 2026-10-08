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

/// Bridge string ringing both devices of extension 20 plus its pickup group.
fn ring20(f: &Fixture) -> String {
    format!(
        "user/20-1@talkops.local,user/20-2@talkops.local,pickup/ext-{}",
        f.ext20["id"].as_str().unwrap().replace('-', "")
    )
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
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");
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

    // Feature codes: DND on/off. Toggles write no CDR.
    let a = internal_call(&router, &f.ext20, "*78").await;
    assert!(has(&a, "answer", ""), "{a:?}");
    assert!(
        !a.iter().any(|(_, d)| d.starts_with("talkops_direction")),
        "{a:?}"
    );
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(has(&a, "respond", "486 Busy Here"), "{a:?}");
    internal_call(&router, &f.ext20, "*79").await;
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");

    // Call forwarding to another extension (21 has no devices) and outside.
    internal_call(&router, &f.ext20, "*7221").await;
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(has(&a, "respond", "480 Temporarily Unavailable"), "{a:?}");
    internal_call(&router, &f.ext20, "*72030123456").await;
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(has(&a, "bridge", &format!("{gw}/030123456")), "{a:?}");
    assert!(has(&a, "set", "talkops_forwarded_from=20"), "{a:?}");
    // Forwarding to itself is refused, *73 clears it.
    let a = internal_call(&router, &f.ext20, "*7220").await;
    assert!(has(&a, "respond", "484 Address Incomplete"), "{a:?}");
    internal_call(&router, &f.ext20, "*73").await;
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");

    // Directed pickup.
    let a = internal_call(&router, &f.ext21, "**20").await;
    let group = format!("ext-{}", f.ext20["id"].as_str().unwrap().replace('-', ""));
    assert!(has(&a, "pickup", &group), "{a:?}");
    let a = internal_call(&router, &f.ext21, "**99").await;
    assert!(has(&a, "respond", "404 Not Found"), "{a:?}");

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
        assert!(has(&a, "bridge", &ring20(&f)), "{dest}: {a:?}");
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

    // The called extension's owner sees internal calls too.
    let (_, bob) = f.admin.post("/api/v1/users", json!({"username": "bob", "display_name": "Bob", "role": "user", "password": "bob-password-1"})).await;
    let mut e21 = f.ext21.clone();
    e21["user_id"] = bob["id"].clone();
    f.admin
        .put(
            &format!("/api/v1/extensions/{}", e21["id"].as_str().unwrap()),
            e21.clone(),
        )
        .await;
    let internal = xml
        .replace("call-1", "call-2")
        .replace("<talkops_direction>outbound", "<talkops_direction>internal")
        .replace(
            "</talkops_extension_id>",
            &format!(
                "</talkops_extension_id><talkops_dest_extension_id>{}</talkops_dest_extension_id>",
                e21["id"].as_str().unwrap()
            ),
        );
    fs_post(&router, "/fs/cdr", &[("cdr", &internal)]).await;
    let bob_client = login(&router, "bob", "bob-password-1").await.unwrap();
    let (_, calls) = bob_client.get("/api/v1/calls").await;
    let calls = calls.as_array().unwrap();
    assert_eq!(
        calls.len(),
        1,
        "bob sees only the internal call to his extension"
    );
    assert_eq!(calls[0]["call_uuid"], "call-2");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn voicemail_routing(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let id = |e: &Value| e["id"].as_str().unwrap().to_owned();
    let socket = "127.0.0.1:8084 async full";
    let enable = |ext: String| {
        let admin = &f.admin;
        async move {
            let (status, b) = admin
                .put(
                    &format!("/api/v1/extensions/{ext}/voicemail"),
                    json!({"enabled": true, "pin": "1234"}),
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{b}");
        }
    };

    // Without a box: unreachable and *97 are rejected as before.
    let a = internal_call(&router, &f.ext20, "21").await;
    assert!(has(&a, "respond", "480 Temporarily Unavailable"));
    let a = internal_call(&router, &f.ext20, "*97").await;
    assert!(has(&a, "respond", "404 Not Found"));

    // 21 has no devices: straight to voicemail.
    enable(id(&f.ext21)).await;
    let a = internal_call(&router, &f.ext20, "21").await;
    assert!(has(&a, "set", "talkops_app=vm_deposit"), "{a:?}");
    assert!(has(
        &a,
        "set",
        &format!("talkops_vm_extension_id={}", id(&f.ext21))
    ));
    assert!(has(&a, "socket", socket));

    // 20 rings first; unanswered calls continue to voicemail.
    enable(id(&f.ext20)).await;
    let a = internal_call(&router, &f.ext21, "20").await;
    let bridge = a.iter().position(|(app, _)| app == "bridge").unwrap();
    let sock = a.iter().position(|(app, _)| app == "socket").unwrap();
    assert!(bridge < sock, "{a:?}");
    assert!(!has(&a, "hangup", ""));

    // DND sends callers to voicemail instead of busy.
    internal_call(&router, &f.ext20, "*78").await;
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(!a.iter().any(|(app, _)| app == "bridge"), "{a:?}");
    assert!(has(&a, "set", "talkops_app=vm_deposit"));
    internal_call(&router, &f.ext20, "*79").await;

    // Mailbox access.
    let a = internal_call(&router, &f.ext20, "*97").await;
    assert!(has(&a, "set", "talkops_app=vm_check"), "{a:?}");
    assert!(has(
        &a,
        "set",
        &format!("talkops_vm_extension_id={}", id(&f.ext20))
    ));
    let a = internal_call(&router, &f.ext21, "*98").await;
    assert!(has(&a, "set", "talkops_app=vm_login"), "{a:?}");
    assert!(has(&a, "socket", socket));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn ring_group_routing(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let id = |e: &Value| e["id"].as_str().unwrap().to_owned();
    let pickup20 = format!("pickup/ext-{}", id(&f.ext20).replace('-', ""));

    // Number space is shared with extensions.
    let (status, _) = f
        .admin
        .post(
            "/api/v1/ring-groups",
            json!({"number": "20", "name": "Clash", "members": [id(&f.ext20)]}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, g) = f
        .admin
        .post(
            "/api/v1/ring-groups",
            json!({"number": "50", "name": "Support", "caller_id_prefix": "Support: ",
                   "members": [id(&f.ext21), id(&f.ext20)]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{g}");
    let group_id = id(&g);
    let (status, _) = f
        .admin
        .post(
            "/api/v1/extensions",
            json!({"number": "50", "display_name": "Clash"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Simultaneous: 21 has no devices and is skipped.
    let a = internal_call(&router, &f.ext21, "50").await;
    assert!(
        has(
            &a,
            "bridge",
            &format!("user/20-1@talkops.local,user/20-2@talkops.local,{pickup20}")
        ),
        "{a:?}"
    );
    assert!(has(&a, "set", "call_timeout=25"));
    assert!(
        has(&a, "set", "effective_caller_id_name=Support: Lab"),
        "{a:?}"
    );
    assert!(has(&a, "set", "talkops_destination=50"));
    assert!(has(&a, "hangup", ""));

    // Sequential with a fallback to the voicemail of 21.
    let (status, _) = f
        .admin
        .put(
            &format!("/api/v1/extensions/{}/voicemail", id(&f.ext21)),
            json!({"enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, g) = f
        .admin
        .put(
            &format!("/api/v1/ring-groups/{group_id}"),
            json!({"number": "50", "name": "Support", "strategy": "sequential", "ring_timeout_secs": 15,
                   "caller_id_prefix": "Support: ",
                   "members": [id(&f.ext20), id(&f.ext21)],
                   "fallback_type": "voicemail", "fallback_id": id(&f.ext21)}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{g}");
    let a = internal_call(&router, &f.ext21, "50").await;
    assert!(
        has(
            &a,
            "bridge",
            &format!(
                "[leg_timeout=15]user/20-1@talkops.local,[leg_timeout=15]user/20-2@talkops.local,\
                 [leg_timeout=15]{pickup20}"
            )
        ),
        "{a:?}"
    );
    assert!(has(&a, "set", "talkops_app=vm_deposit"), "{a:?}");
    assert!(!has(&a, "hangup", ""));

    // Nobody available (20 on DND): straight to the fallback.
    internal_call(&router, &f.ext20, "*78").await;
    let a = internal_call(&router, &f.ext21, "50").await;
    assert!(!a.iter().any(|(app, _)| app == "bridge"), "{a:?}");
    assert!(has(&a, "set", "talkops_app=vm_deposit"));
    internal_call(&router, &f.ext20, "*79").await;

    // A phone number can point to the group.
    let mut n = f.number.clone();
    n["destination_type"] = json!("ring_group");
    n["destination_id"] = json!(group_id);
    let (status, body) = f
        .admin
        .put(&format!("/api/v1/numbers/{}", id(&f.number)), n)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "dialplan"),
            ("Caller-Context", "public"),
            ("Caller-Destination-Number", "+49891234567"),
            ("Caller-Caller-ID-Number", "+4930999888"),
            ("Caller-Caller-ID-Name", "Anna"),
        ],
    )
    .await;
    let a = actions(&xml);
    assert!(
        has(&a, "set", "effective_caller_id_name=Support: Anna"),
        "{a:?}"
    );
    assert!(has(&a, "set", "talkops_direction=inbound"));
    assert!(a.iter().any(|(app, _)| app == "bridge"));

    // Members of a deleted extension disappear; deleting the group works.
    let (status, _) = f
        .admin
        .call("DELETE", &format!("/api/v1/ring-groups/{group_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn time_condition_routing(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let id = |e: &Value| e["id"].as_str().unwrap().to_owned();
    let all_day: Value = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
        .iter()
        .map(|d| (d.to_string(), json!([["00:00", "24:00"]])))
        .collect::<serde_json::Map<_, _>>()
        .into();
    let (status, _) = f
        .admin
        .post(
            "/api/v1/time-conditions",
            json!({"name": "Bad", "schedule": {"mon": [["17:00", "08:00"]]},
                   "open_type": "none", "closed_type": "none"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, tc) = f
        .admin
        .post(
            "/api/v1/time-conditions",
            json!({"number": "60", "name": "Office hours", "schedule": all_day,
                   "open_type": "extension", "open_id": id(&f.ext20),
                   "closed_type": "none"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{tc}");
    assert_eq!(tc["state"]["open"], true);
    let tc_id = id(&tc);

    // Open: rings extension 20.
    let a = internal_call(&router, &f.ext21, "60").await;
    assert!(has(&a, "set", "talkops_time_condition=open"), "{a:?}");
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");

    // *30<number> forces it closed (no closed destination: rejected) and back.
    let a = internal_call(&router, &f.ext21, "*3060").await;
    assert!(has(&a, "answer", ""), "{a:?}");
    let (_, tc) = f
        .admin
        .get(&format!("/api/v1/time-conditions/{tc_id}"))
        .await;
    assert_eq!(tc["override"], "closed");
    assert_eq!(tc["state"]["reason"], "override");
    let a = internal_call(&router, &f.ext21, "60").await;
    assert!(has(&a, "respond", "480 Temporarily Unavailable"), "{a:?}");
    internal_call(&router, &f.ext21, "*3060").await;
    let (_, tc) = f
        .admin
        .get(&format!("/api/v1/time-conditions/{tc_id}"))
        .await;
    assert_eq!(tc["override"], "auto");
    let a = internal_call(&router, &f.ext21, "*3099").await;
    assert!(has(&a, "respond", "404 Not Found"));

    // Operators may switch the override in the UI.
    let (status, tc) = f
        .admin
        .put(
            &format!("/api/v1/time-conditions/{tc_id}/override"),
            json!({"override": "closed"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tc["state"]["open"], false);

    // Holiday calendar.
    let (status, cal) = f.admin.get("/api/v1/holidays?region=DE-BY&year=2026").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cal["holidays"].as_array().unwrap().len(), 12);
    assert_eq!(cal["regions"].as_array().unwrap().len(), 17);
    let (status, _) = f.admin.get("/api/v1/holidays?region=XX").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn ivr_routing_and_transfers(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let id = |e: &Value| e["id"].as_str().unwrap().to_owned();

    let ext = |e: &Value| {
        json!({"type": "transfer", "id": "one",
        "destination_type": "extension", "destination_id": id(e)})
    };
    let (status, _) = f
        .admin
        .post(
            "/api/v1/attendants",
            json!({"name": "Bad", "flow": {"type": "menu", "id": "start", "clip_id": null,
                   "options": [{"digit": "12", "next": ext(&f.ext20)}]}}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, menu) = f
        .admin
        .post(
            "/api/v1/attendants",
            json!({"number": "70", "name": "Main", "flow": {
            "type": "menu", "id": "start", "clip_id": null,
            "options": [
                {"digit": "1", "next": ext(&f.ext20)},
                {"digit": "2", "next": {"type": "ring", "id": "ring",
                    "extensions": [id(&f.ext20)], "ring_secs": 15,
                    "next": {"type": "goto", "id": "back", "target": "start"}}},
                {"digit": "3", "next": {"type": "ring", "id": "ring-last",
                    "extensions": [id(&f.ext20)], "strategy": "sequential"}}
            ]}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{menu}");
    let (status, list) = f.admin.get("/api/v1/attendants").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["flow"]["options"][1]["next"]["ring_secs"], 15);

    // Calling the menu hands the call to TalkOps.
    let a = internal_call(&router, &f.ext21, "70").await;
    assert!(has(&a, "set", "talkops_app=ivr"), "{a:?}");
    assert!(has(&a, "set", &format!("talkops_ivr_id={}", id(&menu))));
    assert!(has(&a, "socket", "127.0.0.1:8084 async full"));

    // Choices come back through the transfer context.
    let transfer = |dest: String, tenant: bool| {
        let router = router.clone();
        async move {
            let mut form = vec![
                ("section", "dialplan".to_owned()),
                ("Caller-Context", "talkops".to_owned()),
                ("Caller-Destination-Number", dest),
                ("variable_talkops_caller_name", "Anna".to_owned()),
            ];
            if tenant {
                form.push((
                    "variable_talkops_tenant_id",
                    "00000000-0000-4000-8000-000000000001".to_owned(),
                ));
            }
            let form: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let (_, xml) = fs_post(&router, "/fs/xml", &form).await;
            actions(&xml)
        }
    };
    let a = transfer(format!("dest:extension:{}", id(&f.ext20)), true).await;
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");
    let a = transfer("dial:20".into(), true).await;
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");
    let a = transfer("dest:bogus:x".into(), true).await;
    assert!(has(&a, "respond", "404 Not Found"));
    let a = transfer("dial:20".into(), false).await;
    assert!(has(&a, "respond", "403 Forbidden"));

    // "Ring phones" steps ring through the dialplan and come back to the
    // attendant at the following step, or hang up after the last one.
    let a = transfer(format!("attendant:{}:ring", id(&menu)), true).await;
    assert!(has(&a, "set", "call_timeout=15"), "{a:?}");
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");
    assert!(has(&a, "set", "talkops_attendant_step=back"), "{a:?}");
    assert!(has(&a, "socket", "127.0.0.1:8084 async full"));
    let a = transfer(format!("attendant:{}:ring-last", id(&menu)), true).await;
    assert!(
        a.iter()
            .any(|(app, d)| app == "bridge" && d.contains("leg_timeout=30")),
        "{a:?}"
    );
    assert_eq!(a.last().unwrap().0, "hangup");
    let a = transfer(format!("attendant:{}:start", id(&menu)), true).await;
    assert!(has(&a, "respond", "404 Not Found"), "not a ring step");
    // Entering an attendant clears a step left from another one.
    let a = internal_call(&router, &f.ext21, "70").await;
    assert!(
        a.iter()
            .any(|(app, d)| app == "set" && d == "talkops_attendant_step="),
        "{a:?}"
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn parking_and_blind_transfers(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let tenant = "00000000-0000-4000-8000-000000000001";

    // Calls carry the tenant to bridged legs and route transfers privately.
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(
        has(&a, "export", &format!("talkops_tenant_id={tenant}")),
        "{a:?}"
    );
    assert!(has(&a, "export", "force_transfer_context=talkops"));

    // Park slots *51 … *59 (also dialed by BLF keys as park+*5N).
    for dest in ["*51", "park+*59"] {
        let a = internal_call(&router, &f.ext21, dest).await;
        let slot = dest.trim_start_matches("park+");
        assert!(has(&a, "valet_park", &format!("talkops {slot}")), "{a:?}");
    }
    let a = internal_call(&router, &f.ext21, "*50").await;
    assert!(!a.iter().any(|(app, _)| app == "valet_park"), "{a:?}");

    // A phone blind-transfers a caller: internal number, park slot, external.
    let transfer = |dest: &'static str| {
        let router = router.clone();
        async move {
            let (_, xml) = fs_post(
                &router,
                "/fs/xml",
                &[
                    ("section", "dialplan"),
                    ("Caller-Context", "talkops"),
                    ("Caller-Destination-Number", dest),
                    ("variable_talkops_tenant_id", tenant),
                ],
            )
            .await;
            actions(&xml)
        }
    };
    assert!(has(&transfer("20").await, "bridge", &ring20(&f)));
    assert!(has(&transfer("*52").await, "valet_park", "talkops *52"));
    let gw = format!(
        "sofia/gateway/gw-{}",
        f.number["account_id"].as_str().unwrap().replace('-', "")
    );
    let a = transfer("030123456").await;
    assert!(has(&a, "bridge", &format!("{gw}/030123456")), "{a:?}");
    assert!(has(&transfer("0").await, "respond", "404 Not Found"));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn queue_routing_and_callcenter_conf(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let id = |e: &Value| e["id"].as_str().unwrap().to_owned();
    let (status, _) = f
        .admin
        .post(
            "/api/v1/queues",
            json!({"name": "Bad", "strategy": "fastest", "members": [id(&f.ext20)]}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, q) = f
        .admin
        .post(
            "/api/v1/queues",
            json!({"number": "80", "name": "Support", "strategy": "ring-all",
                   "members": [id(&f.ext20), id(&f.ext21)],
                   "timeout_type": "extension", "timeout_id": id(&f.ext21)}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{q}");
    let cc_name = format!("q-{}", id(&q).replace('-', ""));

    let a = internal_call(&router, &f.ext21, "80").await;
    assert!(has(&a, "callcenter", &cc_name), "{a:?}");
    assert!(has(&a, "answer", ""));
    // Unanswered: overflow to extension 21 (no devices: rejected → hangup).
    assert!(has(&a, "hangup", ""), "{a:?}");

    // mod_callcenter fetches its configuration from TalkOps.
    let (status, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "configuration"),
            ("key_value", "callcenter.conf"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let queue = doc.descendants().find(|n| n.has_tag_name("queue")).unwrap();
    assert_eq!(queue.attribute("name"), Some(cc_name.as_str()));
    let agents: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("agent"))
        .collect();
    assert_eq!(agents.len(), 2);
    let agent20 = format!("a-{}", id(&f.ext20).replace('-', ""));
    let a20 = agents
        .iter()
        .find(|a| a.attribute("name") == Some(agent20.as_str()))
        .unwrap();
    assert_eq!(a20.attribute("status"), Some("Available"));
    assert_eq!(
        a20.attribute("contact"),
        Some("[leg_timeout=20]user/20-1@talkops.local,[leg_timeout=20]user/20-2@talkops.local")
    );
    // 21 has no devices and is on break.
    assert!(
        agents
            .iter()
            .any(|a| a.attribute("status") == Some("On Break"))
    );
    assert_eq!(
        doc.descendants().filter(|n| n.has_tag_name("tier")).count(),
        2
    );

    // Disabled queues send callers straight to the overflow.
    let mut input = q.clone();
    input["enabled"] = json!(false);
    let (status, _) = f
        .admin
        .put(&format!("/api/v1/queues/{}", id(&q)), input)
        .await;
    assert_eq!(status, StatusCode::OK);
    let a = internal_call(&router, &f.ext21, "80").await;
    assert!(!a.iter().any(|(app, _)| app == "callcenter"), "{a:?}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn queue_hours_greeting_music_and_voicemail(db: PgPool) {
    let state = state(db.clone());
    let sounds = state.media.sounds.clone();
    let router = talkops_api::app(state, None);
    let f = fixture(&router).await;
    let id = |e: &Value| e["id"].as_str().unwrap().to_owned();
    let tenant = talkops_core::tenant::TenantId::DEFAULT;
    let clip = |n: u128| {
        let db = db.clone();
        let sounds = sounds.clone();
        async move {
            let id = uuid::Uuid::from_u128(n);
            write_wav(&sounds.join(talkops_core::audio::clip_file(tenant, id)), 1);
            talkops_core::audio::create_file(&db, tenant, None, id, "upload", 1000)
                .await
                .unwrap();
            id
        }
    };
    let (greeting, music) = (clip(1).await, clip(2).await);
    let (status, hours) = f
        .admin
        .post(
            "/api/v1/time-conditions",
            json!({"name": "Hours", "override": "closed",
                   "open_type": "none", "closed_type": "none"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{hours}");
    let (status, q) = f
        .admin
        .post(
            "/api/v1/queues",
            json!({"number": "81", "name": "Sales", "members": [id(&f.ext20)],
                   "greeting_clip_id": greeting, "moh_clip_id": music, "max_callers": 3,
                   "time_condition_id": id(&hours),
                   "closed_type": "extension", "closed_id": id(&f.ext20),
                   "voicemail_recipients": [id(&f.ext20), id(&f.ext21)]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{q}");
    assert_eq!(q["max_callers"], 3);
    assert_eq!(q["voicemail_recipients"].as_array().unwrap().len(), 2);
    assert!(q.get("tenant_id").is_none());

    // Closed: straight to the after-hours destination.
    let a = internal_call(&router, &f.ext21, "81").await;
    assert!(has(&a, "set", "talkops_queue_closed=true"), "{a:?}");
    assert!(has(&a, "bridge", &ring20(&f)), "{a:?}");
    assert!(!a.iter().any(|(app, _)| app == "callcenter"));

    // Open: greeting, queue, then a message for the recipients.
    let (status, _) = f
        .admin
        .put(
            &format!("/api/v1/time-conditions/{}/override", id(&hours)),
            json!({"override": "open"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let a = internal_call(&router, &f.ext21, "81").await;
    let play = a
        .iter()
        .position(|(app, d)| app == "playback" && d.ends_with(&format!("{greeting}.wav")));
    let cc = a.iter().position(|(app, _)| app == "callcenter");
    assert!(play.is_some() && play < cc, "{a:?}");
    assert!(has(&a, "set", "talkops_app=queue_vm"), "{a:?}");
    assert!(a.last().is_some_and(|(app, _)| app == "socket"), "{a:?}");

    // Its own music on hold.
    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "configuration"),
            ("key_value", "callcenter.conf"),
        ],
    )
    .await;
    assert!(xml.contains(&format!("{music}.wav")), "{xml}");

    // Validation: unknown clip, a queue overflowing into itself.
    let mut bad = q.clone();
    bad["moh_clip_id"] = json!(uuid::Uuid::new_v4());
    let (status, _) = f
        .admin
        .put(&format!("/api/v1/queues/{}", id(&q)), bad)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let mut bad = q.clone();
    bad["overflow_type"] = json!("queue");
    bad["overflow_id"] = q["id"].clone();
    let (status, _) = f
        .admin
        .put(&format!("/api/v1/queues/{}", id(&q)), bad)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

fn write_wav(path: &std::path::Path, secs: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..secs * 8000 * 2 {
        w.write_sample(0i16).unwrap();
    }
    w.finalize().unwrap();
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn call_recording(db: PgPool) {
    let state = state(db.clone());
    let dir = state.media.recordings.clone();
    let router = talkops_api::app(state, None);
    let f = fixture(&router).await;
    let id = |e: &Value| e["id"].as_str().unwrap().to_owned();

    // Nothing is recorded by default.
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(
        !a.iter().any(|(_, d)| d.contains("record_session")),
        "{a:?}"
    );

    let (_, mut s) = f.admin.get("/api/v1/settings").await;
    s["record_internal"] = json!(true);
    s["transcription_enabled"] = json!(true);
    s["recording_retention_days"] = json!(30);
    let (status, s) = f.admin.put("/api/v1/settings", s).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let a = internal_call(&router, &f.ext21, "20").await;
    let rec = a
        .iter()
        .position(|(app, d)| app == "set" && d.starts_with("execute_on_answer=record_session "))
        .expect("recorded on answer");
    let bridge = a.iter().position(|(app, _)| app == "bridge").unwrap();
    assert!(rec < bridge, "{a:?}");
    assert!(has(&a, "set", "RECORD_STEREO=true"));
    let file = a
        .iter()
        .find_map(|(app, d)| {
            (app == "set")
                .then(|| d.strip_prefix("talkops_recording="))
                .flatten()
        })
        .unwrap()
        .to_owned();
    assert!(
        file.starts_with(TENANT) && file.ends_with("/${uuid}.wav"),
        "{file}"
    );
    assert_eq!(
        a[rec].1,
        format!(
            "execute_on_answer=record_session {}",
            dir.join(&file).display()
        )
    );
    // Unanswered calls go on to voicemail without recording it.
    assert_eq!(a[bridge + 1].0, "stop_record_session", "{a:?}");

    // An extension that is never recorded wins over the tenant setting.
    let mut e20 = f.ext20.clone();
    e20["record_calls"] = json!("never");
    let (status, _) = f
        .admin
        .put(&format!("/api/v1/extensions/{}", id(&e20)), e20.clone())
        .await;
    assert_eq!(status, StatusCode::OK);
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(
        !a.iter().any(|(_, d)| d.contains("record_session")),
        "{a:?}"
    );

    // FreeSWITCH wrote the file; the CDR registers it.
    let file = file.replace("${uuid}", "call-rec");
    write_wav(&dir.join(&file), 3);
    let xml = format!(
        r#"<?xml version="1.0"?><cdr><variables>
        <uuid>call-rec</uuid><talkops_direction>internal</talkops_direction>
        <talkops_tenant_id>{TENANT}</talkops_tenant_id><talkops_extension_id>{}</talkops_extension_id>
        <talkops_dest_extension_id>{}</talkops_dest_extension_id>
        <talkops_caller_number>21</talkops_caller_number><talkops_destination>20</talkops_destination>
        <talkops_recording>{}</talkops_recording>
        <start_epoch>1791150000</start_epoch><answer_epoch>1791150003</answer_epoch><end_epoch>1791150033</end_epoch>
        <duration>33</duration><billsec>30</billsec><hangup_cause>NORMAL_CLEARING</hangup_cause>
        </variables></cdr>"#,
        id(&f.ext21),
        id(&f.ext20),
        file.replace('/', "%2F")
    );
    let (status, _) = fs_post(&router, "/fs/cdr", &[("cdr", &xml)]).await;
    assert_eq!(status, StatusCode::OK);
    let (_, calls) = f.admin.get("/api/v1/calls").await;
    let rec_id = calls[0]["recording_id"]
        .as_str()
        .expect("linked")
        .to_owned();
    let (status, r) = f.admin.get(&format!("/api/v1/recordings/{rec_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["duration_secs"], 3);
    assert_eq!(r["transcript_status"], "pending");
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'transcribe'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(jobs, 1);
    // A path outside the volume is ignored.
    let evil = xml
        .replace("call-rec", "call-evil")
        .replace(&file.replace('/', "%2F"), "..%2F..%2Fetc%2Fpasswd.wav");
    fs_post(&router, "/fs/cdr", &[("cdr", &evil)]).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM recordings")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Access: the owner of an involved extension, admins; nobody else.
    for (name, ext) in [("bob", Some(&f.ext21)), ("carol", None)] {
        let (_, u) = f.admin.post("/api/v1/users", json!({"username": name, "display_name": name, "role": "operator", "password": "a-long-password-1"})).await;
        if let Some(ext) = ext {
            let mut e = ext.clone();
            e["user_id"] = u["id"].clone();
            f.admin
                .put(&format!("/api/v1/extensions/{}", id(&e)), e)
                .await;
        }
    }
    let bob = login(&router, "bob", "a-long-password-1").await.unwrap();
    let carol = login(&router, "carol", "a-long-password-1").await.unwrap();
    let audio = format!("/api/v1/recordings/{rec_id}/audio");
    let res = raw(
        &router,
        axum::http::Request::get(&audio)
            .header(axum::http::header::COOKIE, &bob.cookie)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[axum::http::header::CONTENT_TYPE], "audio/wav");
    assert_eq!(carol.get(&audio).await.0, StatusCode::NOT_FOUND);
    assert_eq!(f.admin.get(&audio).await.0, StatusCode::OK);
    let (_, log) = f.admin.get("/api/v1/audit").await;
    assert!(
        log.as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "listen" && e["entity_type"] == "recording"),
        "{log}"
    );

    // Transcript and search.
    let transcript = format!("/api/v1/recordings/{rec_id}/transcript");
    assert_eq!(bob.get(&transcript).await.0, StatusCode::NOT_FOUND);
    use talkops_core::recordings::{self, Segment, Source};
    let seg = |speaker: &str, text: &str| Segment {
        start: 0.0,
        end: 1.0,
        speaker: speaker.into(),
        text: text.into(),
    };
    recordings::save_transcript(
        &db,
        talkops_core::tenant::TenantId::DEFAULT,
        Source::Recording(rec_id.parse().unwrap()),
        "de",
        &[
            seg("caller", " Hallo, wegen der Rechnung 4711."),
            seg("called", " Die Rechnung ist bezahlt."),
        ],
    )
    .await
    .unwrap();
    let (status, t) = bob.get(&transcript).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        t["text"],
        "Hallo, wegen der Rechnung 4711. Die Rechnung ist bezahlt."
    );
    assert_eq!(t["segments"][1]["speaker"], "called");
    let (_, r) = bob.get(&format!("/api/v1/recordings/{rec_id}")).await;
    assert_eq!(r["transcript_status"], "done");
    let (_, hits) = bob.get("/api/v1/search?q=rechnung%204711").await;
    assert_eq!(hits.as_array().unwrap().len(), 1, "{hits}");
    assert_eq!(hits[0]["recording_id"], rec_id.as_str());
    assert!(hits[0]["snippet"].as_str().unwrap().contains("[Rechnung]"));
    let (_, hits) = carol.get("/api/v1/search?q=rechnung").await;
    assert!(hits.as_array().unwrap().is_empty());
    let (_, hits) = f.admin.get("/api/v1/search?q=bezahlt%20-rechnung").await;
    assert!(hits.as_array().unwrap().is_empty());

    // Retention removes old recordings with file and transcript.
    sqlx::query("UPDATE recordings SET created_at = now() - interval '31 days'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(talkops_api::retention::purge(&db, &dir).await.unwrap(), 1);
    assert!(!dir.join(&file).exists());
    let (status, _) = f.admin.get(&format!("/api/v1/recordings/{rec_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, calls) = f.admin.get("/api/v1/calls").await;
    assert!(calls[0]["recording_id"].is_null());

    // Only admins delete.
    assert_eq!(
        bob.call("DELETE", &format!("/api/v1/recordings/{rec_id}"), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn hold_music_selection(db: PgPool) {
    let state = state(db.clone());
    let sounds = state.media.sounds.clone();
    let router = talkops_api::app(state, None);
    let f = fixture(&router).await;
    let hold = |a: &[(String, String)]| {
        a.iter()
            .find_map(|(app, d)| {
                (app == "export")
                    .then(|| d.strip_prefix("hold_music="))
                    .flatten()
            })
            .map(str::to_owned)
    };
    // Default: all built-in pieces shuffled.
    let a = internal_call(&router, &f.ext21, "20").await;
    assert_eq!(hold(&a).as_deref(), Some("local_stream://default"));

    // The list of built-in pieces; a chosen piece once FreeSWITCH copied it.
    let (_, music) = f.admin.get("/api/v1/audio/music").await;
    assert_eq!(music.as_array().unwrap().len(), 4);
    assert_eq!(music[0]["available"], false);
    let track = "ponce-preludio-in-e-major";
    write_wav(&sounds.join(format!("music/{track}.wav")), 1);
    let (status, _) = f.admin.get(&format!("/api/v1/audio/music/{track}")).await;
    assert_eq!(status, StatusCode::OK);
    let (_, mut s) = f.admin.get("/api/v1/settings").await;
    s["hold_music"] = json!(track);
    let (status, _) = f.admin.put("/api/v1/settings", s.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let a = internal_call(&router, &f.ext21, "20").await;
    let path = sounds.join(format!("music/{track}.wav"));
    assert_eq!(hold(&a).as_deref(), path.to_str());

    // An own clip wins; unknown pieces and clips are rejected.
    let tenant = talkops_core::tenant::TenantId::DEFAULT;
    let clip = uuid::Uuid::from_u128(7);
    write_wav(
        &sounds.join(talkops_core::audio::clip_file(tenant, clip)),
        1,
    );
    talkops_core::audio::create_file(&db, tenant, None, clip, "upload", 1000)
        .await
        .unwrap();
    s["hold_music_clip_id"] = json!(clip);
    assert_eq!(
        f.admin.put("/api/v1/settings", s.clone()).await.0,
        StatusCode::OK
    );
    let a = internal_call(&router, &f.ext21, "20").await;
    let clip_path = sounds.join(talkops_core::audio::clip_file(tenant, clip));
    assert_eq!(hold(&a).as_deref(), clip_path.to_str());
    let mut bad = s.clone();
    bad["hold_music"] = json!("../../etc/passwd");
    assert_eq!(
        f.admin.put("/api/v1/settings", bad).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut bad = s;
    bad["hold_music_clip_id"] = json!(uuid::Uuid::from_u128(8));
    assert_ne!(f.admin.put("/api/v1/settings", bad).await.0, StatusCode::OK);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn conference_legs_offer_real_codecs(db: PgPool) {
    let router = router(db);
    let f = fixture(&router).await;
    let ext_id = f.ext21["id"].as_str().unwrap();
    let (status, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "dialplan"),
            ("Caller-Context", "internal"),
            ("Caller-Destination-Number", "20"),
            ("Caller-Caller-ID-Number", "21"),
            ("variable_talkops_tenant_id", TENANT),
            ("variable_talkops_extension_id", ext_id),
            ("variable_talkops_conference", "talkops-abc"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let a = actions(&xml);
    assert!(
        has(
            &a,
            "export",
            "nolocal:absolute_codec_string=OPUS,G722,PCMA,PCMU"
        ),
        "{a:?}"
    );
    assert!(has(&a, "bridge", &ring20(&f)));
    // Normal calls negotiate late, without a fixed list.
    let a = internal_call(&router, &f.ext21, "20").await;
    assert!(
        !a.iter()
            .any(|(_, d)| d.starts_with("absolute_codec_string"))
    );
}
