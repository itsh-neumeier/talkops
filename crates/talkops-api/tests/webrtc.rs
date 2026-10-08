//! Browser softphone account: SIP credentials and TURN servers.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::*;

async fn account(state: talkops_api::AppState) -> (StatusCode, serde_json::Value, String) {
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;
    let (_, me) = admin.get("/api/v1/auth/me").await;
    let (status, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "20", "display_name": "Office", "user_id": me["user"]["id"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ext}");
    let (status, acc) = admin.post("/api/v1/me/webrtc", json!({})).await;
    (status, acc, me["user"]["id"].as_str().unwrap().to_owned())
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn softphone_account_without_turn(db: PgPool) {
    let (status, acc, _) = account(state(db)).await;
    assert_eq!(status, StatusCode::OK, "{acc}");
    assert_eq!(acc["ws_path"], "/api/v1/webrtc/ws");
    assert_eq!(acc["ice_servers"], json!([]));
    assert_eq!(acc["relay_only"], false);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn softphone_account_with_turn(db: PgPool) {
    let state = state(db).with_turn(
        vec!["turn:pbx.example.com:3478?transport=udp".into(), " ".into()],
        Some("s3cret".into()),
        true,
    );
    let (status, acc, user_id) = account(state).await;
    assert_eq!(status, StatusCode::OK, "{acc}");
    let server = &acc["ice_servers"][0];
    assert_eq!(
        server["urls"],
        json!(["turn:pbx.example.com:3478?transport=udp"])
    );
    let username = server["username"].as_str().unwrap();
    let (expiry, user) = username.split_once(':').unwrap();
    let expiry: u64 = expiry.parse().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(expiry > now + 23 * 3600 && expiry <= now + 24 * 3600 + 5);
    assert_eq!(user, user_id);
    let (_, expected) =
        talkops_core::turn::credentials("s3cret", user, expiry - 24 * 3600, 24 * 3600);
    assert_eq!(server["credential"], expected);
    assert_eq!(acc["relay_only"], true);

    // Without a secret, TURN stays off.
    let off = state_without_secret();
    assert!(off.turn.is_none());
}

fn state_without_secret() -> talkops_api::AppState {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/none")
        .unwrap();
    state(pool).with_turn(vec!["turn:x:3478".into()], Some(String::new()), false)
}
