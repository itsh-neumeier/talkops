//! Changes to the UniFi voice SSID (ADR 0042) against a mock console in the
//! format of UniFi Network 10.6.106 and 11.0.89: gates, apply, rollback,
//! foreign changes, roles and audit entries without secrets.

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::routing::any;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::roles::{OPERATOR_DEFAULTS, Permission};
use talkops_core::tenant::TenantId;
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;
const SITE: &str = "88f7af54-98f8-306a-a1c7-c9349722b1f6";
const WIFI: &str = "6f1c2a3b-0000-4000-8000-000000000001";
const OTHER: &str = "7a2d0000-0000-4000-8000-000000000002";
const PASS: &str = "correct-horse-battery";

#[derive(Clone)]
struct Console {
    version: Arc<Mutex<String>>,
    wifi: Arc<Mutex<Value>>,
    put_status: Arc<Mutex<u16>>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    puts: Arc<Mutex<Vec<Value>>>,
}

fn wifi(version: &str) -> Value {
    let mut v = json!({
        "type": "STANDARD", "id": WIFI, "name": "Voice", "enabled": true,
        "metadata": {"origin": "USER_DEFINED"},
        "network": {"type": "NATIVE"},
        "securityConfiguration": {"type": "WPA2_WPA3_PERSONAL", "passphrase": PASS,
            "pmfMode": null, "fastRoamingEnabled": false, "wpa3FastRoamingEnabled": false,
            "saeConfiguration": {"anticloggingThresholdSeconds": 5, "syncTimeSeconds": 5}},
        "broadcastingFrequenciesGHz": [2.4, 5], "bssTransitionEnabled": false,
        "uapsdEnabled": false, "multicastToUnicastConversionEnabled": false,
        "arpProxyEnabled": false, "channel2gLockedTo6": false, "dtimPeriod2gLockedTo3": false,
        "hideName": false, "clientIsolationEnabled": false, "advertiseDeviceName": false,
        "basicDataRateKbpsByFrequencyGHz": {"2.4": 12000, "5": 12000}
    });
    if version.starts_with("11.") {
        v["futureFieldOf11"] = json!(true);
    }
    v
}

async fn handler(
    State(c): State<Console>,
    method: Method,
    uri: Uri,
    h: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> (StatusCode, axum::Json<Value>) {
    let path = uri.path().to_owned();
    c.requests
        .lock()
        .unwrap()
        .push((method.to_string(), path.clone()));
    if h.get("x-api-key").and_then(|v| v.to_str().ok()) != Some("k1") {
        return (StatusCode::UNAUTHORIZED, axum::Json(json!({})));
    }
    let base = "/proxy/network/integration/v1";
    let one = format!("{base}/sites/{SITE}/wifi/broadcasts/{WIFI}");
    let ok = |v: Value| (StatusCode::OK, axum::Json(v));
    match (method, path.as_str()) {
        (Method::GET, p) if p == format!("{base}/info") => {
            ok(json!({"applicationVersion": *c.version.lock().unwrap()}))
        }
        (Method::GET, p) if p == format!("{base}/sites") => {
            ok(json!({"data": [{"id": SITE, "internalReference": "default", "name": "Default"}]}))
        }
        (Method::GET, p) if p == format!("{base}/sites/{SITE}/wifi/broadcasts") => {
            let w = c.wifi.lock().unwrap().clone();
            ok(
                json!({"offset": 0, "limit": 200, "count": 2, "totalCount": 2, "data": [
                {"type": "STANDARD", "id": WIFI, "name": w["name"], "enabled": true,
                 "metadata": {"origin": "USER_DEFINED"},
                 "securityConfiguration": {"type": "WPA2_WPA3_PERSONAL"},
                 "broadcastingFrequenciesGHz": w["broadcastingFrequenciesGHz"]},
                {"type": "IOT_OPTIMIZED", "id": OTHER, "name": "IoT", "enabled": true,
                 "metadata": {"origin": "USER_DEFINED"},
                 "securityConfiguration": {"type": "WPA2_PERSONAL"}}]}),
            )
        }
        (Method::GET, p) if p == one => ok(c.wifi.lock().unwrap().clone()),
        (Method::GET, p) if p == format!("{base}/sites/{SITE}/wifi/broadcasts/{OTHER}") => {
            ok(json!({"type": "IOT_OPTIMIZED", "id": OTHER, "name": "IoT",
                "metadata": {"origin": "USER_DEFINED"}, "securityConfiguration": {"type": "WPA2_PERSONAL",
                "passphrase": PASS}}))
        }
        (Method::PUT, p) if p == one => {
            let status = *c.put_status.lock().unwrap();
            if status != 200 {
                return (
                    StatusCode::from_u16(status).unwrap(),
                    axum::Json(json!({"code": "api.err", "message": PASS})),
                );
            }
            let mut v: Value = serde_json::from_slice(&body).unwrap();
            c.puts.lock().unwrap().push(v.clone());
            v["id"] = json!(WIFI);
            v["metadata"] = json!({"origin": "USER_DEFINED"});
            *c.wifi.lock().unwrap() = v.clone();
            ok(v)
        }
        _ => (StatusCode::NOT_FOUND, axum::Json(json!({}))),
    }
}

async fn console(version: &str) -> (String, Console) {
    let c = Console {
        version: Arc::new(Mutex::new(version.into())),
        wifi: Arc::new(Mutex::new(wifi(version))),
        put_status: Arc::new(Mutex::new(200)),
        requests: Arc::default(),
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

/// Stores the connection directly: the API accepts only `https://`, the
/// mock console speaks plain HTTP.
async fn connect(db: &PgPool, admin: &Client, url: &str) {
    let enc = SecretBox::from_hex(KEY).unwrap().encrypt("k1").unwrap();
    sqlx::query(
        "INSERT INTO unifi_connections (tenant_id, url, site, api_key_enc) VALUES ($1, $2, 'default', $3)",
    )
    .bind(T)
    .bind(url)
    .bind(enc)
    .execute(db)
    .await
    .unwrap();
    let (status, view) = admin.post("/api/v1/unifi/connection/test", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{view}");
}

async fn survey(db: &PgPool, state: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO wifi_surveys (tenant_id, mode, state, expires_at, summary)
         VALUES ($1, 'originate', $2, now() + interval '1 day', $3) RETURNING id",
    )
    .bind(T)
    .bind(state)
    .bind(json!({"recommendations": [
        {"id": "fast_roaming", "severity": "warning", "values": {}},
        {"id": "min_rssi", "severity": "warning", "values": {}},
        {"id": "band_5ghz", "severity": "info", "values": {}},
        {"id": "coverage", "severity": "warning", "values": {}},
        {"id": "voice_ssid", "severity": "info", "values": {}}]}))
    .fetch_one(db)
    .await
    .unwrap()
}

fn selected(proposal: &Value) -> Vec<Value> {
    proposal["changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["selected"] == true)
        .map(|c| json!({"field": c["field"], "value": c["proposed"]}))
        .collect()
}

async fn texts(db: &PgPool) -> String {
    let audit: Vec<String> = sqlx::query_scalar("SELECT details::text FROM audit_log")
        .fetch_all(db)
        .await
        .unwrap();
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT row_to_json(c)::text FROM unifi_wifi_changes c")
            .fetch_all(db)
            .await
            .unwrap();
    let conn: Vec<String> =
        sqlx::query_scalar("SELECT (to_jsonb(c) - 'api_key_enc')::text FROM unifi_connections c")
            .fetch_all(db)
            .await
            .unwrap();
    [audit, rows, conn].concat().join("\n")
}

async fn flow(db: PgPool, version: &str) {
    let (url, mock) = console(version).await;
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    connect(&db, &admin, &url).await;
    let id = survey(&db, "analysed").await;

    // The connection test reports the write capabilities.
    let (_, view) = admin.get("/api/v1/unifi/connection").await;
    let caps = &view["connection"]["capabilities"];
    assert_eq!(caps["wifi_broadcasts"], true);
    assert_eq!(caps["write_supported"], true);
    assert_eq!(caps["write_min_version"], "10.6.106");
    assert_eq!(view["connection"]["write_enabled"], false);

    let (status, list) = admin.get("/api/v1/unifi/wifi-broadcasts").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().unwrap().len(), 2);
    assert_eq!(list[1]["type"], "IOT_OPTIMIZED");

    // Without the write switch: proposal yes, apply no.
    let (status, _) = admin
        .put("/api/v1/unifi/voice-ssid", json!({"wifi_id": WIFI}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, p) = admin
        .get(&format!("/api/v1/wifi-surveys/{id}/unifi-proposal"))
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["write"]["allowed"], false);
    assert_eq!(p["write"]["reason"], "write_disabled");
    assert_eq!(p["wifi_name"], "Voice");
    let fields: Vec<&str> = p["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        fields,
        [
            "bssTransitionEnabled",
            "handoffSuggestionsConfiguration",
            "broadcastingFrequenciesGHz",
            "uapsdEnabled",
            "multicastToUnicastConversionEnabled",
            "arpProxyEnabled",
            "dtimPeriodByFrequencyGHzOverride",
            "securityConfiguration.pmfMode"
        ]
    );
    let hints: Vec<&str> = p["hints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["id"].as_str().unwrap())
        .collect();
    assert!(hints.contains(&"fast_roaming_80211r") && hints.contains(&"tx_power"));
    let apply = json!({"survey_id": id, "wifi_id": WIFI, "changes": selected(&p),
        "base_hash": p["base_hash"], "confirm": true});
    let (status, err) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(err["error"], "write_disabled");
    assert!(mock.puts.lock().unwrap().is_empty());

    // Switch writes on.
    let (status, view) = admin
        .put(
            "/api/v1/unifi/voice-ssid",
            json!({"wifi_id": WIFI, "write_enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["connection"]["write_enabled"], true);
    assert_eq!(view["connection"]["voice_wifi_name"], "Voice");

    // No write without confirmation.
    let mut unconfirmed = apply.clone();
    unconfirmed["confirm"] = json!(false);
    let (status, _) = admin.post("/api/v1/unifi/wifi-changes", unconfirmed).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // Not an allowlist field.
    let mut bad = apply.clone();
    bad["changes"] = json!([{"field": "securityConfiguration.passphrase", "value": "x"}]);
    let (status, _) = admin.post("/api/v1/unifi/wifi-changes", bad).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(mock.puts.lock().unwrap().is_empty());

    let (status, change) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CREATED, "{change}");
    assert_eq!(change["state"], "applied");
    assert_eq!(change["kind"], "apply");
    assert_eq!(change["survey_id"], json!(id));
    assert_eq!(change["before"]["uapsdEnabled"], json!({"value": false}));
    assert_eq!(change["after"]["uapsdEnabled"], json!({"value": true}));
    assert_eq!(
        change["before"]["handoffSuggestionsConfiguration"],
        json!({"absent": true})
    );
    // The band change is not selected by default.
    assert!(
        !change["fields"]
            .as_array()
            .unwrap()
            .contains(&json!("broadcastingFrequenciesGHz"))
    );

    let now = mock.wifi.lock().unwrap().clone();
    assert_eq!(now["securityConfiguration"]["passphrase"], PASS);
    assert_eq!(now["broadcastingFrequenciesGHz"], json!([2.4, 5]));
    assert_eq!(
        now["handoffSuggestionsConfiguration"]["band5GHzRssiThreshold"],
        -75
    );
    assert_eq!(
        now["dtimPeriodByFrequencyGHzOverride"],
        json!({"2.4": 1, "5": 1, "6": 1})
    );
    assert_eq!(now["securityConfiguration"]["pmfMode"], "OPTIONAL");
    if version.starts_with("11.") {
        assert_eq!(now["futureFieldOf11"], true);
    }
    // Only GET and PUT, and the PUT only on the voice SSID.
    for (m, path) in mock.requests.lock().unwrap().iter() {
        assert!(m == "GET" || m == "PUT", "{m}");
        assert!(!path.contains("/api/s/") || m == "GET", "{path}");
        if m == "PUT" {
            assert!(path.ends_with(&format!("/wifi/broadcasts/{WIFI}")));
        }
    }
    assert_eq!(mock.puts.lock().unwrap().len(), 1);

    // An old proposal does not apply again.
    let (status, err) = admin.post("/api/v1/unifi/wifi-changes", apply).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "ssid_changed");

    // History and rollback.
    let (_, history) = admin.get("/api/v1/unifi/wifi-changes").await;
    assert_eq!(history[0]["state"], "failed");
    assert_eq!(history[0]["error"], "ssid_changed");
    assert_eq!(history[1]["id"], change["id"]);
    let change_id = change["id"].as_str().unwrap();
    let (status, _) = admin
        .post(
            &format!("/api/v1/unifi/wifi-changes/{change_id}/rollback"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "confirm needed");
    // A field that TalkOps did not change may change meanwhile.
    mock.wifi.lock().unwrap()["hideName"] = json!(true);
    let (status, rb) = admin
        .post(
            &format!("/api/v1/unifi/wifi-changes/{change_id}/rollback"),
            json!({"confirm": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{rb}");
    assert_eq!(rb["kind"], "rollback");
    assert_eq!(rb["rollback_of"], change["id"]);
    let now = mock.wifi.lock().unwrap().clone();
    assert_eq!(now["uapsdEnabled"], false);
    assert!(now.get("handoffSuggestionsConfiguration").is_none());
    assert!(now.get("dtimPeriodByFrequencyGHzOverride").is_none());
    assert_eq!(now["securityConfiguration"]["pmfMode"], Value::Null);
    assert_eq!(now["hideName"], true, "the other change stays");
    assert_eq!(now["securityConfiguration"]["passphrase"], PASS);
    let (_, history) = admin.get("/api/v1/unifi/wifi-changes").await;
    let original = history
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == change["id"])
        .unwrap();
    assert_eq!(original["state"], "rolled_back");
    let (status, err) = admin
        .post(
            &format!("/api/v1/unifi/wifi-changes/{change_id}/rollback"),
            json!({"confirm": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "not_revertible");

    // Audit entries for apply and rollback, never a secret anywhere.
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE entity_type = 'unifi' ORDER BY id")
            .fetch_all(&db)
            .await
            .unwrap();
    for a in [
        "unifi.voice_ssid",
        "unifi.write_access",
        "unifi.wifi.apply",
        "unifi.wifi.rollback",
    ] {
        assert!(actions.iter().any(|x| x == a), "{a}: {actions:?}");
    }
    assert!(!texts(&db).await.contains(PASS));
    assert!(!history.to_string().contains(PASS));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn apply_and_rollback_on_10_6_106(db: PgPool) {
    flow(db, "10.6.106").await;
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn apply_and_rollback_on_11_0_89(db: PgPool) {
    flow(db, "11.0.89").await;
}

async fn ready(db: &PgPool, version: &str) -> (Client, Console, Uuid, Value) {
    let (url, mock) = console(version).await;
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    connect(db, &admin, &url).await;
    let id = survey(db, "analysed").await;
    let (status, _) = admin
        .put(
            "/api/v1/unifi/voice-ssid",
            json!({"wifi_id": WIFI, "write_enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, p) = admin
        .get(&format!("/api/v1/wifi-surveys/{id}/unifi-proposal"))
        .await;
    let apply = json!({"survey_id": id, "wifi_id": WIFI, "changes": [{"field": "uapsdEnabled", "value": true}],
        "base_hash": p["base_hash"], "confirm": true});
    (admin, mock, id, apply)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn gates_and_unifi_errors(db: PgPool) {
    let (admin, mock, _, apply) = ready(&db, "11.0.89").await;

    // A running survey blocks writes.
    let running = survey(&db, "running").await;
    let (status, err) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "survey_active");
    sqlx::query("UPDATE wifi_surveys SET state = 'analysed' WHERE id = $1")
        .bind(running)
        .execute(&db)
        .await
        .unwrap();

    // Another SSID than the voice SSID.
    let mut other = apply.clone();
    other["wifi_id"] = json!(OTHER);
    let (status, err) = admin.post("/api/v1/unifi/wifi-changes", other).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "ssid_not_voice");
    // An IoT SSID cannot be the voice SSID with writes on.
    let (status, err) = admin
        .put(
            "/api/v1/unifi/voice-ssid",
            json!({"wifi_id": OTHER, "write_enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "ssid_type");

    // The key may read but not write.
    *mock.put_status.lock().unwrap() = 403;
    let (status, err) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(err["error"], "unifi_forbidden");
    *mock.put_status.lock().unwrap() = 401;
    let (status, _) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    *mock.put_status.lock().unwrap() = 400;
    let (status, err) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(!err.to_string().contains(PASS));
    let (_, history) = admin.get("/api/v1/unifi/wifi-changes").await;
    let errors: Vec<&str> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["error"].as_str().unwrap())
        .collect();
    assert_eq!(errors, ["rejected", "unauthorized", "forbidden"]);
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "failed")
    );

    // A masked passphrase stops the write before the PUT.
    *mock.put_status.lock().unwrap() = 200;
    mock.wifi.lock().unwrap()["securityConfiguration"]["passphrase"] = json!("**********");
    let (_, p) = admin
        .get(&format!(
            "/api/v1/wifi-surveys/{}/unifi-proposal",
            apply["survey_id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(p["not_writable"], "secret_not_readable");
    let mut masked = apply.clone();
    masked["base_hash"] = p["base_hash"].clone();
    let (status, err) = admin.post("/api/v1/unifi/wifi-changes", masked).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "secret_not_readable");
    assert!(mock.puts.lock().unwrap().is_empty());

    // Switching writes off restores the read-only behavior.
    let (status, _) = admin
        .put("/api/v1/unifi/voice-ssid", json!({"wifi_id": WIFI}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin.post("/api/v1/unifi/wifi-changes", apply).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!texts(&db).await.contains(PASS));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn old_versions_stay_read_only(db: PgPool) {
    let (url, mock) = console("10.6.105").await;
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    connect(&db, &admin, &url).await;
    let (_, view) = admin.get("/api/v1/unifi/connection").await;
    assert_eq!(view["connection"]["capabilities"]["write_supported"], false);
    let (status, err) = admin
        .put(
            "/api/v1/unifi/voice-ssid",
            json!({"wifi_id": WIFI, "write_enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "version_unsupported");
    // The voice SSID alone (read only) is fine.
    let (status, _) = admin
        .put("/api/v1/unifi/voice-ssid", json!({"wifi_id": WIFI}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(mock.puts.lock().unwrap().is_empty());
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn foreign_change_asks_before_rollback(db: PgPool) {
    let (admin, mock, _, apply) = ready(&db, "11.0.89").await;
    let (status, change) = admin.post("/api/v1/unifi/wifi-changes", apply).await;
    assert_eq!(status, StatusCode::CREATED, "{change}");
    // Another person switches U-APSD off again in UniFi.
    mock.wifi.lock().unwrap()["uapsdEnabled"] = json!(false);
    let path = format!(
        "/api/v1/unifi/wifi-changes/{}/rollback",
        change["id"].as_str().unwrap()
    );
    let (status, err) = admin.post(&path, json!({"confirm": true})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "foreign_change");
    assert_eq!(err["fields"][0]["field"], "uapsdEnabled");
    assert_eq!(err["fields"][0]["after"], json!({"value": true}));
    assert_eq!(err["fields"][0]["current"], json!({"value": false}));
    assert_eq!(err["fields"][0]["before"], json!({"value": false}));
    assert_eq!(mock.puts.lock().unwrap().len(), 1);
    let (status, rb) = admin
        .post(
            &path,
            json!({"confirm": true, "accept_foreign_changes": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{rb}");
    let detail: String = sqlx::query_scalar(
        "SELECT details::text FROM audit_log WHERE action = 'unifi.wifi.rollback'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(
        detail.contains("\"foreign_changes_accepted\": true"),
        "{detail}"
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn roles(db: PgPool) {
    assert!(Permission::UnifiWrite.sensitive());
    assert!(!OPERATOR_DEFAULTS.contains(&Permission::UnifiWrite));
    let (admin, _, survey_id, apply) = ready(&db, "11.0.89").await;
    let (status, change) = admin
        .post("/api/v1/unifi/wifi-changes", apply.clone())
        .await;
    assert_eq!(status, StatusCode::CREATED);

    // A role with `unifi.manage` but without `unifi.write`.
    let (status, role) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "Wi-Fi", "permissions": ["unifi.manage", "wifi_survey.run", "wifi_survey.manage"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{role}");
    for (name, role) in [
        ("wifi", json!({"role_id": role["id"]})),
        ("op", json!({"role": "operator"})),
    ] {
        let mut body = json!({"username": name, "display_name": name, "password": "correct-horse-2", "role": "user"});
        body.as_object_mut()
            .unwrap()
            .extend(role.as_object().unwrap().clone());
        let (status, user) = admin.post("/api/v1/users", body).await;
        assert_eq!(status, StatusCode::OK, "{user}");
    }
    let router = admin.router.clone();
    let wifi = login(&router, "wifi", "correct-horse-2").await.unwrap();
    let (status, p) = wifi
        .get(&format!("/api/v1/wifi-surveys/{survey_id}/unifi-proposal"))
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["write"]["reason"], "permission");
    let (status, _) = wifi.get("/api/v1/unifi/wifi-changes").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = wifi.post("/api/v1/unifi/wifi-changes", apply).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = wifi
        .post(
            &format!(
                "/api/v1/unifi/wifi-changes/{}/rollback",
                change["id"].as_str().unwrap()
            ),
            json!({"confirm": true}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Writes are on: another SSID or the switch need `unifi.write`.
    let (status, _) = wifi
        .put("/api/v1/unifi/voice-ssid", json!({"wifi_id": WIFI}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let op = login(&router, "op", "correct-horse-2").await.unwrap();
    for path in [
        format!("/api/v1/wifi-surveys/{survey_id}/unifi-proposal"),
        "/api/v1/unifi/wifi-changes".to_owned(),
        "/api/v1/unifi/wifi-broadcasts".to_owned(),
    ] {
        let (status, _) = op.get(&path).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
}
