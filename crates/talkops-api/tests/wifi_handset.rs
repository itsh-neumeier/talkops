//! Wi-Fi handset roaming threshold and band protection (ADR 0043): the
//! proposal for the survey handset, apply with `waiting` until the handset
//! fetches its configuration, rollback, roles, and the band check before a
//! band removal on the voice SSID.

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Method, Request, StatusCode, Uri, header};
use axum::routing::any;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::phones;
use talkops_core::tenant::TenantId;
use talkops_core::wifi_survey::Thresholds;
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;
const SITE: &str = "88f7af54-98f8-306a-a1c7-c9349722b1f6";
const WIFI: &str = "6f1c2a3b-0000-4000-8000-000000000001";
const MAC: &str = "249ad8000001";
const MAC_COLON: &str = "24:9a:d8:00:00:01";

#[derive(Clone)]
struct Console {
    wifi: Arc<Mutex<Value>>,
    /// Classic `stat/sta` answer for the handset: `None` = not connected.
    station: Arc<Mutex<Option<Value>>>,
    puts: Arc<Mutex<u32>>,
}

async fn handler(
    State(c): State<Console>,
    method: Method,
    uri: Uri,
    body: axum::body::Bytes,
) -> (StatusCode, axum::Json<Value>) {
    let base = "/proxy/network/integration/v1";
    let one = format!("{base}/sites/{SITE}/wifi/broadcasts/{WIFI}");
    let ok = |v: Value| (StatusCode::OK, axum::Json(v));
    let path = uri.path().to_owned();
    match (method, path.as_str()) {
        (Method::GET, p) if p == format!("{base}/info") => {
            ok(json!({"applicationVersion": "11.0.89"}))
        }
        (Method::GET, p) if p == format!("{base}/sites") => {
            ok(json!({"data": [{"id": SITE, "internalReference": "default"}]}))
        }
        (Method::GET, p) if p == format!("{base}/sites/{SITE}/wifi/broadcasts") => ok(
            json!({"data": [{"type": "STANDARD", "id": WIFI, "name": "Voice", "enabled": true,
                "metadata": {"origin": "USER_DEFINED"}}]}),
        ),
        (Method::GET, p) if p == one => ok(c.wifi.lock().unwrap().clone()),
        (Method::PUT, p) if p == one => {
            let mut v: Value = serde_json::from_slice(&body).unwrap();
            v["id"] = json!(WIFI);
            v["metadata"] = json!({"origin": "USER_DEFINED"});
            *c.wifi.lock().unwrap() = v.clone();
            *c.puts.lock().unwrap() += 1;
            ok(v)
        }
        (Method::GET, "/proxy/network/api/s/default/stat/sta") => ok(json!({"data": []})),
        (Method::GET, p) if p == format!("/proxy/network/api/s/default/stat/sta/{MAC_COLON}") => {
            match c.station.lock().unwrap().clone() {
                Some(s) => ok(json!({"data": [s]})),
                None => (StatusCode::BAD_REQUEST, axum::Json(json!({}))),
            }
        }
        _ => (StatusCode::NOT_FOUND, axum::Json(json!({}))),
    }
}

async fn console() -> (String, Console) {
    let c = Console {
        wifi: Arc::new(Mutex::new(json!({
            "type": "STANDARD", "id": WIFI, "name": "Voice", "enabled": true,
            "metadata": {"origin": "USER_DEFINED"},
            "securityConfiguration": {"type": "WPA2_WPA3_PERSONAL", "passphrase": "correct-horse-battery",
                "pmfMode": "OPTIONAL", "fastRoamingEnabled": false, "wpa3FastRoamingEnabled": false},
            "broadcastingFrequenciesGHz": [2.4, 5], "bssTransitionEnabled": true,
            "uapsdEnabled": true, "multicastToUnicastConversionEnabled": true,
            "arpProxyEnabled": true, "channel2gLockedTo6": false, "dtimPeriod2gLockedTo3": false,
            "dtimPeriodByFrequencyGHzOverride": {"2.4": 1, "5": 1, "6": 1},
            "hideName": false, "clientIsolationEnabled": false, "advertiseDeviceName": false
        }))),
        station: Arc::new(Mutex::new(Some(json!({"mac": MAC_COLON,
            "ap_mac": "f4:92:bf:00:00:02", "essid": "Voice", "radio": "ng", "signal": -60})))),
        puts: Arc::default(),
    };
    let app = Router::new()
        .route("/{*path}", any(handler))
        .with_state(c.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), c)
}

struct Setup {
    admin: Client,
    mock: Console,
    phone: String,
    survey: Uuid,
}

async fn setup(db: &PgPool) -> Setup {
    let (url, mock) = console().await;
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let enc = SecretBox::from_hex(KEY).unwrap().encrypt("k1").unwrap();
    sqlx::query(
        "INSERT INTO unifi_connections (tenant_id, url, site, api_key_enc) VALUES ($1, $2, 'default', $3)",
    )
    .bind(T)
    .bind(&url)
    .bind(enc)
    .execute(db)
    .await
    .unwrap();
    let (status, _) = admin.post("/api/v1/unifi/connection/test", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin
        .put(
            "/api/v1/unifi/voice-ssid",
            json!({"wifi_id": WIFI, "write_enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, phone) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": MAC, "model": "ax83h", "name": "Hall handset"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let phone_id = phone["id"].as_str().unwrap().to_owned();
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "300", "display_name": "Walker"}),
        )
        .await;
    let (status, dev) = admin
        .post(
            &format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap()),
            json!({"name": "AX83H", "kind": "wifi", "phone_id": phone_id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{dev}");

    // An analysed survey of this handset with a sticky client.
    let thresholds = Thresholds {
        sticky_rssi: -80,
        ..Thresholds::default()
    };
    let survey = sqlx::query_scalar(
        "INSERT INTO wifi_surveys (tenant_id, phone_id, mode, state, expires_at, thresholds, summary)
         VALUES ($1, $2, 'originate', 'analysed', now() + interval '1 day', $3, $4) RETURNING id",
    )
    .bind(T)
    .bind(Uuid::parse_str(&phone_id).unwrap())
    .bind(serde_json::to_value(thresholds).unwrap())
    .bind(json!({"recommendations": [
        {"id": "min_rssi", "severity": "warning", "values": {}},
        {"id": "band_5ghz", "severity": "info", "values": {}}]}))
    .fetch_one(db)
    .await
    .unwrap();
    Setup {
        admin,
        mock,
        phone: phone_id,
        survey,
    }
}

async fn config(admin: &Client, phone: &str) -> String {
    let res = raw(
        &admin.router,
        Request::get(format!("/api/v1/phones/{phone}/config"))
            .header(header::COOKIE, &admin.cookie)
            .header("x-requested-with", "TalkOps")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    body_string(res).await
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn handset_threshold_flow(db: PgPool) {
    let Setup {
        admin,
        phone,
        survey,
        ..
    } = setup(&db).await;
    assert!(!config(&admin, &phone).await.contains("roaming_threshold"));

    let (status, p) = admin
        .get(&format!("/api/v1/wifi-surveys/{survey}/unifi-proposal"))
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    let h = &p["handset"];
    assert_eq!(h["phone_id"], json!(phone));
    assert_eq!(h["phone_name"], "Hall handset");
    assert_eq!(h["current"], json!({"absent": true}));
    assert_eq!(h["effective"], "-70");
    assert_eq!(h["proposed"], "-75");
    assert_eq!(h["write"]["allowed"], true);

    let apply = json!({"survey_id": survey, "phone_id": phone, "value": "-75",
        "base_value": null, "confirm": true});
    let mut unconfirmed = apply.clone();
    unconfirmed["confirm"] = json!(false);
    let (status, _) = admin
        .post("/api/v1/wifi-handset-changes", unconfirmed)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let mut bad = apply.clone();
    bad["value"] = json!("-20");
    let (status, _) = admin.post("/api/v1/wifi-handset-changes", bad).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, change) = admin
        .post("/api/v1/wifi-handset-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CREATED, "{change}");
    assert_eq!(change["target"], "handset");
    assert_eq!(change["state"], "waiting");
    assert_eq!(change["phone_name"], "Hall handset");
    assert_eq!(change["fields"], json!(["wifi_roaming_threshold"]));
    assert_eq!(
        change["before"],
        json!({"wifi_roaming_threshold": {"absent": true}})
    );
    assert_eq!(
        change["after"],
        json!({"wifi_roaming_threshold": {"value": "-75"}})
    );
    assert!(
        config(&admin, &phone)
            .await
            .contains("static.network.wifi.roaming_threshold = -75")
    );
    // The same proposal does not apply twice.
    let (status, err) = admin.post("/api/v1/wifi-handset-changes", apply).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "handset_changed");

    // The handset fetches its configuration: the change is applied.
    let id = Uuid::parse_str(&phone).unwrap();
    phones::record_seen(&db, id, Some("10.0.0.9"), None)
        .await
        .unwrap();
    let (_, history) = admin.get("/api/v1/unifi/wifi-changes").await;
    let row = history
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == change["id"])
        .unwrap();
    assert_eq!(row["state"], "applied");

    // Rollback asks after a change by another person.
    let path = format!(
        "/api/v1/unifi/wifi-changes/{}/rollback",
        change["id"].as_str().unwrap()
    );
    phones::set_setting(&db, T, id, "wifi_roaming_threshold", Some("-68"))
        .await
        .unwrap();
    let (status, err) = admin.post(&path, json!({"confirm": true})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "foreign_change");
    assert_eq!(err["fields"][0]["current"], json!({"value": "-68"}));
    assert_eq!(err["fields"][0]["before"], json!({"absent": true}));
    let (status, rb) = admin
        .post(
            &path,
            json!({"confirm": true, "accept_foreign_changes": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{rb}");
    assert_eq!(rb["kind"], "rollback");
    assert_eq!(rb["state"], "waiting");
    assert!(!config(&admin, &phone).await.contains("roaming_threshold"));
    let (_, history) = admin.get("/api/v1/unifi/wifi-changes").await;
    let original = history
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == change["id"])
        .unwrap();
    assert_eq!(original["state"], "rolled_back");

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE action LIKE 'wifi.handset.%' ORDER BY id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(actions, ["wifi.handset.apply", "wifi.handset.rollback"]);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn handset_gates_and_roles(db: PgPool) {
    let Setup {
        admin,
        phone,
        survey,
        ..
    } = setup(&db).await;
    let apply = json!({"survey_id": survey, "phone_id": phone, "value": "-75",
        "base_value": null, "confirm": true});

    // A running survey blocks the change.
    let running: Uuid = sqlx::query_scalar(
        "INSERT INTO wifi_surveys (tenant_id, mode, state, expires_at)
         VALUES ($1, 'originate', 'running', now() + interval '1 day') RETURNING id",
    )
    .bind(T)
    .fetch_one(&db)
    .await
    .unwrap();
    let (status, err) = admin
        .post("/api/v1/wifi-handset-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "survey_active");
    sqlx::query("DELETE FROM wifi_surveys WHERE id = $1")
        .bind(running)
        .execute(&db)
        .await
        .unwrap();

    // A desk phone does not take the threshold.
    let (_, desk) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": "805ec0000009", "model": "t54w", "name": "Desk"}),
        )
        .await;
    let mut other = apply.clone();
    other["phone_id"] = desk["id"].clone();
    let (status, err) = admin.post("/api/v1/wifi-handset-changes", other).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "not_wifi_handset");

    // `unifi.write` without `phones.manage` (and the reverse) is not enough.
    for (name, perms) in [
        (
            "wifi",
            json!([
                "unifi.manage",
                "unifi.write",
                "wifi_survey.run",
                "wifi_survey.manage"
            ]),
        ),
        (
            "phones",
            json!([
                "phones.manage",
                "phones.view",
                "wifi_survey.run",
                "wifi_survey.manage",
                "unifi.manage"
            ]),
        ),
    ] {
        let (status, role) = admin
            .post("/api/v1/roles", json!({"name": name, "permissions": perms}))
            .await;
        assert_eq!(status, StatusCode::OK, "{role}");
        let (status, _) = admin
            .post(
                "/api/v1/users",
                json!({"username": name, "display_name": name, "password": "correct-horse-2",
                       "role": "user", "role_id": role["id"]}),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let user = login(&admin.router, name, "correct-horse-2").await.unwrap();
        let (status, p) = user
            .get(&format!("/api/v1/wifi-surveys/{survey}/unifi-proposal"))
            .await;
        assert_eq!(status, StatusCode::OK, "{p}");
        assert_eq!(p["handset"]["write"]["reason"], "permission");
        let (status, _) = user
            .post("/api/v1/wifi-handset-changes", apply.clone())
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{name}");
    }
    let (status, _) = admin.post("/api/v1/wifi-handset-changes", apply).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn band_check_protects_handsets(db: PgPool) {
    let Setup {
        admin,
        mock,
        phone,
        survey,
    } = setup(&db).await;
    let (_, p) = admin
        .get(&format!("/api/v1/wifi-surveys/{survey}/unifi-proposal"))
        .await;
    let check = &p["band_check"];
    assert_eq!(check[0]["phone_id"], json!(phone));
    assert_eq!(check[0]["state"], "blocked");
    assert_eq!(check[0]["band"], 2.4);
    let apply = json!({"survey_id": survey, "wifi_id": WIFI, "base_hash": p["base_hash"],
        "confirm": true, "changes": [{"field": "broadcastingFrequenciesGHz", "value": [5]}]});

    // Connected on 2.4 GHz to the voice SSID: blocked.
    let (status, err) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "handset_band");
    assert_eq!(err["handsets"][0]["name"], "Hall handset");

    // Not reported by UniFi: needs a named confirmation.
    *mock.station.lock().unwrap() = None;
    let (status, err) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "handset_unknown");
    assert_eq!(err["handsets"][0]["state"], "unknown");
    assert_eq!(*mock.puts.lock().unwrap(), 0);

    // On 5 GHz: the change goes through.
    *mock.station.lock().unwrap() =
        Some(json!({"mac": MAC_COLON, "essid": "Voice", "radio": "na"}));
    let (status, change) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CREATED, "{change}");
    assert_eq!(
        mock.wifi.lock().unwrap()["broadcastingFrequenciesGHz"],
        json!([5])
    );

    // A rollback adds 2.4 GHz again: no band is removed, no check.
    *mock.station.lock().unwrap() = None;
    let (status, rb) = admin
        .post(
            &format!(
                "/api/v1/unifi/wifi-changes/{}/rollback",
                change["id"].as_str().unwrap()
            ),
            json!({"confirm": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{rb}");

    // Unknown handset confirmed by name: allowed.
    let (_, p) = admin
        .get(&format!("/api/v1/wifi-surveys/{survey}/unifi-proposal"))
        .await;
    let mut confirmed = apply;
    confirmed["base_hash"] = p["base_hash"].clone();
    confirmed["confirm_unknown_handsets"] = json!([phone]);
    let (status, change) = admin.post("/api/v1/unifi/wifi-changes", confirmed).await;
    assert_eq!(status, StatusCode::CREATED, "{change}");
}
