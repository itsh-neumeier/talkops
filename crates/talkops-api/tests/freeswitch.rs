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

    let (status, _) = f
        .admin
        .post(
            "/api/v1/ivr-menus",
            json!({"name": "Bad", "greeting_text": "x",
                   "options": [{"digit": "12", "type": "extension", "id": id(&f.ext20)}]}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, menu) = f
        .admin
        .post(
            "/api/v1/ivr-menus",
            json!({"number": "70", "name": "Main", "greeting_text": "Willkommen bei Beispiel.",
                   "options": [{"digit": "1", "type": "extension", "id": id(&f.ext20)}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{menu}");
    assert_eq!(menu["greeting_status"], "pending");

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

    // WAV upload replaces the TTS greeting.
    let mut wav = Vec::new();
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::new(std::io::Cursor::new(&mut wav), spec).unwrap();
        for _ in 0..800 {
            w.write_sample(0i16).unwrap();
        }
        w.finalize().unwrap();
    }
    let upload = |data: Vec<u8>| {
        let router = router.clone();
        let admin = (f.admin.cookie.clone(), f.admin.csrf.clone());
        let path = format!("/api/v1/ivr-menus/{}/greeting", id(&menu));
        async move {
            let mut body =
                b"--b\r\nContent-Disposition: form-data; name=\"file\"; filename=\"g.wav\"\r\n\r\n"
                    .to_vec();
            body.extend(data);
            body.extend(b"\r\n--b--\r\n");
            let res = raw(
                &router,
                axum::http::Request::post(path)
                    .header("cookie", admin.0)
                    .header("x-requested-with", "TalkOps")
                    .header("x-csrf-token", admin.1)
                    .header("content-type", "multipart/form-data; boundary=b")
                    .body(axum::body::Body::from(body))
                    .unwrap(),
            )
            .await;
            let status = res.status();
            (status, body_json(res).await)
        }
    };
    let (status, _) = upload(b"not a wav".to_vec()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, m) = upload(wav).await;
    assert_eq!(status, StatusCode::OK, "{m}");
    assert_eq!(m["greeting"], "upload");
    assert_eq!(m["greeting_status"], "ready");
    let (status, _) = f
        .admin
        .get(&format!("/api/v1/ivr-menus/{}/greeting", id(&menu)))
        .await;
    assert_eq!(status, StatusCode::OK);
}
