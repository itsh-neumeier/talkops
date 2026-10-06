//! Tests against a fake door station that checks Digest authentication the
//! way the firmware does (MD5, qop=auth).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use digest_auth::{Algorithm, AlgorithmType};

use super::*;

const USER: &str = "admin";
const PASS: &str = "door-pass";
const REALM: &str = "Login to 7G0123PAZ";
const NONCE: &str = "1866468553";

#[derive(Default)]
struct Fake {
    challenges: AtomicUsize,
    opened: std::sync::Mutex<Vec<String>>,
}

fn md5(s: &str) -> String {
    Algorithm::new(AlgorithmType::MD5, false).hash(s.as_bytes())
}

/// Validates the Digest header; `None` means "send a challenge".
fn authorized(headers: &HeaderMap, uri: &Uri) -> bool {
    let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Digest "))
    else {
        return false;
    };
    let params: HashMap<String, String> = auth
        .split(", ")
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), v.trim_matches('"').to_owned()))
        .collect();
    let p = |k: &str| params.get(k).map(String::as_str).unwrap_or_default();
    let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or_default();
    if p("username") != USER || p("uri") != path || p("nonce") != NONCE {
        return false;
    }
    let ha1 = md5(&format!("{USER}:{REALM}:{PASS}"));
    let ha2 = md5(&format!("GET:{path}"));
    let expected = md5(&format!(
        "{ha1}:{NONCE}:{}:{}:auth:{ha2}",
        p("nc"),
        p("cnonce")
    ));
    p("response") == expected
}

fn challenge(fake: &Fake) -> Response {
    fake.challenges.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::UNAUTHORIZED,
        [(
            header::WWW_AUTHENTICATE,
            format!(r#"Digest realm="{REALM}", qop="auth", nonce="{NONCE}", opaque="5ccc069c""#),
        )],
    )
        .into_response()
}

async fn magic(State(f): State<Arc<Fake>>, headers: HeaderMap, uri: Uri) -> Response {
    if !authorized(&headers, &uri) {
        return challenge(&f);
    }
    "type=VTO2202F-P\r\n".into_response()
}

async fn access(
    State(f): State<Arc<Fake>>,
    headers: HeaderMap,
    uri: Uri,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if !authorized(&headers, &uri) {
        return challenge(&f);
    }
    if q.get("action").map(String::as_str) != Some("openDoor") {
        return (StatusCode::BAD_REQUEST, "Error\r\nBad Request!\r\n").into_response();
    }
    let channel = q.get("channel").cloned().unwrap_or_default();
    if channel == "3" {
        return "Error\r\n".into_response();
    }
    f.opened.lock().unwrap().push(channel);
    "OK\r\n".into_response()
}

async fn snapshot(State(f): State<Arc<Fake>>, headers: HeaderMap, uri: Uri) -> Response {
    if !authorized(&headers, &uri) {
        return challenge(&f);
    }
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 0xFF, 0xD9];
    jpeg.extend_from_slice(b"dhav\x00\x01\x02\x03");
    ([(header::CONTENT_TYPE, "image/jpeg")], jpeg).into_response()
}

async fn events(State(f): State<Arc<Fake>>, headers: HeaderMap, uri: Uri) -> Response {
    if !authorized(&headers, &uri) {
        return challenge(&f);
    }
    let parts = [
        "--myboundary\r\nContent-Type: text/plain\r\nContent-Length: 9\r\n\r\nHeartbeat\r\n",
        "--myboundary\r\nContent-Type: text/plain\r\nContent-Length: 120\r\n\r\nCode=Invite;action=Pulse;index=0;data={\n   \"CallID\" : \"7\",\n   \"Lock",
        "Num\" : 2\n}\r\n\r\n--myboundary\r\nContent-Type: text/plain\r\n\r\nCode=DoorStatus;action=Pulse;index=0;data={ \"Status\" : \"Open\" }\r\nCode=AccessControl;action=Pulse;index=0;data={\"Method\":4,\"Name\":\"OpenDoor\"}\r\n\r\n",
        "--myboundary\r\n",
    ];
    let stream = futures_util::stream::iter(
        parts
            .into_iter()
            .map(|p| Ok::<_, std::io::Error>(bytes::Bytes::from(p))),
    );
    (
        [(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=myboundary",
        )],
        Body::from_stream(stream),
    )
        .into_response()
}

async fn fake_vto() -> (Vto, Arc<Fake>) {
    let fake = Arc::new(Fake::default());
    let app = Router::new()
        .route("/cgi-bin/magicBox.cgi", get(magic))
        .route("/cgi-bin/accessControl.cgi", get(access))
        .route("/cgi-bin/snapshot.cgi", get(snapshot))
        .route("/cgi-bin/eventManager.cgi", get(events))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let vto = Vto::new(Config {
        host: "127.0.0.1".into(),
        port,
        username: USER.into(),
        password: PASS.into(),
    })
    .unwrap();
    (vto, fake)
}

#[tokio::test]
async fn digest_requests() {
    let (vto, fake) = fake_vto().await;
    assert_eq!(vto.device_type().await.unwrap(), "VTO2202F-P");
    vto.open_door(1).await.unwrap();
    vto.open_door(2).await.unwrap();
    assert_eq!(*fake.opened.lock().unwrap(), ["1", "2"]);
    // The challenge is reused (nonce count increases).
    assert_eq!(fake.challenges.load(Ordering::SeqCst), 1);
    assert!(matches!(vto.open_door(3).await, Err(Error::Unexpected(_))));

    let jpeg = vto.snapshot().await.unwrap();
    assert_eq!(&jpeg[..], [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 0xFF, 0xD9]);

    let wrong = Vto::new(Config {
        password: "nope".into(),
        ..vto.cfg.clone()
    })
    .unwrap();
    assert!(matches!(
        wrong.device_type().await,
        Err(Error::Unauthorized)
    ));
}

#[tokio::test]
async fn event_stream() {
    let (vto, _) = fake_vto().await;
    let mut stream = vto.events(5).await.unwrap();
    let mut all = Vec::new();
    while let Some(batch) = stream.next().await.unwrap() {
        all.extend(batch);
    }
    let codes: Vec<_> = all.iter().map(|e| e.code.as_str()).collect();
    assert_eq!(codes, ["Invite", "DoorStatus", "AccessControl"]);
    assert_eq!(all[0].data["LockNum"], 2);
    assert_eq!(all[0].action, "Pulse");
    assert_eq!(all[1].data["Status"], "Open");
    assert_eq!(all[2].data["Method"], 4);
}

#[test]
fn parser_edge_cases() {
    assert_eq!(
        events::boundary_of("multipart/x-mixed-replace; boundary=myboundary").as_deref(),
        Some("myboundary")
    );
    assert_eq!(
        events::boundary_of("multipart/x-mixed-replace;boundary=\"--abc\"").as_deref(),
        Some("abc")
    );
    let mut p = EventParser::new("b");
    // Byte-wise feeding gives the same result.
    let text = "--b\r\n\r\nCode=VideoMotion;action=Start;index=0\r\n--b\r\n";
    let mut out = Vec::new();
    for byte in text.as_bytes() {
        out.extend(p.push(std::slice::from_ref(byte)));
    }
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].code, "VideoMotion");
    assert_eq!(out[0].data, serde_json::Value::Null);
    // Garbage without boundaries does not grow the buffer forever.
    let mut p = EventParser::new("b");
    for _ in 0..100 {
        assert!(p.push(&[b'x'; 4096]).is_empty());
    }
}

#[test]
fn basic_auth_encoding() {
    let vto = Vto::new(Config {
        host: "::1".into(),
        port: 80,
        username: "Aladdin".into(),
        password: "open sesame".into(),
    })
    .unwrap();
    assert_eq!(vto.base, "http://[::1]:80");
    let mut ch = Challenge::Basic;
    assert_eq!(
        vto.authorization(&mut ch, "/").unwrap(),
        "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
    );
}
