//! Door stations against a fake Dahua VTO: ring routing with snapshot,
//! opening the door (web, feature code, token hook) and webhooks.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;

const TENANT: &str = "00000000-0000-4000-8000-000000000001";
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 7, 7, 0xFF, 0xD9];

#[derive(Default)]
struct Fake {
    opened: Mutex<Vec<String>>,
    hooks: Mutex<Vec<Value>>,
}

/// Any Digest answer is accepted; the client library is tested elsewhere.
fn needs_auth(headers: &HeaderMap) -> Option<Response> {
    (!headers.contains_key(header::AUTHORIZATION)).then(|| {
        (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                r#"Digest realm="Login to VTO", qop="auth", nonce="1", opaque="x""#,
            )],
        )
            .into_response()
    })
}

async fn fake_vto() -> (String, u16, Arc<Fake>) {
    let fake = Arc::new(Fake::default());
    let app = Router::new()
        .route(
            "/cgi-bin/magicBox.cgi",
            get(|h: HeaderMap| async move {
                needs_auth(&h).unwrap_or_else(|| "type=VTO2202F-P\r\n".into_response())
            }),
        )
        .route(
            "/cgi-bin/accessControl.cgi",
            get(
                |State(f): State<Arc<Fake>>,
                 h: HeaderMap,
                 Query(q): Query<std::collections::HashMap<String, String>>| async move {
                    if let Some(r) = needs_auth(&h) {
                        return r;
                    }
                    f.opened.lock().unwrap().push(q["channel"].clone());
                    "OK\r\n".into_response()
                },
            ),
        )
        .route(
            "/cgi-bin/snapshot.cgi",
            get(|h: HeaderMap| async move {
                needs_auth(&h).unwrap_or_else(|| {
                    let mut body = JPEG.to_vec();
                    body.extend_from_slice(b"dhav1234");
                    body.into_response()
                })
            }),
        )
        .route(
            "/hook",
            post(
                |State(f): State<Arc<Fake>>, axum::Json(v): axum::Json<Value>| async move {
                    f.hooks.lock().unwrap().push(v);
                    StatusCode::OK
                },
            ),
        )
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    ("127.0.0.1".into(), port, fake)
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

async fn call(router: &Router, ext: &Value, dest: &str) -> Vec<(String, String)> {
    let (status, xml) = fs_post(
        router,
        "/fs/xml",
        &[
            ("section", "dialplan"),
            ("Caller-Context", "internal"),
            ("Caller-Destination-Number", dest),
            ("variable_talkops_tenant_id", TENANT),
            ("variable_talkops_extension_id", ext["id"].as_str().unwrap()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    actions(&xml)
}

fn has(a: &[(String, String)], app: &str, data: &str) -> bool {
    a.iter().any(|(x, d)| x == app && d == data)
}

async fn wait_for<F: Fn(&Value) -> bool>(client: &Client, path: &str, f: F) -> Value {
    for _ in 0..50 {
        let (_, v) = client.get(path).await;
        if f(&v) {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("condition not reached for {path}");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn door_station(db: PgPool) {
    let state = state(db.clone());
    let snapshots = state.media.snapshots.clone();
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;
    let (host, port, fake) = fake_vto().await;
    let id = |v: &Value| v["id"].as_str().unwrap().to_owned();

    let (_, ext20) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "200", "display_name": "Office"}),
        )
        .await;
    admin
        .post(
            &format!("/api/v1/extensions/{}/devices", id(&ext20)),
            json!({"name": "Desk", "kind": "desk"}),
        )
        .await;
    let (_, ext21) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "210", "display_name": "Lab"}),
        )
        .await;
    let (_, door_ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "8001", "display_name": "Front door"}),
        )
        .await;
    let (status, _) = admin
        .post(
            &format!("/api/v1/extensions/{}/devices", id(&door_ext)),
            json!({"name": "VTO", "kind": "door"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Invalid input is rejected.
    let (status, _) = admin
        .post(
            "/api/v1/door-stations",
            json!({"name": "X", "extension_id": id(&door_ext), "host": "bad host"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, station) = admin
        .post(
            "/api/v1/door-stations",
            json!({"name": "Front door", "extension_id": id(&door_ext),
                   "host": host, "port": port, "password": "vto-secret",
                   "events_enabled": false,
                   "destination_type": "extension", "destination_id": id(&ext20),
                   "buttons": [{"number": "9902", "type": "extension", "id": id(&ext21)}],
                   "webhook_url": format!("http://127.0.0.1:{port}/hook")}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{station}");
    assert_eq!(station["has_password"], true);
    assert_eq!(station["has_webhook"], true);
    assert!(station.get("password").is_none() && station.get("password_enc").is_none());
    let sid = id(&station);
    let (status, _) = admin
        .post(
            "/api/v1/door-stations",
            json!({"name": "Twice", "extension_id": id(&door_ext)}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, t) = admin
        .post(&format!("/api/v1/door-stations/{sid}/test"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{t}");
    assert_eq!(t["model"], "VTO2202F-P");

    // The button rings extension 20; the second button extension 21.
    let a = call(&router, &door_ext, "9901").await;
    assert!(has(&a, "set", &format!("talkops_door_id={sid}")), "{a:?}");
    assert!(
        a.iter()
            .any(|(app, d)| app == "bridge" && d.contains("user/200-1@")),
        "{a:?}"
    );
    let a = call(&router, &door_ext, "9902#0").await;
    assert!(
        has(
            &a,
            "set",
            &format!("talkops_dest_extension_id={}", id(&ext21))
        ) || has(&a, "respond", "480 Temporarily Unavailable"),
        "{a:?}"
    );

    // Rings are logged with a snapshot (without Dahua's trailer).
    let events = wait_for(&admin, "/api/v1/door-events", |v| {
        v.as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "ring" && e["has_snapshot"] == true)
            .count()
            == 2
    })
    .await;
    let ring = &events[0];
    assert_eq!(ring["detail"]["dialed"], "9902#0");
    let res = raw(
        &router,
        Request::get(format!("/api/v1/door-events/{}/snapshot", id(ring)))
            .header(header::COOKIE, &admin.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/jpeg");
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(&bytes[..], JPEG);

    // A user opens the door from the web UI; the second lock does not exist.
    let (_, user) = admin
        .post("/api/v1/users", json!({"username": "kim", "display_name": "Kim", "role": "user", "password": "kim-password-1"}))
        .await;
    assert_eq!(user["username"], "kim");
    let kim = login(&router, "kim", "kim-password-1").await.unwrap();
    let (_, list) = kim.get("/api/v1/door-stations").await;
    assert_eq!(list[0]["host"], "", "users do not see connection details");
    let (status, _) = kim
        .post(&format!("/api/v1/door-stations/{sid}/open"), json!({}))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = kim
        .post(
            &format!("/api/v1/door-stations/{sid}/open"),
            json!({"door": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = kim
        .put(&format!("/api/v1/door-stations/{sid}"), json!({}))
        .await;
    assert!(status == StatusCode::FORBIDDEN || status == StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(*fake.opened.lock().unwrap(), ["1"]);
    let events = wait_for(&admin, "/api/v1/door-events", |v| {
        v[0]["kind"] == "open_command"
    })
    .await;
    assert_eq!(events[0]["detail"]["by"]["user"], "kim");
    assert_eq!(events[0]["detail"]["ok"], true);

    // Webhooks get every event.
    for _ in 0..50 {
        if fake.hooks.lock().unwrap().len() >= 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let hooks = fake.hooks.lock().unwrap().clone();
    assert!(
        hooks
            .iter()
            .any(|h| h["event"] == "ring" && h["has_snapshot"] == true),
        "{hooks:?}"
    );
    assert!(
        hooks.iter().any(|h| h["event"] == "open_command"),
        "{hooks:?}"
    );
    assert_eq!(hooks[0]["door_station"]["name"], "Front door");

    // Feature codes: *85 (first station), *85<number>; *86 needs a second lock.
    let a = call(&router, &ext20, "*85").await;
    assert!(has(&a, "set", "talkops_app=door_open"), "{a:?}");
    assert!(has(&a, "set", &format!("talkops_open_door_id={sid}")));
    assert!(
        !a.iter().any(|(_, d)| d.starts_with("talkops_door_id")),
        "not a ring"
    );
    let a = call(&router, &ext20, "*858001").await;
    assert!(has(&a, "set", "talkops_door=1"), "{a:?}");
    let a = call(&router, &ext20, "*86").await;
    assert!(has(&a, "respond", "404 Not Found"), "{a:?}");
    let a = call(&router, &ext20, "*859999").await;
    assert!(has(&a, "respond", "404 Not Found"), "{a:?}");

    // Token hook for Home Assistant.
    let hook = |token: Option<String>| {
        let mut req = Request::post(format!("/hooks/door/{sid}/open"))
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        req.body(Body::from(r#"{"door":1}"#)).unwrap()
    };
    assert_eq!(
        raw(&router, hook(None)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let (_, token) = admin
        .post(&format!("/api/v1/door-stations/{sid}/token"), json!({}))
        .await;
    let token = token["token"].as_str().unwrap().to_owned();
    assert_eq!(
        raw(&router, hook(Some("wrong".into()))).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        raw(&router, hook(Some(token.clone()))).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(fake.opened.lock().unwrap().len(), 2);
    admin
        .call(
            "DELETE",
            &format!("/api/v1/door-stations/{sid}/token"),
            None,
        )
        .await;
    assert_eq!(
        raw(&router, hook(Some(token))).await.status(),
        StatusCode::UNAUTHORIZED
    );

    // Deleting the station removes its events and snapshots; the extension stays.
    let files: Vec<String> =
        sqlx::query_scalar("SELECT snapshot FROM door_events WHERE snapshot IS NOT NULL")
            .fetch_all(&db)
            .await
            .unwrap();
    assert!(snapshots.join(&files[0]).is_file());
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/door-stations/{sid}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!snapshots.join(&files[0]).exists());
    let (status, _) = admin
        .get(&format!("/api/v1/extensions/{}", id(&door_ext)))
        .await;
    assert_eq!(status, StatusCode::OK);
    // Calls from the former door station are normal calls again.
    let a = call(&router, &door_ext, "9901").await;
    assert!(
        !a.iter().any(|(_, d)| d.starts_with("talkops_door_id")),
        "{a:?}"
    );
}
