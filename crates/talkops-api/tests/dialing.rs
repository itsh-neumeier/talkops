//! Outbound dialing permissions (ADR 0045): seeded profiles, the API and
//! the enforcement on every path that leads out of TalkOps.

mod common;

use std::path::Path;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::dial_permissions::{self, PREMIUM_RULES};
use talkops_core::geo::GeoIndex;
use talkops_core::tenant::TenantId;
use uuid::Uuid;

const TENANT: &str = "00000000-0000-4000-8000-000000000001";
const T: TenantId = TenantId::DEFAULT;

/// The application with the real geolocation data (`presets/geo`).
fn geo_router(db: PgPool) -> axum::Router {
    let geo = GeoIndex::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/geo"))
        .unwrap();
    talkops_api::app(state(db).with_geo(geo), None)
}

struct Fixture {
    admin: Client,
    ext20: Value,
    ext21: Value,
    gw: String,
}

/// Admin, extension 200 (two devices) and 210, a LEONET line +49891234567
/// routed to 200, area code 89 and the default number.
async fn fixture(router: &axum::Router) -> Fixture {
    let admin = setup_admin(router).await;
    let (_, ext20) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "200", "display_name": "Office"}),
        )
        .await;
    let (_, ext21) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "210", "display_name": "Lab"}),
        )
        .await;
    for user in ["200-1", "200-2"] {
        admin
            .post(
                &format!("/api/v1/extensions/{}/devices", id(&ext20)),
                json!({"name": user, "kind": "desk", "sip_username": user}),
            )
            .await;
    }
    let (_, trunk) = admin
        .post(
            "/api/v1/trunks",
            json!({"name": "LEONET", "preset": "leonet"}),
        )
        .await;
    let (_, line) = admin
        .post(
            &format!("/api/v1/trunks/{}/lines", id(&trunk)),
            json!({"e164": "+49891234567", "password": "leo-secret", "destination_extension_id": ext20["id"]}),
        )
        .await;
    let (_, mut s) = admin.get("/api/v1/settings").await;
    s["area_code"] = json!("89");
    s["default_number_id"] = line["number"]["id"].clone();
    s["sip_uri_dialing"] = json!(true);
    admin.put("/api/v1/settings", s).await;
    let gw = format!(
        "sofia/gateway/gw-{}",
        line["number"]["account_id"]
            .as_str()
            .unwrap()
            .replace('-', "")
    );
    Fixture {
        admin,
        ext20,
        ext21,
        gw,
    }
}

fn id(v: &Value) -> &str {
    v["id"].as_str().unwrap()
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

/// The call leaves through the trunk.
fn goes_out(a: &[(String, String)], gw: &str) -> bool {
    a.iter()
        .any(|(app, data)| app == "bridge" && data.starts_with(gw))
}

fn blocked(a: &[(String, String)], reason: &str) -> bool {
    has(a, "respond", "403 Forbidden")
        && has(a, "set", "talkops_direction=outbound")
        && has(a, "set", &format!("talkops_blocked={reason}"))
}

async fn dialplan(router: &axum::Router, params: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut form = vec![("section", "dialplan")];
    form.extend_from_slice(params);
    let (status, xml) = fs_post(router, "/fs/xml", &form).await;
    assert_eq!(status, StatusCode::OK);
    actions(&xml)
}

/// A call from a device of `ext`, with more channel variables.
async fn call_with(
    router: &axum::Router,
    ext: &Value,
    dest: &str,
    vars: &[(&str, &str)],
) -> Vec<(String, String)> {
    let mut params = vec![
        ("Caller-Context", "internal"),
        ("Caller-Destination-Number", dest),
        ("variable_talkops_tenant_id", TENANT),
        ("variable_talkops_extension_id", id(ext)),
    ];
    params.extend_from_slice(vars);
    dialplan(router, &params).await
}

async fn call(router: &axum::Router, ext: &Value, dest: &str) -> Vec<(String, String)> {
    call_with(router, ext, dest, &[]).await
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn emergency_alert_storage_failure_does_not_block_the_call(db: PgPool) {
    let router = geo_router(db.clone());
    let f = fixture(&router).await;
    // Simulate a broken alert insert without changing routing tables.
    sqlx::query("DROP INDEX notification_emergency_call_once")
        .execute(&db)
        .await
        .unwrap();
    let uuid = Uuid::new_v4().to_string();
    let a = call_with(&router, &f.ext20, "112", &[("variable_uuid", &uuid)]).await;
    assert!(
        goes_out(&a, &f.gw),
        "notification storage must not reject emergency routing: {a:?}"
    );
}

/// A blind transfer (SIP REFER) of a call to `dest`.
async fn transfer(
    router: &axum::Router,
    dest: &str,
    vars: &[(&str, &str)],
) -> Vec<(String, String)> {
    let mut params = vec![
        ("Caller-Context", "talkops"),
        ("Caller-Destination-Number", dest),
        ("variable_talkops_tenant_id", TENANT),
    ];
    params.extend_from_slice(vars);
    dialplan(router, &params).await
}

async fn inbound(router: &axum::Router) -> Vec<(String, String)> {
    dialplan(
        router,
        &[
            ("Caller-Context", "public"),
            ("Caller-Destination-Number", "+49891234567"),
            ("Caller-Caller-ID-Number", "+4930999888"),
        ],
    )
    .await
}

async fn profile_id(admin: &Client, name: &str) -> String {
    let (_, list) = admin.get("/api/v1/dialing-profiles").await;
    list.as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .map(|p| id(p).to_owned())
        .unwrap_or_else(|| panic!("profile {name}: {list}"))
}

async fn assign(admin: &Client, ext: &Value, profile: Option<&str>) {
    let (status, body) = admin
        .put(
            &format!("/api/v1/extensions/{}/dialing-profile", id(ext)),
            json!({ "profile_id": profile }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn new_installations_block_premium_rate_numbers(db: PgPool) {
    let profiles = dial_permissions::list(&db, T).await.unwrap();
    let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Internal only",
            "International",
            "National",
            "National + Europe",
            "Unrestricted"
        ]
    );
    let default: Vec<&str> = profiles
        .iter()
        .filter(|p| p.is_default)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(default, ["International"], "a new installation");
    let get = |name: &str| profiles.iter().find(|p| p.name == name).unwrap();

    // The seeded list is the list in the code.
    let mut seeded: Vec<&str> = get("International")
        .rules
        .iter()
        .map(|r| r.value.as_str())
        .collect();
    let mut listed: Vec<&str> = PREMIUM_RULES.iter().map(|r| r.prefix).collect();
    seeded.sort_unstable();
    listed.sort_unstable();
    assert_eq!(seeded, listed);

    let internal = get("Internal only");
    assert!(internal.rules.is_empty() && !internal.sip_addresses);
    assert!(
        get("National")
            .rules
            .iter()
            .any(|r| r.value == "+49" && r.action == dial_permissions::RuleAction::Allow)
    );
    assert_eq!(
        get("National + Europe")
            .rules
            .iter()
            .filter(|r| r.kind == dial_permissions::RuleKind::Country)
            .count(),
        31
    );
    assert!(get("Unrestricted").rules.is_empty());
}

#[sqlx::test(migrations = false)]
async fn updates_keep_existing_installations_unrestricted(db: PgPool) {
    // All migrations before this feature, from a copy of the directory.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let before = std::env::temp_dir().join(format!("talkops-migrations-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&before).unwrap();
    for entry in std::fs::read_dir(&source).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if name.as_str() < "20261110000001" {
            std::fs::copy(&path, before.join(&name)).unwrap();
        }
    }
    sqlx::migrate::Migrator::new(before.as_path())
        .await
        .unwrap()
        .run(&db)
        .await
        .unwrap();
    // An installation in use: one extension.
    sqlx::query(
        "INSERT INTO extensions (tenant_id, number, display_name) VALUES ($1, '200', 'Office')",
    )
    .bind(T)
    .execute(&db)
    .await
    .unwrap();
    talkops_core::db::MIGRATOR.run(&db).await.unwrap();
    std::fs::remove_dir_all(&before).unwrap();

    let profiles = dial_permissions::list(&db, T).await.unwrap();
    assert_eq!(profiles.len(), 5);
    let default = dial_permissions::effective(&db, T, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(default.name, "Unrestricted");
    assert!(default.rules.is_empty());
    let geo = GeoIndex::default();
    for number in ["+499001234567", "+881612345678", "+4930123456"] {
        assert!(
            dial_permissions::check(Some(&default), number, &geo).allowed,
            "{number}"
        );
    }
    let ext: Option<Uuid> = sqlx::query_scalar("SELECT dialing_profile_id FROM extensions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(ext, None, "extensions use the default profile");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn emergency_calls_are_never_blocked(db: PgPool) {
    let router = geo_router(db);
    let f = fixture(&router).await;
    let internal = profile_id(&f.admin, "Internal only").await;
    assert_eq!(
        f.admin
            .put(
                &format!("/api/v1/dialing-profiles/{internal}/default"),
                json!({})
            )
            .await
            .0,
        StatusCode::OK
    );
    // A profile that blocks everything, also with explicit rules for the
    // emergency numbers in every form.
    let (status, all) = f
        .admin
        .post(
            "/api/v1/dialing-profiles",
            json!({"name": "Nothing", "base_mode": "deny", "sip_addresses": false, "rules": [
                {"kind": "prefix", "value": "+", "action": "deny"},
                {"kind": "prefix", "value": "+49112", "action": "deny"},
                {"kind": "prefix", "value": "+49110", "action": "deny"},
                {"kind": "country", "value": "DE", "action": "deny"}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{all}");
    let (status, all) = f
        .admin
        .post(
            "/api/v1/dialing-profiles",
            json!({"name": "Nothing", "base_mode": "deny", "sip_addresses": false, "rules": [
                {"kind": "prefix", "value": "+4", "action": "deny"},
                {"kind": "prefix", "value": "+49112", "action": "deny"},
                {"kind": "prefix", "value": "+49110", "action": "deny"},
                {"kind": "country", "value": "DE", "action": "deny"}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{all}");
    assign(&f.admin, &f.ext20, Some(id(&all))).await;
    for ext in [&f.ext20, &f.ext21] {
        for dest in ["112", "110"] {
            let a = call(&router, ext, dest).await;
            assert!(
                has(&a, "bridge", &format!("{}/{dest}", f.gw)),
                "{dest}: {a:?}"
            );
            assert!(!a.iter().any(|(_, d)| d.starts_with("talkops_blocked=")));
        }
        // Normal numbers are blocked for both.
        assert!(blocked(
            &call(&router, ext, "030123456").await,
            if ext == &f.ext20 {
                // A prefix rule wins over a country rule.
                "dialing:prefix:+4"
            } else {
                "dialing:base"
            }
        ));
    }
    // The test endpoint agrees.
    let (_, r) = f
        .admin
        .post(
            "/api/v1/dialing-profiles/test",
            json!({"number": "112", "profile_id": id(&all)}),
        )
        .await;
    assert_eq!(r["class"], "emergency");
    assert_eq!(r["allowed"], true);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn direct_dialing_follows_the_profile(db: PgPool) {
    let router = geo_router(db);
    let f = fixture(&router).await;
    let gw = &f.gw;

    // Default profile of a new installation: International.
    let a = call(&router, &f.ext20, "030123456").await;
    assert!(goes_out(&a, gw), "{a:?}");
    let a = call(&router, &f.ext20, "0900 1234567").await;
    assert!(blocked(&a, "dialing:prefix:+49900"), "{a:?}");
    assert!(has(&a, "set", "talkops_destination=+499001234567"), "{a:?}");
    assert!(!a.iter().any(|(app, _)| app == "bridge"));
    // Directory enquiries (service numbers) are checked in national form.
    let a = call(&router, &f.ext20, "11833").await;
    assert!(blocked(&a, "dialing:prefix:+49118"), "{a:?}");
    assert!(has(&a, "set", "talkops_destination=11833"), "{a:?}");
    // International: 00 and + dialing.
    assert!(goes_out(
        &call(&router, &f.ext20, "0033123456789").await,
        gw
    ));
    assert!(blocked(
        &call(&router, &f.ext20, "+881612345678").await,
        "dialing:prefix:+881"
    ));

    // National: own country only, premium blocked.
    let national = profile_id(&f.admin, "National").await;
    assign(&f.admin, &f.ext20, Some(&national)).await;
    assert!(goes_out(&call(&router, &f.ext20, "030123456").await, gw));
    // A local number with the area code.
    assert!(goes_out(&call(&router, &f.ext20, "1234567").await, gw));
    assert!(blocked(
        &call(&router, &f.ext20, "0043123456789").await,
        "dialing:base"
    ));
    assert!(blocked(
        &call(&router, &f.ext20, "09001234567").await,
        "dialing:prefix:+49900"
    ));
    // The other extension keeps the default profile.
    assert!(goes_out(
        &call(&router, &f.ext21, "0043123456789").await,
        gw
    ));

    // National + Europe: countries from the geolocation data.
    let europe = profile_id(&f.admin, "National + Europe").await;
    assign(&f.admin, &f.ext20, Some(&europe)).await;
    assert!(goes_out(
        &call(&router, &f.ext20, "0043123456789").await,
        gw
    ));
    assert!(blocked(
        &call(&router, &f.ext20, "001212555123").await,
        "dialing:base"
    ));
    assert!(blocked(
        &call(&router, &f.ext20, "0043900123456").await,
        "dialing:prefix:+43900"
    ));

    // A single number wins over a prefix and a country.
    let (status, own) = f
        .admin
        .post(
            "/api/v1/dialing-profiles",
            json!({"name": "Own", "base_mode": "allow", "rules": [
                {"kind": "country", "value": "AT", "action": "deny", "label": "Austria"},
                {"kind": "prefix", "value": "+49900*", "action": "deny"},
                {"kind": "number", "value": "+49 900 1234567", "action": "allow", "label": "Hotline"}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{own}");
    assign(&f.admin, &f.ext20, Some(id(&own))).await;
    assert!(goes_out(&call(&router, &f.ext20, "09001234567").await, gw));
    assert!(blocked(
        &call(&router, &f.ext20, "09001234568").await,
        "dialing:prefix:+49900"
    ));
    assert!(blocked(
        &call(&router, &f.ext20, "0043123456789").await,
        "dialing:country:AT"
    ));

    // Back to the default profile.
    assign(&f.admin, &f.ext20, None).await;
    assert!(goes_out(
        &call(&router, &f.ext20, "0043123456789").await,
        gw
    ));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn sip_addresses_and_softphone_conferences(db: PgPool) {
    let router = geo_router(db);
    let f = fixture(&router).await;
    let uri = |ext: Value| {
        let router = router.clone();
        async move {
            call_with(
                &router,
                &ext,
                "test.echo@sip5060.net",
                &[("variable_sip_req_host", "talkops.local")],
            )
            .await
        }
    };
    let a = uri(f.ext20.clone()).await;
    assert!(
        has(&a, "bridge", "sofia/external/sip:test.echo@sip5060.net"),
        "{a:?}"
    );
    let internal = profile_id(&f.admin, "Internal only").await;
    assign(&f.admin, &f.ext20, Some(&internal)).await;
    let a = uri(f.ext20.clone()).await;
    assert!(blocked(&a, "dialing:sip"), "{a:?}");
    assert!(!a.iter().any(|(app, _)| app == "bridge"));

    // A softphone conference dials through a loopback leg into the
    // internal context as the softphone's extension.
    let conference = [("variable_talkops_conference", "talkops-abc")];
    let a = call_with(&router, &f.ext20, "030123456", &conference).await;
    assert!(blocked(&a, "dialing:base"), "{a:?}");
    let a = call_with(&router, &f.ext21, "030123456", &conference).await;
    assert!(goes_out(&a, &f.gw), "{a:?}");
    // Internal participants stay possible (210 has no devices).
    let a = call_with(&router, &f.ext20, "210", &conference).await;
    assert!(has(&a, "set", "talkops_direction=internal"), "{a:?}");
    assert!(!has(&a, "respond", "403 Forbidden"), "{a:?}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn call_forwarding_uses_the_forwarding_extension(db: PgPool) {
    let router = geo_router(db.clone());
    let f = fixture(&router).await;
    let national = profile_id(&f.admin, "National").await;
    assign(&f.admin, &f.ext20, Some(&national)).await;

    // *72 refuses a target the profile blocks.
    let a = call(&router, &f.ext20, "*720043123456789").await;
    assert!(has(&a, "respond", "403 Forbidden"), "{a:?}");
    let (_, e) = f
        .admin
        .get(&format!("/api/v1/extensions/{}", id(&f.ext20)))
        .await;
    assert_eq!(e["forward_all"], Value::Null);
    // Internal and allowed targets are accepted.
    let a = call(&router, &f.ext20, "*72030123456").await;
    assert!(a.iter().any(|(app, _)| app == "playback"), "{a:?}");
    let a = inbound(&router).await;
    assert!(has(&a, "bridge", &format!("{}/030123456", f.gw)), "{a:?}");

    // The web interface (call settings) refuses it too.
    let (status, body) = f
        .admin
        .put(
            &format!("/api/v1/extensions/{}/call-settings", id(&f.ext20)),
            json!({"dnd": false, "forward_all": "0043123456789"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, _) = f
        .admin
        .put(
            &format!("/api/v1/extensions/{}/call-settings", id(&f.ext20)),
            json!({"dnd": false, "forward_all": "210"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // A target set before the profile changed: the extension rings.
    sqlx::query("UPDATE extensions SET forward_all = '0043123456789' WHERE number = '200'")
        .execute(&db)
        .await
        .unwrap();
    for a in [inbound(&router).await, call(&router, &f.ext21, "200").await] {
        assert!(!goes_out(&a, &f.gw), "{a:?}");
        let bridge = a.iter().find(|(app, _)| app == "bridge").unwrap();
        assert!(bridge.1.contains("user/200-1@talkops.local"), "{a:?}");
        assert!(!has(&a, "set", "talkops_forwarded_from=200"));
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn blind_transfers_use_the_transferring_extension(db: PgPool) {
    let router = geo_router(db);
    let f = fixture(&router).await;
    let internal = profile_id(&f.admin, "Internal only").await;
    assign(&f.admin, &f.ext20, Some(&internal)).await;

    // Devices that answer mark the other leg; the caller marks the called
    // legs.
    let (_, xml) = fs_post(
        &router,
        "/fs/xml",
        &[
            ("section", "dialplan"),
            ("Caller-Context", "internal"),
            ("Caller-Destination-Number", "200"),
            ("variable_talkops_tenant_id", TENANT),
            ("variable_talkops_extension_id", id(&f.ext21)),
        ],
    )
    .await;
    let a = actions(&xml);
    let mark = format!(
        "[api_on_answer_talkops_xfer=uuid_setvar ${{uuid}} talkops_xfer_extension_id {}]user/200-1@talkops.local",
        id(&f.ext20)
    );
    assert!(
        a.iter()
            .any(|(app, d)| app == "bridge" && d.contains(&mark)),
        "{a:?}"
    );
    assert!(has(
        &a,
        "export",
        &format!("nolocal:talkops_xfer_extension_id={}", id(&f.ext21))
    ));

    // The restricted extension transfers: blocked, the call ends.
    let restricted = [("variable_talkops_xfer_extension_id", id(&f.ext20))];
    let a = transfer(&router, "030123456", &restricted).await;
    assert!(has(&a, "hangup", "CALL_REJECTED"), "{a:?}");
    assert!(!a.iter().any(|(app, _)| app == "bridge"));
    // Emergency numbers stay possible.
    let a = transfer(&router, "112", &restricted).await;
    assert!(has(&a, "bridge", &format!("{}/112", f.gw)), "{a:?}");
    // An extension with the default profile (International).
    let a = transfer(
        &router,
        "030123456",
        &[("variable_talkops_xfer_extension_id", id(&f.ext21))],
    )
    .await;
    assert!(goes_out(&a, &f.gw), "{a:?}");
    // A queue agent (mod_callcenter sets cc_agent).
    let agent = format!("a-{}", id(&f.ext20).replace('-', ""));
    let a = transfer(&router, "030123456", &[("variable_cc_agent", &agent)]).await;
    assert!(has(&a, "hangup", "CALL_REJECTED"), "{a:?}");
    // Without a mark: the tenant default profile.
    let a = transfer(&router, "030123456", &[]).await;
    assert!(goes_out(&a, &f.gw), "{a:?}");
    let a = transfer(&router, "09001234567", &[]).await;
    assert!(has(&a, "hangup", "CALL_REJECTED"), "{a:?}");
    // Internal targets are not affected.
    let a = transfer(&router, "210", &restricted).await;
    assert!(!has(&a, "hangup", "CALL_REJECTED"), "{a:?}");

    // Smart Attendant direct dialing reaches internal numbers only.
    let a = transfer(&router, "dial:09001234567", &[]).await;
    assert!(has(&a, "respond", "404 Not Found"), "{a:?}");
    assert!(!goes_out(&a, &f.gw));

    // A queue clears an earlier mark before the caller waits.
    let (status, q) = f
        .admin
        .post(
            "/api/v1/queues",
            json!({"name": "Support", "strategy": "ring-all", "members": [id(&f.ext21)]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{q}");
    let a = transfer(&router, &format!("dest:queue:{}", id(&q)), &restricted).await;
    let clear = a
        .iter()
        .position(|(app, d)| app == "set" && d == "talkops_xfer_extension_id=")
        .expect("mark cleared");
    let queue = a.iter().position(|(app, _)| app == "callcenter").unwrap();
    assert!(clear < queue, "{a:?}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn fax_uses_the_owners_extensions(db: PgPool) {
    let router = geo_router(db.clone());
    let f = fixture(&router).await;
    let geo = GeoIndex::default();
    let owner: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&db)
        .await
        .unwrap();
    // No own extension: the default profile (International).
    let blocks = |number: &'static str| {
        let db = db.clone();
        let geo = &geo;
        async move {
            dial_permissions::user_blocks(&db, T, owner, number, geo)
                .await
                .unwrap()
        }
    };
    assert_eq!(blocks("+4930123456").await, None);
    assert_eq!(
        blocks("+499001234567").await.as_deref(),
        Some("dialing:prefix:+49900")
    );
    // Two extensions: each must allow the number.
    for ext in [&f.ext20, &f.ext21] {
        let mut e = (*ext).clone();
        e["user_id"] = json!(owner);
        let (status, body) = f
            .admin
            .put(&format!("/api/v1/extensions/{}", id(ext)), e)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    assert_eq!(blocks("+4930123456").await, None);
    let internal = profile_id(&f.admin, "Internal only").await;
    assign(&f.admin, &f.ext21, Some(&internal)).await;
    assert_eq!(blocks("+4930123456").await.as_deref(), Some("dialing:base"));

    // Submitting a prepared fax checks the profiles.
    let trunk: Uuid = sqlx::query_scalar("SELECT id FROM trunks")
        .fetch_one(&db)
        .await
        .unwrap();
    let number: Uuid = sqlx::query_scalar("SELECT id FROM numbers")
        .fetch_one(&db)
        .await
        .unwrap();
    let s = state(db.clone());
    talkops_core::fax::save_settings(
        &db,
        T,
        &talkops_core::fax::Settings {
            enabled: true,
            global_number_id: Some(number),
            ..Default::default()
        },
        None,
        &s.secrets,
    )
    .await
    .unwrap();
    let fax: Uuid = sqlx::query_scalar(
        "INSERT INTO faxes (tenant_id, owner_id, direction, state, number_id, trunk_id, source, destination, pages)
         VALUES ($1, $2, 'outbound', 'draft', $3, $4, '+49891234567', '+4930123456', 1) RETURNING id",
    )
    .bind(T)
    .bind(owner)
    .bind(number)
    .bind(trunk)
    .fetch_one(&db)
    .await
    .unwrap();
    let (status, body) = f
        .admin
        .post(&format!("/api/v1/faxes/{fax}/submit"), json!({}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["message"], talkops_api::fax::DIALING_BLOCKED, "{body}");
    assign(&f.admin, &f.ext21, None).await;
    let (status, body) = f
        .admin
        .post(&format!("/api/v1/faxes/{fax}/submit"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn profile_api_permissions_and_audit(db: PgPool) {
    let router = geo_router(db.clone());
    let f = fixture(&router).await;

    // Validation.
    for rules in [
        json!([{"kind": "country", "value": "Germany", "action": "deny"}]),
        json!([{"kind": "prefix", "value": "0900", "action": "deny"}]),
        json!([{"kind": "number", "value": "+49${x}", "action": "deny"}]),
        json!([{"kind": "prefix", "value": "+49", "action": "deny"},
               {"kind": "prefix", "value": "+49*", "action": "allow"}]),
    ] {
        let (status, body) = f
            .admin
            .post(
                "/api/v1/dialing-profiles",
                json!({"name": "Bad", "base_mode": "allow", "rules": rules}),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let (status, _) = f
        .admin
        .post(
            "/api/v1/dialing-profiles",
            json!({"name": "national", "base_mode": "allow"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "names are unique");

    // Create, change, test, assign.
    let (status, p) = f
        .admin
        .post(
            "/api/v1/dialing-profiles",
            json!({"name": "Night", "description": "After hours", "base_mode": "deny",
                   "rules": [{"kind": "country", "value": "de", "action": "allow"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{p}");
    assert_eq!(p["rules"][0]["value"], "DE");
    assert_eq!(p["sip_addresses"], true);
    let pid = id(&p).to_owned();
    let (status, p) = f
        .admin
        .put(
            &format!("/api/v1/dialing-profiles/{pid}"),
            json!({"name": "Night", "base_mode": "deny", "sip_addresses": false,
                   "rules": [{"kind": "prefix", "value": "+4989", "action": "allow"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["rules"].as_array().unwrap().len(), 1);
    let (_, r) = f
        .admin
        .post(
            "/api/v1/dialing-profiles/test",
            json!({"number": "030 123456", "profile_id": pid}),
        )
        .await;
    assert_eq!(r["class"], "external");
    assert_eq!(r["number"], "+4930123456");
    assert_eq!(r["country"], "DE");
    assert_eq!(r["allowed"], false);
    assert_eq!(r["reason"], "dialing:base");
    assert_eq!(r["profile"]["name"], "Night");
    let (_, r) = f
        .admin
        .post(
            "/api/v1/dialing-profiles/test",
            json!({"number": "1234567", "profile_id": pid}),
        )
        .await;
    assert_eq!(r["allowed"], true);
    assert_eq!(r["rule"]["value"], "+4989");
    // With an extension: its profile, else the default profile.
    let (_, r) = f
        .admin
        .post(
            "/api/v1/dialing-profiles/test",
            json!({"number": "09001234567", "extension_id": id(&f.ext20)}),
        )
        .await;
    assert_eq!(r["profile"]["name"], "International");
    assert_eq!(r["reason"], "dialing:prefix:+49900");
    let (_, r) = f
        .admin
        .post("/api/v1/dialing-profiles/test", json!({"number": "abc"}))
        .await;
    assert_eq!(r["class"], "invalid");
    assert_eq!(r["allowed"], false);

    assign(&f.admin, &f.ext20, Some(&pid)).await;
    // Changing the extension does not reset its profile.
    let (_, e) = f
        .admin
        .get(&format!("/api/v1/extensions/{}", id(&f.ext20)))
        .await;
    assert_eq!(e["dialing_profile_id"], pid.as_str());
    let (status, e) = f
        .admin
        .put(
            &format!("/api/v1/extensions/{}", id(&f.ext20)),
            json!({"number": "200", "display_name": "Office 2"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{e}");
    assert_eq!(e["dialing_profile_id"], pid.as_str());
    // Unknown profile.
    let (status, _) = f
        .admin
        .put(
            &format!("/api/v1/extensions/{}/dialing-profile", id(&f.ext20)),
            json!({"profile_id": Uuid::new_v4()}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // In use and default profiles cannot be deleted.
    let (status, _) = f
        .admin
        .call("DELETE", &format!("/api/v1/dialing-profiles/{pid}"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let international = profile_id(&f.admin, "International").await;
    let (status, _) = f
        .admin
        .call(
            "DELETE",
            &format!("/api/v1/dialing-profiles/{international}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, p) = f
        .admin
        .put(
            &format!("/api/v1/dialing-profiles/{pid}/default"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["is_default"], true);
    let defaults: i64 =
        sqlx::query_scalar("SELECT count(*) FROM dialing_profiles WHERE is_default")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(defaults, 1);
    assign(&f.admin, &f.ext20, None).await;
    let (status, _) = f
        .admin
        .call(
            "DELETE",
            &format!("/api/v1/dialing-profiles/{international}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, premium) = f.admin.get("/api/v1/dialing-profiles/premium-rules").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(premium.as_array().unwrap().len(), PREMIUM_RULES.len());
    assert!(
        premium[0]["source"]
            .as_str()
            .unwrap()
            .contains("Bundesnetzagentur")
    );

    // Every change has an audit entry.
    let actions: Vec<(String, String)> = sqlx::query_as(
        "SELECT action, entity_type FROM audit_log
         WHERE entity_type IN ('dialing_profile', 'extension_dialing_profile') ORDER BY created_at, id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    for expected in [
        ("create", "dialing_profile"),
        ("update", "dialing_profile"),
        ("update", "extension_dialing_profile"),
        ("set_default", "dialing_profile"),
        ("delete", "dialing_profile"),
    ] {
        assert!(
            actions
                .iter()
                .any(|(a, e)| a == expected.0 && e == expected.1),
            "{expected:?} in {actions:?}"
        );
    }

    // Operators have no dialing.manage; extension viewers can read.
    let (_, _) = f
        .admin
        .post(
            "/api/v1/users",
            json!({"username": "op", "display_name": "Operator", "password": "correct-horse", "role": "operator"}),
        )
        .await;
    let (_, _) = f
        .admin
        .post(
            "/api/v1/users",
            json!({"username": "plain", "display_name": "User", "password": "correct-horse", "role": "user"}),
        )
        .await;
    let op = login(&router, "op", "correct-horse").await.unwrap();
    let (status, list) = op.get("/api/v1/dialing-profiles").await;
    assert_eq!(status, StatusCode::OK, "operators have extensions.view");
    assert!(list.as_array().unwrap().len() >= 4);
    for (status, _) in [
        op.post(
            "/api/v1/dialing-profiles",
            json!({"name": "X", "base_mode": "allow"}),
        )
        .await,
        op.put(
            &format!("/api/v1/dialing-profiles/{pid}"),
            json!({"name": "X", "base_mode": "allow"}),
        )
        .await,
        op.call("DELETE", &format!("/api/v1/dialing-profiles/{pid}"), None)
            .await,
        op.put(
            &format!("/api/v1/dialing-profiles/{pid}/default"),
            json!({}),
        )
        .await,
        op.post("/api/v1/dialing-profiles/test", json!({"number": "112"}))
            .await,
        op.put(
            &format!("/api/v1/extensions/{}/dialing-profile", id(&f.ext20)),
            json!({"profile_id": null}),
        )
        .await,
    ] {
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let plain = login(&router, "plain", "correct-horse").await.unwrap();
    assert_eq!(
        plain.get("/api/v1/dialing-profiles").await.0,
        StatusCode::FORBIDDEN
    );
    let (_, catalog) = f.admin.get("/api/v1/roles").await;
    assert!(catalog.to_string().contains("dialing.manage"), "{catalog}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn blocked_outbound_calls_keep_their_reason(db: PgPool) {
    let insert = |direction: &'static str, reason: &'static str| {
        let db = db.clone();
        async move {
            sqlx::query(
                "INSERT INTO cdr (tenant_id, call_uuid, direction, caller_number, destination,
                                  started_at, ended_at, hangup_cause, blocked_reason)
                 VALUES ($1, $2, $3::call_direction, '200', '+499001234567', now(), now(), 'CALL_REJECTED', $4)",
            )
            .bind(T)
            .bind(Uuid::new_v4().to_string())
            .bind(direction)
            .bind(reason)
            .execute(&db)
            .await
        }
    };
    for reason in [
        "dialing:base",
        "dialing:sip",
        "dialing:country:AT",
        "dialing:prefix:+49900",
        "dialing:number:+4930123456",
    ] {
        insert("outbound", reason).await.unwrap();
        assert!(insert("inbound", reason).await.is_err(), "{reason}");
    }
    assert!(insert("outbound", "phoneblock:12").await.is_err());
    assert!(insert("outbound", "dialing:prefix:+49900*").await.is_err());
    // The call list filter finds them.
    let blocked = talkops_core::cdr::list(
        &db,
        T,
        &talkops_core::cdr::CdrQuery {
            result: Some("blocked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(blocked.len(), 5);
}
