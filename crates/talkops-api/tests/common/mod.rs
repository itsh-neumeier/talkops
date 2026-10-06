//! Shared helpers for HTTP integration tests.
#![allow(dead_code)]

use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use talkops_api::fsxml::sofia::ProfileSettings;
use talkops_api::{AppState, app};
use talkops_core::crypto::SecretBox;
use talkops_core::presets::PresetCatalog;
use talkops_provisioning::PhoneCatalog;
use tower::ServiceExt;

pub const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
pub const XMLCURL_AUTH: &str = "Basic dGFsa29wczpzM2NyZXQ="; // talkops:s3cret

pub fn state(db: PgPool) -> AppState {
    let catalog = PresetCatalog::load_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/trunks"),
    )
    .unwrap();
    let phones =
        PhoneCatalog::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/phones"))
            .unwrap();
    let dir = std::env::temp_dir().join(format!("talkops-test-{}", uuid::Uuid::new_v4()));
    AppState::new(
        db,
        SecretBox::from_hex(KEY).unwrap(),
        catalog,
        phones,
        dir.clone(),
        ProfileSettings::default(),
        "s3cret",
    )
    .with_media(talkops_api::MediaPaths {
        voicemail: dir.join("voicemail"),
        sounds: dir.join("sounds"),
        recordings: dir.join("recordings"),
    })
}

pub fn router(db: PgPool) -> Router {
    app(state(db), None)
}

/// A logged-in browser session.
pub struct Client {
    pub router: Router,
    pub cookie: String,
    pub csrf: String,
}

pub async fn body_json(res: axum::response::Response) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    }
}

pub async fn body_string(res: axum::response::Response) -> String {
    String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
}

pub async fn raw(router: &Router, req: Request<Body>) -> axum::response::Response {
    router.clone().oneshot(req).await.unwrap()
}

/// Runs the first-run setup and returns the admin session.
pub async fn setup_admin(router: &Router) -> Client {
    let res = raw(
        router,
        Request::post("/api/v1/setup")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-requested-with", "TalkOps")
            .body(Body::from(
                r#"{"username":"admin","display_name":"Admin","password":"correct-horse"}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    client_from(router, res).await
}

pub async fn login(router: &Router, username: &str, password: &str) -> Option<Client> {
    let res = raw(
        router,
        Request::post("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-requested-with", "TalkOps")
            .body(Body::from(
                serde_json::json!({"username": username, "password": password}).to_string(),
            ))
            .unwrap(),
    )
    .await;
    (res.status() == StatusCode::OK).then_some(())?;
    Some(client_from(router, res).await)
}

async fn client_from(router: &Router, res: axum::response::Response) -> Client {
    let cookie = res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let json = body_json(res).await;
    Client {
        router: router.clone(),
        cookie,
        csrf: json["csrf_token"].as_str().unwrap().to_owned(),
    }
}

impl Client {
    pub async fn call(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::COOKIE, &self.cookie)
            .header("x-requested-with", "TalkOps")
            .header("x-csrf-token", &self.csrf);
        let body = match body {
            Some(v) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let res = raw(&self.router, req.body(body).unwrap()).await;
        let status = res.status();
        (status, body_json(res).await)
    }

    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.call("GET", path, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call("POST", path, Some(body)).await
    }

    pub async fn put(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call("PUT", path, Some(body)).await
    }
}

/// POSTs a form to a FreeSWITCH endpoint with valid credentials.
pub async fn fs_post(router: &Router, path: &str, form: &[(&str, &str)]) -> (StatusCode, String) {
    let body = form
        .iter()
        .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let res = raw(
        router,
        Request::post(path)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::AUTHORIZATION, XMLCURL_AUTH)
            .body(Body::from(body))
            .unwrap(),
    )
    .await;
    let status = res.status();
    (status, body_string(res).await)
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
