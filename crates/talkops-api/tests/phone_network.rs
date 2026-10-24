//! Network data of provisioned phones from the Ubiquiti Networks API: the
//! switch, roles, matching by MAC, device names and console errors.

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::routing::any;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::tenant::TenantId;

const T: TenantId = TenantId::DEFAULT;

#[derive(Clone, Default)]
struct Console {
    /// Number of `stat/sta` requests.
    reads: Arc<Mutex<u32>>,
}

async fn handler(State(c): State<Console>, uri: Uri) -> (StatusCode, axum::Json<Value>) {
    let ok = |v: Value| (StatusCode::OK, axum::Json(v));
    match uri.path() {
        "/proxy/network/api/s/default/stat/sta" => {
            *c.reads.lock().unwrap() += 1;
            ok(json!({"data": [
                {"mac": "24:9a:d8:00:00:01", "ap_mac": "f4:92:bf:00:00:02", "essid": "Voice",
                 "radio": "na", "channel": 36, "signal": -61, "tx_rate": 144444,
                 "rx_rate": 130000, "ip": "10.0.20.31", "vlan": 20, "satisfaction": 98},
                {"mac": "80:5e:c0:00:00:02", "is_wired": true, "sw_mac": "d0:21:f9:00:00:09",
                 "sw_port": 7, "wired_rate_mbps": 1000, "ip": "10.0.20.32"},
                {"mac": "aa:bb:cc:00:00:99", "ip": "10.0.0.99"}
            ]}))
        }
        "/proxy/network/api/s/default/stat/device-basic" => ok(json!({"data": [
            {"mac": "f4:92:bf:00:00:02", "name": "Hall AP"},
            {"mac": "d0:21:f9:00:00:09", "name": "Office switch"}
        ]})),
        _ => (StatusCode::NOT_FOUND, axum::Json(json!({}))),
    }
}

async fn console() -> (String, Console) {
    let c = Console::default();
    let app = Router::new()
        .route("/{*path}", any(handler))
        .with_state(c.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), c)
}

async fn connect(db: &PgPool, url: &str) {
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
}

async fn phone(admin: &Client, mac: &str, model: &str, name: &str) -> String {
    let (status, p) = admin
        .post(
            "/api/v1/phones",
            json!({"mac": mac, "model": model, "name": name}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    p["id"].as_str().unwrap().to_owned()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn phone_network_data(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (status, v) = admin.get("/api/v1/phones/network").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["state"], "not_configured");
    let (status, _) = admin
        .put("/api/v1/unifi/phone-network", json!({"enabled": true}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (url, mock) = console().await;
    connect(&db, &url).await;
    let handset = phone(&admin, "249AD8000001", "ax83h", "Hall handset").await;
    let desk = phone(&admin, "805ec0000002", "t54w", "Desk").await;
    let offline = phone(&admin, "805ec0000003", "t54w", "Store").await;

    // Off by default: no console request.
    let (_, v) = admin.get("/api/v1/phones/network").await;
    assert_eq!(v["state"], "disabled");
    assert_eq!(*mock.reads.lock().unwrap(), 0);

    let (status, view) = admin
        .put("/api/v1/unifi/phone-network", json!({"enabled": true}))
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["connection"]["phone_network"], true);

    let (status, v) = admin.get("/api/v1/phones/network").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["state"], "ok");
    let list = v["phones"].as_array().unwrap();
    assert_eq!(list.len(), 2, "{v}");
    let find = |id: &str| list.iter().find(|p| p["phone_id"] == id).unwrap();
    let w = find(&handset);
    assert_eq!(w["client"]["rssi"], -61);
    assert_eq!(w["client"]["wired"], false);
    assert_eq!(w["ap_name"], "Hall AP");
    let d = find(&desk);
    assert_eq!(d["client"]["sw_port"], 7);
    assert_eq!(d["switch_name"], "Office switch");
    assert!(list.iter().all(|p| p["phone_id"] != offline.as_str()));

    // A second read inside the cache interval does not ask the console.
    admin.get("/api/v1/phones/network").await;
    assert_eq!(*mock.reads.lock().unwrap(), 1);

    let audit: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'unifi.phone_network'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(audit, 1);

    // phones.view reads the data; only unifi.manage switches it.
    let (_, role) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "viewer", "permissions": ["phones.view"]}),
        )
        .await;
    admin
        .post(
            "/api/v1/users",
            json!({"username": "viewer", "display_name": "Viewer", "password": "correct-horse-2",
                   "role": "user", "role_id": role["id"]}),
        )
        .await;
    let viewer = login(&admin.router, "viewer", "correct-horse-2")
        .await
        .unwrap();
    let (status, v) = viewer.get("/api/v1/phones/network").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["state"], "ok");
    let (status, _) = viewer
        .put("/api/v1/unifi/phone-network", json!({"enabled": false}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn phone_network_console_error(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    // Nothing listens on this port.
    connect(&db, "http://127.0.0.1:9").await;
    admin
        .put("/api/v1/unifi/phone-network", json!({"enabled": true}))
        .await;
    let (status, v) = admin.get("/api/v1/phones/network").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["state"], "error");
    assert_eq!(v["error"], "unreachable");
    assert_eq!(v["phones"], json!([]));
}
