//! Call origin map (`GET /api/v1/calls/map`) and the area filter of the call log.
mod common;

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::cdr::{self, Cdr, Direction};
use talkops_core::geo::GeoIndex;
use talkops_core::tenant::TenantId;
use uuid::Uuid;

const GEO: &str = "# title: test\n\
    A\tDE\tDE\tcountry\t51\t10\t49\tGermany\n\
    P\t49\tDE\n\
    A\tDE-951\tDE\tarea\t49.89\t10.89\t49 951\tBamberg\n\
    P\t49951\tDE-951\n\
    A\tDE-mobile\tDE\tmobile\t51\t10\t49\tGermany\n\
    P\t4917\tDE-mobile\n\
    A\tAT\tAT\tcountry\t47.5\t14\t43\tAustria\n\
    P\t43\tAT\n\
    A\tAT-1\tAT\tarea\t48.2\t16.37\t43 1\tWien\n\
    P\t431\tAT-1\n";

struct Call<'a> {
    uuid: &'a str,
    direction: Direction,
    caller: &'a str,
    destination: &'a str,
    extension: Uuid,
    minutes_ago: i64,
    answered: bool,
    billsec: i32,
    blocked: Option<&'a str>,
}

async fn insert(db: &PgPool, c: Call<'_>) {
    let started = Utc::now() - Duration::minutes(c.minutes_ago);
    let record = Cdr {
        id: Uuid::nil(),
        call_uuid: c.uuid.into(),
        direction: c.direction,
        caller_number: c.caller.into(),
        caller_name: String::new(),
        destination: c.destination.into(),
        extension_id: Some(c.extension),
        dest_extension_id: None,
        trunk_id: None,
        number_id: None,
        started_at: started,
        answered_at: c.answered.then_some(started),
        ended_at: started + Duration::seconds(i64::from(c.billsec)),
        duration_secs: c.billsec,
        billsec: c.billsec,
        hangup_cause: if c.answered {
            "NORMAL_CLEARING".into()
        } else {
            "NO_ANSWER".into()
        },
        blocked_reason: c.blocked.map(Into::into),
        recording_id: None,
    };
    assert!(cdr::insert(db, TenantId::DEFAULT, &record).await.unwrap());
}

fn area<'a>(map: &'a Value, id: &str) -> Option<&'a Value> {
    map["areas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
}

fn ids(map: &Value) -> Vec<String> {
    let mut out: Vec<String> = map["areas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect();
    out.sort();
    out
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn counts_external_calls_by_area_category_range_and_scope(db: PgPool) {
    let state = state(db.clone()).with_geo(GeoIndex::from_files([("test.tsv", GEO)]).unwrap());
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;
    let mut ext = Vec::new();
    for number in ["101", "102"] {
        let (_, e) = admin
            .post(
                "/api/v1/extensions",
                json!({"number": number, "display_name": format!("Ext {number}")}),
            )
            .await;
        ext.push(Uuid::parse_str(e["id"].as_str().unwrap()).unwrap());
    }
    let (own, other) = (ext[0], ext[1]);
    admin
        .post(
            "/api/v1/contacts",
            json!({"name": "Bäckerei Bamberg", "phone_work": "095112345"}),
        )
        .await;
    let base = Call {
        uuid: "",
        direction: Direction::Inbound,
        caller: "",
        destination: "101",
        extension: own,
        minutes_ago: 0,
        answered: false,
        billsec: 0,
        blocked: None,
    };
    for call in [
        // Bamberg, answered, last hour; national format of the same area.
        Call {
            uuid: "bamberg-answered",
            caller: "+4995112345",
            minutes_ago: 10,
            answered: true,
            billsec: 60,
            ..base
        },
        Call {
            uuid: "bamberg-missed",
            caller: "095154321",
            extension: other,
            minutes_ago: 61,
            ..base
        },
        // Vienna, outbound, three days ago.
        Call {
            uuid: "vienna-outbound",
            direction: Direction::Outbound,
            caller: "101",
            destination: "+431234567",
            minutes_ago: 3 * 24 * 60,
            answered: true,
            billsec: 30,
            ..base
        },
        // Mobile, blocked by PhoneBlock.
        Call {
            uuid: "mobile-blocked",
            caller: "+491711234567",
            minutes_ago: 20,
            blocked: Some("phoneblock:3"),
            ..base
        },
        // Anonymous, blocked by the anonymous rule.
        Call {
            uuid: "anonymous",
            caller: "anonymous",
            minutes_ago: 25,
            blocked: Some("anonymous"),
            ..base
        },
        // Unknown country.
        Call {
            uuid: "unknown",
            caller: "+999123456789",
            extension: other,
            minutes_ago: 30,
            ..base
        },
        // Internal calls are not on the map.
        Call {
            uuid: "internal",
            direction: Direction::Internal,
            caller: "101",
            destination: "102",
            minutes_ago: 5,
            answered: true,
            billsec: 10,
            ..base
        },
        // Older than 30 days.
        Call {
            uuid: "old",
            caller: "+4995199999",
            minutes_ago: 31 * 24 * 60,
            ..base
        },
    ] {
        insert(&db, call).await;
    }

    let (status, hour) = admin.get("/api/v1/calls/map?range=1h").await;
    assert_eq!(status, StatusCode::OK, "{hour}");
    assert_eq!(hour["range"], "1h");
    assert_eq!(hour["truncated"], false);
    assert_eq!(ids(&hour), ["DE-951", "DE-none", "none"]);
    let bamberg = area(&hour, "DE-951").unwrap();
    assert_eq!(bamberg["kind"], "area");
    assert_eq!(bamberg["name"], "Bamberg");
    assert_eq!(bamberg["prefix"], "+49 951");
    assert_eq!(bamberg["country"], "DE");
    assert_eq!(bamberg["counts"]["answered"], 1);
    assert_eq!(bamberg["counts"]["missed"], 0, "61 minutes ago");
    assert_eq!(bamberg["billsec"], 60);
    assert_eq!(bamberg["calls"][0]["caller"]["name"], "Bäckerei Bamberg");
    let mobile = area(&hour, "DE-none").unwrap();
    assert_eq!(mobile["kind"], "none");
    assert_eq!(mobile["counts"]["blocked"], 1);
    assert_eq!(mobile["mobile"], 1);
    assert_eq!(
        (mobile["lat"].as_f64(), mobile["lon"].as_f64()),
        (Some(51.0), Some(10.0))
    );
    let unknown = area(&hour, "none").unwrap();
    assert_eq!(unknown["unresolved"], 1);
    assert!(unknown["lat"].is_null() && unknown["country"].is_null());
    assert_eq!(hour["anonymous"]["blocked"], 1);
    assert_eq!(hour["total"], 4, "the internal call is not counted");
    assert!(!hour["sources"].as_array().unwrap().is_empty());

    let (_, day) = admin.get("/api/v1/calls/map").await;
    assert_eq!(day["range"], "1d", "default range");
    let bamberg = area(&day, "DE-951").unwrap();
    assert_eq!(
        (&bamberg["counts"]["answered"], &bamberg["counts"]["missed"]),
        (&json!(1), &json!(1))
    );
    assert_eq!(bamberg["total"], 2);
    let calls = bamberg["calls"].as_array().unwrap();
    assert_eq!(calls[0]["call_uuid"], "bamberg-answered", "newest first");
    assert_eq!(calls[1]["call_uuid"], "bamberg-missed");
    assert!(area(&day, "AT-1").is_none());

    let (_, week) = admin.get("/api/v1/calls/map?range=1w").await;
    let vienna = area(&week, "AT-1").unwrap();
    assert_eq!(vienna["counts"]["outbound"], 1);
    assert_eq!(vienna["name"], "Wien");
    let (_, month) = admin.get("/api/v1/calls/map?range=1m").await;
    assert_eq!(
        area(&month, "DE-951").unwrap()["total"],
        2,
        "not older than 30 days"
    );

    let (_, outbound) = admin
        .get("/api/v1/calls/map?range=1w&categories=outbound")
        .await;
    assert_eq!(ids(&outbound), ["AT-1"]);
    assert_eq!(outbound["counts"]["answered"], 0);
    assert_eq!(
        admin.get("/api/v1/calls/map?categories=spam").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        admin.get("/api/v1/calls/map?range=2y").await.0,
        StatusCode::BAD_REQUEST
    );

    // The call log filter of a bubble.
    let since = (Utc::now() - Duration::days(1)).to_rfc3339();
    let query = |area: &str| format!("/api/v1/calls?area={area}&since={}", urlencode(&since));
    let (_, rows) = admin.get(&query("DE-951")).await;
    let uuids: Vec<&str> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["call_uuid"].as_str().unwrap())
        .collect();
    assert_eq!(uuids, ["bamberg-answered", "bamberg-missed"]);
    let (_, rows) = admin.get(&query("DE-none")).await;
    assert_eq!(rows[0]["call_uuid"], "mobile-blocked");
    assert_eq!(rows.as_array().unwrap().len(), 1);
    let (_, rows) = admin.get(&query("XX-1")).await;
    assert!(rows.as_array().unwrap().is_empty());

    // A user without calls.view_all sees only the calls of own extensions.
    admin
        .post(
            "/api/v1/users",
            json!({"username": "user", "display_name": "User", "password": "user-password", "role": "user"}),
        )
        .await;
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username = 'user'")
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE extensions SET user_id = $1 WHERE id = $2")
        .bind(user_id)
        .bind(own)
        .execute(&db)
        .await
        .unwrap();
    let user = login(&router, "user", "user-password").await.unwrap();
    let (status, mine) = user.get("/api/v1/calls/map?range=1w").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&mine), ["AT-1", "DE-951", "DE-none"]);
    assert_eq!(area(&mine, "DE-951").unwrap()["counts"]["missed"], 0);
    assert_eq!(area(&mine, "DE-951").unwrap()["total"], 1);
    let (_, rows) = user.get(&query("DE-951")).await;
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["call_uuid"], "bamberg-answered");

    // Without geo data, all calls count as unknown country.
    let empty = talkops_api::app(common::state(db.clone()), None);
    let user = login(&empty, "user", "user-password").await.unwrap();
    let (_, plain) = user.get("/api/v1/calls/map?range=1w").await;
    assert_eq!(ids(&plain), ["none"]);
}

fn urlencode(text: &str) -> String {
    text.replace('+', "%2B").replace(':', "%3A")
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn requires_a_login(db: PgPool) {
    let router = router(db);
    let res = raw(
        &router,
        axum::http::Request::get("/api/v1/calls/map")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn tile_server_setting_is_validated_audited_and_allowed_by_the_page_policy(db: PgPool) {
    let web = std::env::temp_dir().join(format!("talkops-web-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&web).unwrap();
    std::fs::write(
        web.join("index.html"),
        "<html><head><meta http-equiv=\"content-security-policy\" content=\"default-src 'self'; img-src 'self' data: blob:; media-src 'self'\"></head></html>",
    )
    .unwrap();
    let router = talkops_api::app(common::state(db.clone()), Some(&web));
    let admin = setup_admin(&router).await;
    // `cached`: the browser has the page from an earlier visit.
    let page = |path: &'static str, cached: bool| {
        let router = router.clone();
        async move {
            let mut req = axum::http::Request::get(path);
            if cached {
                req = req.header("if-modified-since", "Wed, 21 Oct 2099 07:28:00 GMT");
            }
            let res = raw(&router, req.body(axum::body::Body::empty()).unwrap()).await;
            assert_eq!(res.status(), StatusCode::OK, "{path}");
            body_string(res).await
        }
    };

    // Default: offline map, the page policy is unchanged.
    let (status, tiles) = admin.get("/api/v1/settings/call-map").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tiles, json!({"tile_url": "", "attribution": ""}));
    assert!(
        page("/calls", false)
            .await
            .contains("img-src 'self' data: blob:")
    );

    for (body, why) in [
        (
            json!({"tile_url": "http://tile.example.com/{z}/{x}/{y}.png", "attribution": "x"}),
            "https only",
        ),
        (
            json!({"tile_url": "https://tile.example.com/{z}/{x}.png", "attribution": "x"}),
            "placeholders",
        ),
        (
            json!({"tile_url": "https://tile.example.com/{z}/{x}/{y}.png'; script-src *", "attribution": "x"}),
            "characters",
        ),
        (
            json!({"tile_url": "https://tile.example.com/{z}/{x}/{y}.png", "attribution": ""}),
            "attribution",
        ),
    ] {
        assert_eq!(
            admin.put("/api/v1/settings/call-map", body).await.0,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{why}"
        );
    }
    let osm = json!({"tile_url": "https://{s}.tile.example.com/{z}/{x}/{y}.png", "attribution": "© OpenStreetMap contributors"});
    let (status, saved) = admin.put("/api/v1/settings/call-map", osm.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved, osm);
    let audits: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE entity_type = 'call_map_tiles'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(audits, 1);
    for path in ["/", "/calls"] {
        // A cached page must not keep the old policy.
        let html = page(path, true).await;
        assert!(
            html.contains("img-src 'self' https://*.tile.example.com data: blob:"),
            "{path}: {html}"
        );
    }

    // Users read the setting for the map, only settings managers change it.
    admin
        .post(
            "/api/v1/users",
            json!({"username": "user", "display_name": "User", "password": "user-password", "role": "user"}),
        )
        .await;
    let user = login(&router, "user", "user-password").await.unwrap();
    assert_eq!(user.get("/api/v1/settings/call-map").await.1, osm);
    assert_eq!(
        user.put(
            "/api/v1/settings/call-map",
            json!({"tile_url": "", "attribution": ""})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // Clearing the URL returns to the offline map.
    let (status, _) = admin
        .put(
            "/api/v1/settings/call-map",
            json!({"tile_url": "", "attribution": ""}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page("/", false)
            .await
            .contains("img-src 'self' data: blob:")
    );
    std::fs::remove_dir_all(&web).ok();
}
