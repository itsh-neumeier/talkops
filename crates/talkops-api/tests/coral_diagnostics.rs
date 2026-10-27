//! Export permission, tenant and redaction checks with a local Coral double.
mod common;
use axum::{Json, Router, extract::Query, http::StatusCode, routing::get};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::tenant::TenantId;

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn coral_support_report_is_scoped_redacted_and_permission_checked(db: PgPool) {
    let service = Router::new().route("/v1/diagnostics", get(|Query(query): Query<std::collections::HashMap<String, String>>| async move {
        assert_eq!(query.get("scope").unwrap(), &TenantId::DEFAULT.to_string());
        Json(json!({"capture": {"running": true, "events": [{"text": "private conversation", "details": {"error": "debug-secret rejected", "api_key": "debug-secret"}}]}, "health": {"status": "ok"}}))
    })).route("/v1/diagnostics/capture", axum::routing::post(|Json(input): Json<Value>| async move {
        assert_eq!(input["scope"], TenantId::DEFAULT.to_string());
        Json(json!({"running": true}))
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, service).await.unwrap() });
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, provider) = admin.post("/api/v1/settings/transcription/providers", json!({"kind":"coral", "name":"Coral", "url":format!("http://127.0.0.1:{port}/v1"), "key":"debug-secret"})).await;
    let id = provider["id"].as_str().unwrap();
    let (_, connections) = admin.get("/api/v1/diagnostics/coral").await;
    assert_eq!(connections[0]["id"], id);
    assert!(connections[0].get("url").is_none() && connections[0].get("has_key").is_none());
    let (_, report) = admin
        .get(&format!(
            "/api/v1/diagnostics/coral/{id}/download?scope=other-tenant"
        ))
        .await;
    assert!(!report.to_string().contains("debug-secret"));
    assert!(!report.to_string().contains("private conversation"));
    assert_eq!(
        report["coral"]["capture"]["events"][0]["details"]["error"],
        "[REDACTED] rejected"
    );
    let (_, report) = admin
        .get(&format!(
            "/api/v1/diagnostics/coral/{id}/download?include_text=true"
        ))
        .await;
    assert_eq!(
        report["coral"]["capture"]["events"][0]["text"],
        "private conversation"
    );
    let (_, role) = admin
        .post(
            "/api/v1/roles",
            json!({"name":"Technical support", "permissions":["diagnostics.manage"]}),
        )
        .await;
    admin.post("/api/v1/users", json!({"username":"support", "display_name":"Support", "role":"user", "role_id":role["id"], "password":"support-password-123"})).await;
    let support = login(&router, "support", "support-password-123")
        .await
        .unwrap();
    assert_eq!(
        support
            .get(&format!("/api/v1/diagnostics/coral/{id}/download"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        support
            .get(&format!(
                "/api/v1/diagnostics/coral/{id}/download?include_text=true"
            ))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        support
            .post(
                &format!("/api/v1/diagnostics/coral/{id}/capture"),
                json!({"minutes":5, "include_text":true})
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        support
            .post(
                &format!("/api/v1/diagnostics/coral/{id}/capture"),
                json!({"minutes":5})
            )
            .await
            .0,
        StatusCode::OK
    );
    let unknown = uuid::Uuid::new_v4();
    assert_eq!(
        admin
            .get(&format!("/api/v1/diagnostics/coral/{unknown}/download"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        admin
            .post(
                &format!("/api/v1/diagnostics/coral/{id}/capture"),
                json!({"minutes":31})
            )
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
