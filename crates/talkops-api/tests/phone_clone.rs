mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::{crypto::SecretBox, tenant::TenantId};
use uuid::Uuid;

async fn source(admin: &Client) -> Value {
    let (status, phone) = admin.post("/api/v1/phones", json!({
        "mac":"805ec0010203", "name":"Source", "model":"t41p",
        "settings":{},
        "line_keys":[{"key":1,"type":"blf","value":"102","label":"Office","account":1}],
        "provisioning":{"language":"de","time_zone":"Europe/Berlin","ntp_server":"time.example.org","endpoint":"http://pbx.example/provisioning","firmware_profile":"yealink-v83",
        "keys":[{"group":"programmable","key":18,"type":"xml_browser","value":"https://apps.example/phone?token=private","label":"App","account":0}]}
    })).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    phone
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn copy_preserves_configuration_and_source_but_generates_distinct_sip_identity(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let phone = source(&admin).await;
    let id = phone["id"].as_str().unwrap();
    let (_, extension) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"101","display_name":"Shared office"}),
        )
        .await;
    let extension_id = extension["id"].as_str().unwrap();
    let (_, old_identity) = admin
        .post(
            &format!("/api/v1/extensions/{extension_id}/devices"),
            json!({"name":"Old","kind":"desk","phone_id":id}),
        )
        .await;
    sqlx::query(
        "UPDATE phones SET last_seen_at=now(),last_ip='192.0.2.3',last_firmware='old' WHERE id=$1",
    )
    .bind(Uuid::parse_str(id).unwrap())
    .execute(&db)
    .await
    .unwrap();
    let (status, preview) = admin
        .post(&format!("/api/v1/phones/{id}/clone-preview"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["http_exception"], true);
    assert_eq!(preview["configuration"]["mac"], "");
    assert_eq!(preview["configuration"]["provisioning"]["keys"], json!([]));
    assert!(!preview.to_string().contains("private"));
    assert!(
        !preview
            .to_string()
            .contains(old_identity["sip_password"].as_str().unwrap())
    );
    let (status, copy) = admin.post(&format!("/api/v1/phones/{id}/clone"), json!({
        "source_revision":preview["source_revision"],"name":"Copy","mac":"80:5e:c0:04:05:06",
        "assignments":[{"account_index":1,"extension_id":extension_id}]
    })).await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    assert_ne!(copy["id"], phone["id"]);
    assert_eq!(copy["mac"], "805ec0040506");
    assert_eq!(copy["settings"], phone["settings"]);
    assert_eq!(copy["provisioning"]["language"], "de");
    assert_eq!(copy["provisioning"]["ntp_server"], "time.example.org");
    assert_eq!(copy["line_keys"], phone["line_keys"]);
    for field in ["last_seen_at", "last_ip", "last_firmware"] {
        assert!(copy[field].is_null());
    }
    let (_, detail) = admin
        .get(&format!("/api/v1/phones/{}", copy["id"].as_str().unwrap()))
        .await;
    assert_eq!(detail["accounts"][0]["extension_id"], extension_id);
    assert_eq!(detail["accounts"][0]["phone_label"], "");
    let copy_identity: (String, String) =
        sqlx::query_as("SELECT sip_username,sip_password_enc FROM devices WHERE phone_id=$1")
            .bind(Uuid::parse_str(copy["id"].as_str().unwrap()).unwrap())
            .fetch_one(&db)
            .await
            .unwrap();
    assert_ne!(
        copy_identity.0,
        old_identity["sip_username"].as_str().unwrap()
    );
    assert_ne!(
        SecretBox::from_hex(KEY)
            .unwrap()
            .decrypt(&copy_identity.1)
            .unwrap(),
        old_identity["sip_password"].as_str().unwrap()
    );
    let (_, unchanged) = admin.get(&format!("/api/v1/phones/{id}")).await;
    assert_eq!(unchanged["line_keys"], phone["line_keys"]);
    assert_eq!(unchanged["provisioning"], phone["provisioning"]);
    let audit: Value = sqlx::query_scalar(
        "SELECT details FROM audit_log WHERE entity_type='phone' AND action='clone'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audit["source_id"], id);
    assert!(!audit.to_string().contains("password"));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn stale_preview_and_duplicate_identity_do_not_create_a_partial_copy(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let phone = source(&admin).await;
    let id = phone["id"].as_str().unwrap();
    let path = format!("/api/v1/phones/{id}/clone");
    let (_, preview) = admin.post(&format!("{path}-preview"), json!({})).await;
    let mut input =
        json!({"source_revision":preview["source_revision"],"name":"Copy","mac":"805ec0010203"});
    assert_eq!(
        admin.post(&path, input.clone()).await.0,
        StatusCode::CONFLICT
    );
    sqlx::query(
        "UPDATE phones SET provisioning=jsonb_set(provisioning,'{language}',$2) WHERE id=$1",
    )
    .bind(Uuid::parse_str(id).unwrap())
    .bind(json!("en"))
    .execute(&db)
    .await
    .unwrap();
    input["mac"] = json!("805ec0040506");
    assert_eq!(
        admin.post(&path, input.clone()).await.0,
        StatusCode::CONFLICT
    );
    let (_, preview) = admin.post(&format!("{path}-preview"), json!({})).await;
    input["source_revision"] = preview["source_revision"].clone();
    input["assignments"] = json!([{"account_index":1,"extension_id":Uuid::new_v4()}]);
    assert_eq!(admin.post(&path, input).await.0, StatusCode::NOT_FOUND);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM phones")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action='clone'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(audit, 0);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn copy_rejects_foreign_sources_references_assignments_and_unknown_fields(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let phone = source(&admin).await;
    let id = phone["id"].as_str().unwrap();
    let other = TenantId(Uuid::new_v4());
    sqlx::query("INSERT INTO tenants(id,name) VALUES($1,'Other')")
        .bind(other)
        .execute(&db)
        .await
        .unwrap();
    let foreign: Uuid=sqlx::query_scalar("INSERT INTO phones(tenant_id,mac,model,name) VALUES($1,'805ec0000001','t41p','Other') RETURNING id")
        .bind(other).fetch_one(&db).await.unwrap();
    assert_eq!(
        admin
            .post(
                &format!("/api/v1/phones/{foreign}/clone-preview"),
                json!({})
            )
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, preview) = admin
        .post(&format!("/api/v1/phones/{id}/clone-preview"), json!({}))
        .await;
    let path = format!("/api/v1/phones/{id}/clone");
    let mut input = json!({"source_revision":preview["source_revision"],"name":"Copy","mac":"805ec0040506","model":"t54w"});
    assert_eq!(
        admin.post(&path, input.clone()).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    input.as_object_mut().unwrap().remove("model");
    input["assignments"] = json!([{"account_index":1,"extension_id":Uuid::new_v4()},{"account_index":1,"extension_id":Uuid::new_v4()}]);
    assert_eq!(
        admin.post(&path, input.clone()).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    input["assignments"] = json!([{"account_index":7,"extension_id":Uuid::new_v4()}]);
    assert_eq!(
        admin.post(&path, input).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    sqlx::query("UPDATE phones SET phonebook_sections=$2 WHERE id=$1")
        .bind(Uuid::parse_str(id).unwrap())
        .bind(vec![Uuid::new_v4()])
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        admin.post(&format!("{path}-preview"), json!({})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn copy_requires_phone_management_and_separate_extension_permission(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let phone = source(&admin).await;
    let id = phone["id"].as_str().unwrap();
    admin.post("/api/v1/users",json!({"username":"phoneadmin","display_name":"Phone admin","password":"correct-horse","role":"user"})).await;
    let user = login(&router, "phoneadmin", "correct-horse").await.unwrap();
    let path = format!("/api/v1/phones/{id}/clone");
    assert_eq!(
        user.post(&format!("{path}-preview"), json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query(
        "UPDATE roles SET permissions=ARRAY['phones.manage'] WHERE tenant_id=$1 AND builtin='user'",
    )
    .bind(TenantId::DEFAULT)
    .execute(&db)
    .await
    .unwrap();
    let (status, preview) = user.post(&format!("{path}-preview"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let mut input = json!({"source_revision":preview["source_revision"],"name":"Copy","mac":"805ec0040506","assignments":[{"account_index":1,"extension_id":Uuid::new_v4()}]});
    assert_eq!(
        user.post(&path, input.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    input["assignments"] = json!([]);
    let (status, result) = user.post(&path, input).await;
    assert_eq!(status, StatusCode::OK, "{result}");
}
