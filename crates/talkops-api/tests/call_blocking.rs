mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::*;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::{blocking, tenant::TenantId};

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn whitelist_permission_grants_revocations_and_call_scope(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (_, role) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "Whitelist only", "permissions": ["blocking.allowlist"]}),
        )
        .await;
    let role_id = role["id"].as_str().unwrap();
    let (status, _) = admin.post("/api/v1/users", json!({"username": "allow", "display_name": "Allow", "password": "correct-horse", "role": "user", "role_id": role_id})).await;
    assert_eq!(status, StatusCode::OK);
    let user = login(&router, "allow", "correct-horse").await.unwrap();
    let (status, entry) = user
        .post(
            "/api/v1/call-allowlist",
            json!({"number": "+4930123456", "label": "Trusted"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{entry}");
    let id = entry["id"].as_str().unwrap();
    assert_eq!(
        user.get("/api/v1/call-allowlist")
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        user.get("/api/v1/call-blocks/settings").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        user.get("/api/v1/call-blocks").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        user.post("/api/v1/call-blocks", json!({"pattern": "+4930123456"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, duplicate) = user
        .post(
            "/api/v1/call-allowlist",
            json!({"number": "+4930123456", "label": "Changed"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(duplicate, entry);
    for number in [
        "101",
        "anonymous",
        "+4930*",
        "+0123456",
        "+1234567890123456",
    ] {
        assert_eq!(
            user.post("/api/v1/call-allowlist", json!({"number": number}))
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let (_, other_role) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "Blocks only", "permissions": ["blocking.manage"]}),
        )
        .await;
    let (_, _) = admin.post("/api/v1/users", json!({"username": "blocks", "display_name": "Blocks", "password": "correct-horse", "role": "user", "role_id": other_role["id"]})).await;
    let blocks = login(&router, "blocks", "correct-horse").await.unwrap();
    assert_eq!(
        blocks.get("/api/v1/call-allowlist").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        blocks
            .post("/api/v1/call-allowlist", json!({"number": "+4930123456"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        blocks
            .call("DELETE", &format!("/api/v1/call-allowlist/{id}"), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (_, operator) = admin.post("/api/v1/users", json!({"username": "op", "display_name": "Operator", "password": "correct-horse", "role": "operator"})).await;
    assert!(operator["id"].is_string());
    let op = login(&router, "op", "correct-horse").await.unwrap();
    assert_eq!(
        op.get("/api/v1/call-allowlist").await.0,
        StatusCode::FORBIDDEN,
        "no automatic grant to operators"
    );

    let xml = blocked_xml("protected", "list:%2B4930*");
    assert_eq!(
        fs_post(&router, "/fs/cdr", &[("cdr", &xml)]).await.0,
        StatusCode::OK
    );
    assert!(
        user.get("/api/v1/calls")
            .await
            .1
            .as_array()
            .unwrap()
            .is_empty(),
        "whitelist permission does not grant call visibility"
    );
    let (_, calls) = admin.get("/api/v1/calls").await;
    assert_eq!(calls[0]["allowlisted"], true);
    assert_eq!(calls[0]["blocked_reason"], "list:+4930*");
    let (_, calls) = op.get("/api/v1/calls").await;
    assert!(
        calls[0].get("allowlisted").is_none(),
        "membership requires the new permission"
    );
    assert_eq!(calls[0]["blocked_reason"], "list:+4930*");

    let other: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO tenants(name) VALUES ('Other') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();
    let (foreign, _) = blocking::allow(
        &db,
        &talkops_core::audit::Actor {
            tenant: TenantId(other),
            user_id: None,
            ip: None,
        },
        &blocking::CallAllowInput {
            number: "+4930999999".into(),
            label: "".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        user.call(
            "DELETE",
            &format!("/api/v1/call-allowlist/{}", foreign.id),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        user.get("/api/v1/call-allowlist")
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );

    assert_eq!(
        admin
            .put(
                &format!("/api/v1/roles/{role_id}"),
                json!({"name": "Whitelist only", "permissions": []})
            )
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        user.get("/api/v1/call-allowlist").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        user.call("DELETE", &format!("/api/v1/call-allowlist/{id}"), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        user.post("/api/v1/call-allowlist", json!({"number": "+4930888888"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        admin
            .put(
                &format!("/api/v1/roles/{role_id}"),
                json!({"name": "Whitelist only", "permissions": ["blocking.allowlist"]})
            )
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        user.call("DELETE", &format!("/api/v1/call-allowlist/{id}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, calls) = admin.get("/api/v1/calls").await;
    assert_eq!(calls[0]["allowlisted"], false);
    assert_eq!(calls[0]["blocked_reason"], "list:+4930*");
}

fn blocked_xml(uuid: &str, reason: &str) -> String {
    format!(
        r#"<cdr><variables><uuid>{uuid}</uuid>
        <talkops_tenant_id>00000000-0000-4000-8000-000000000001</talkops_tenant_id>
        <talkops_direction>inbound</talkops_direction><talkops_caller_number>%2B4930123456</talkops_caller_number>
        <talkops_destination>%2B4930999999</talkops_destination><talkops_blocked>{reason}</talkops_blocked>
        <start_epoch>1791150000</start_epoch><end_epoch>1791150000</end_epoch>
        <hangup_cause>CALL_REJECTED</hangup_cause></variables></cdr>"#
    )
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn block_reason_survives_http_spool_duplicates_and_filtered_counts(db: PgPool) {
    let state = state(db.clone());
    let router = talkops_api::app(state.clone(), None);
    let admin = setup_admin(&router).await;
    for (id, reason) in [
        ("local", "list:%2B4930123456"),
        ("prefix", "list:%2B4930*"),
        ("community", "phoneblock:12"),
        ("anonymous", "anonymous"),
        ("legacy", ""),
    ] {
        let xml = blocked_xml(id, reason);
        for _ in 0..2 {
            assert_eq!(
                fs_post(&router, "/fs/cdr", &[("cdr", &xml)]).await.0,
                StatusCode::OK
            );
        }
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("retry.xml"),
        blocked_xml("spool", "phoneblock:23"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("duplicate.xml"),
        blocked_xml("local", "phoneblock:99"),
    )
    .unwrap();
    let imported =
        talkops_api::cdr_spool::import_dir(&state, dir.path(), std::time::Duration::ZERO)
            .await
            .unwrap();
    assert_eq!(imported.done, 2);
    assert_eq!(imported.rejected, 0);
    let response = raw(
        &router,
        Request::get("/api/v1/calls?result=blocked&limit=2&offset=1&direction=inbound&from=4930")
            .header(header::COOKIE, &admin.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-total-count"], "5");
    assert_eq!(body_json(response).await.as_array().unwrap().len(), 2);
    let (_, busy) = admin.get("/api/v1/calls?result=busy").await;
    assert_eq!(busy.as_array().unwrap().len(), 1);
    assert_eq!(busy[0]["call_uuid"], "legacy");
    assert!(busy[0]["blocked_reason"].is_null());
    let (_, all) = admin.get("/api/v1/calls").await;
    assert_eq!(all.as_array().unwrap().len(), 6);
    let local = all
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["call_uuid"] == "local")
        .unwrap();
    assert_eq!(
        local["blocked_reason"], "list:+4930123456",
        "duplicate does not rewrite history"
    );
    assert_eq!(local["destination"], "+4930999999");
}
