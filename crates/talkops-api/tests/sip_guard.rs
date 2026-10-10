//! SIP login protection: settings, bans and their effect on directory
//! lookups; FreeSWITCH endpoints only for local peers.

mod common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::sip_guard;
use talkops_core::tenant::TenantId;

async fn lookup(router: &axum::Router, user: &str, ip: &str) -> bool {
    let (status, xml) = fs_post(
        router,
        "/fs/xml",
        &[
            ("section", "directory"),
            ("action", "sip_auth"),
            ("user", user),
            ("ip", ip),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    !xml.contains("not found")
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn bans_block_directory_lookups(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "300", "display_name": "Guard"}),
        )
        .await;
    let (status, dev) = admin
        .post(
            &format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap()),
            json!({"name": "Desk", "kind": "desk"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{dev}");
    let user = dev["sip_username"].as_str().unwrap();

    let (status, s) = admin.get("/api/v1/security/sip").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["settings"]["enabled"], true);
    assert_eq!(s["settings"]["max_failures"], 10);

    let input = |nets: serde_json::Value| {
        json!({"enabled": true, "max_failures": 5, "window_minutes": 10,
               "ban_minutes": 60, "trusted_networks": nets})
    };
    let (status, _) = admin
        .put("/api/v1/security/sip", input(json!(["not-an-ip"])))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, s) = admin
        .put(
            "/api/v1/security/sip",
            input(json!(["192.168.0.0/16", "10.1.2.3", " "])),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["trusted_networks"], json!(["192.168.0.0/16", "10.1.2.3"]));

    assert!(lookup(&router, user, "203.0.113.9").await);
    let t = TenantId::DEFAULT;
    let attacker = "203.0.113.9".parse().unwrap();
    sip_guard::ban(&db, t, attacker, 5, "300-1", 60)
        .await
        .unwrap();
    // Trusted networks are never banned, even with a stored ban.
    sip_guard::ban(&db, t, "192.168.7.7".parse().unwrap(), 5, "x", 60)
        .await
        .unwrap();
    assert!(!lookup(&router, user, "203.0.113.9").await, "banned");
    assert!(lookup(&router, user, "203.0.113.10").await);
    assert!(lookup(&router, user, "192.168.7.7").await, "trusted");

    let (_, s) = admin.get("/api/v1/security/sip").await;
    let bans = s["bans"].as_array().unwrap();
    assert_eq!(bans.len(), 2);
    assert!(
        bans.iter()
            .any(|b| b["ip"] == "203.0.113.9" && b["last_user"] == "300-1")
    );

    let (status, _) = admin
        .call("DELETE", "/api/v1/security/sip/bans/203.0.113.9", None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(lookup(&router, user, "203.0.113.9").await, "unbanned");
    let (status, _) = admin
        .call("DELETE", "/api/v1/security/sip/bans/203.0.113.9", None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Disabled protection: stored bans no longer apply.
    sip_guard::ban(&db, t, attacker, 5, "x", 60).await.unwrap();
    let mut off = input(json!([]));
    off["enabled"] = json!(false);
    admin.put("/api/v1/security/sip", off).await;
    assert!(lookup(&router, user, "203.0.113.9").await);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn freeswitch_endpoints_only_for_local_peers(db: PgPool) {
    let router = router(db.clone());
    let post = |peer: &str, forwarded: bool| {
        let mut req = Request::post("/fs/xml")
            .header(header::AUTHORIZATION, XMLCURL_AUTH)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        if forwarded {
            req = req.header("x-forwarded-for", "198.51.100.1");
        }
        let mut req = req.body(Body::from("section=directory&user=x")).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        req
    };
    let res = raw(&router, post("127.0.0.1:40000", false)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = raw(&router, post("192.0.2.5:40000", false)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = raw(&router, post("127.0.0.1:40000", true)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "relayed by a proxy");

    // FreeSWITCH on its own address (macvlan) with TALKOPS_FS_PEERS.
    let router = talkops_api::app(
        state(db).with_fs_peers(vec![sip_guard::parse_network("192.0.2.0/28").unwrap()]),
        None,
    );
    let res = raw(&router, post("192.0.2.5:40000", false)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = raw(&router, post("192.0.2.17:40000", false)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
