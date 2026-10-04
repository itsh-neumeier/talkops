//! HTTP-level tests against the router with a real (per-test) database.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use sqlx::PgPool;
use talkops_api::esl::EslHandle;
use talkops_api::{AppState, app};
use tower::ServiceExt;

fn state(db: PgPool) -> AppState {
    AppState {
        db,
        esl: EslHandle::default(),
        xmlcurl_password: "s3cret".into(),
    }
}

async fn body_string(res: axum::response::Response) -> String {
    String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn health_and_readiness(db: PgPool) {
    let router = app(state(db), None);
    for path in [
        "/healthz",
        "/readyz",
        "/api/v1/status",
        "/api/v1/openapi.json",
    ] {
        let res = router
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
    }

    let res = router
        .oneshot(Request::get("/api/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(json["database"]["ok"], true);
    assert_eq!(json["freeswitch"]["ok"], false);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn xml_curl_requires_auth_and_answers_not_found(db: PgPool) {
    let router = app(state(db), None);
    let form = "section=directory&key_name=domain&key_value=example.com";

    let res = router
        .clone()
        .oneshot(
            Request::post("/fs/xml")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // "talkops:s3cret"
    let res = router
        .oneshot(
            Request::post("/fs/xml")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::AUTHORIZATION, "Basic dGFsa29wczpzM2NyZXQ=")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        body_string(res)
            .await
            .contains(r#"<result status="not found"/>"#)
    );
}
