mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::{phone_provisioning as store, tenant::TenantId};
use uuid::Uuid;

async fn fetch(router: &axum::Router, path: &str, info: &Value) -> (StatusCode, String) {
    let authorization = STANDARD.encode(format!(
        "{}:{}",
        info["username"].as_str().unwrap(),
        info["password"].as_str().unwrap()
    ));
    let response = raw(
        router,
        Request::get(path)
            .header(header::HOST, "secure.example")
            .header("x-forwarded-proto", "https")
            .header(
                header::USER_AGENT,
                "Yealink SIP-T41P 36.83.0.120 80:5e:c0:aa:bb:cc",
            )
            .header(header::AUTHORIZATION, format!("Basic {authorization}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    (status, body_string(response).await)
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn device_http_override_survives_https_preview_and_resync_with_diagnostics(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    assert_eq!(
        admin
            .put(
                "/api/v1/provisioning/options",
                json!({"endpoint":"https://secure.example/provisioning","sip_host":"192.168.1.10"})
            )
            .await
            .0,
        StatusCode::OK
    );
    let mut input = json!({"mac":"805ec0aabbcc","model":"t41p","name":"Desk","line_keys":[],"provisioning":{
        "endpoint":"http://192.168.1.10:8095/provisioning/","firmware_profile":"yealink-v83",
        "keys":[{"group":"programmable","key":18,"type":"xml_phonebook","phonebook":1,"account":0,"label":"Contacts"}],
        "softkeys":{"call_in":["Answer","Forward","Silence","Reject"]}
    }});
    let (status, phone) = admin.post("/api/v1/phones", input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let id = phone["id"].as_str().unwrap();
    let (_, extension) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"101","display_name":"Desk"}),
        )
        .await;
    let (status, credentials) = admin
        .post(
            &format!(
                "/api/v1/extensions/{}/devices",
                extension["id"].as_str().unwrap()
            ),
            json!({"name":"T41P","kind":"desk","phone_id":id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{credentials}");
    let (_, info) = admin.get("/api/v1/provisioning").await;
    assert_eq!(info["url"], "https://secure.example/provisioning");
    let (status, cfg) = fetch(&router, "/provisioning/805ec0aabbcc.cfg", &info).await;
    assert_eq!(status, StatusCode::OK, "{cfg}");
    assert!(cfg.contains("auto_provision.server.url = http://192.168.1.10:8095/provisioning\n"));
    assert!(!cfg.contains("https://secure.example/provisioning"));
    assert!(cfg.contains("programablekey.18.type = 22\n"));
    assert!(cfg.contains("programablekey.18.xml_phonebook = 1\n"));
    assert!(cfg.contains("/softkeys/805ec0aabbcc/call_in.xml"));
    let mut draft = input.clone();
    draft["accounts"] = json!({credentials["device_id"].as_str().unwrap():{"phone_label":"Draft label","phone_display_name":"Draft caller"}});
    let (_, preview) = admin
        .post(&format!("/api/v1/phones/{id}/preview"), draft.clone())
        .await;
    let text = preview["after"].as_str().unwrap();
    assert!(text.contains("REDACTED"));
    assert!(text.contains("account.1.label = Draft label"));
    assert!(text.contains("account.1.display_name = Draft caller"));
    assert!(!text.contains(credentials["sip_password"].as_str().unwrap()));
    let (_, saved) = admin.get(&format!("/api/v1/phones/{id}")).await;
    assert_eq!(saved["accounts"][0]["phone_label"], "");
    draft["accounts"] = json!({Uuid::new_v4().to_string():{"phone_label":"Foreign account"}});
    assert_eq!(
        admin
            .post(&format!("/api/v1/phones/{id}/preview"), draft)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for field in ["password", "phone_admin_password"] {
        assert!(!text.contains(info[field].as_str().unwrap()));
    }
    let (status, boot) = fetch(&router, "/provisioning/805ec0aabbcc.boot", &info).await;
    assert_eq!(status, StatusCode::OK);
    assert!(boot.contains("include:config \"y000000000036.cfg\""));
    assert!(boot.contains("805ec0aabbcc.cfg"));
    let (status, generic_boot) = fetch(&router, "/provisioning/y000000000000.boot", &info).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(generic_boot, boot);
    let (status, xml) = fetch(
        &router,
        "/provisioning/softkeys/805ec0aabbcc/call_in.xml",
        &info,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(xml.contains("<CallIn>"));
    let mut old_client = input.clone();
    old_client.as_object_mut().unwrap().remove("provisioning");
    old_client["name"] = json!("Renamed");
    let (status, saved) = admin.put(&format!("/api/v1/phones/{id}"), old_client).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        saved["provisioning"]["endpoint"],
        input["provisioning"]["endpoint"]
    );
    input["provisioning"]["keys"] = json!([]);
    input["provisioning"]["softkeys"] = json!({});
    assert_eq!(
        admin.put(&format!("/api/v1/phones/{id}"), input).await.0,
        StatusCode::OK
    );
    let (_, reset) = fetch(&router, "/provisioning/805ec0aabbcc.cfg", &info).await;
    assert!(reset.contains("programablekey.18.type = 0\n"));
    assert!(reset.contains("phone_setting.custom_softkey_enable = 0\n"));
    assert_eq!(
        admin
            .post(&format!("/api/v1/phones/{id}/resync"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    let (_, requests) = admin
        .get("/api/v1/provisioning/requests?mac=805ec0aabbcc")
        .await;
    assert!(
        requests
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["scheme"] == "https" && r["status"] == 200)
    );
    assert!(
        !requests
            .to_string()
            .contains(info["password"].as_str().unwrap())
    );
    for url in [
        "tftp://pbx/provisioning",
        "https://user:pass@pbx/provisioning",
        "https://pbx/wrong",
    ] {
        assert_eq!(
            admin
                .put("/api/v1/provisioning/options", json!({"endpoint":url}))
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_log WHERE action='preview'")
            .fetch_one(&db)
            .await
            .unwrap()
            > 0
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn template_drafts_activation_restore_and_source_access(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (status, library) = admin.get("/api/v1/provisioning/library").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(library["profiles"].as_array().unwrap().len(), 70);
    let response = raw(
        &router,
        Request::get("/api/v1/provisioning/library/source/t41p%2Fy000000000036.cfg")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let source = admin
        .get("/api/v1/provisioning/library/source/t41p%2Fy000000000036.cfg")
        .await;
    assert_eq!(source.0, StatusCode::OK);
    let (_, template) = admin
        .post(
            "/api/v1/provisioning/templates",
            json!({"name":"T41P custom","model":"t41p"}),
        )
        .await;
    let id = template["id"].as_str().unwrap();
    let path = format!("/api/v1/provisioning/templates/{id}");
    assert!(template["active_revision"].is_null());
    let (_, native) = admin.get("/api/v1/provisioning/native").await;
    let mut bad = native.clone();
    bad["mac.cfg"] = json!(format!(
        "{}\n{{{{ missing_value }}}}",
        native["mac.cfg"].as_str().unwrap()
    ));
    let (_, draft) = admin
        .post(&format!("{path}/revisions"), json!({"files":bad}))
        .await;
    assert_eq!(draft["revision"], 2);
    assert_eq!(
        admin
            .post(&format!("{path}/activate"), json!({"revision":2}))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        admin
            .post(&format!("{path}/activate"), json!({"revision":1}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let mut files = native;
    files["mac.cfg"] = json!(format!(
        "{}\nphone_setting.test_marker = custom\n",
        files["mac.cfg"].as_str().unwrap()
    ));
    let (_, draft) = admin
        .post(&format!("{path}/revisions"), json!({"files":files}))
        .await;
    assert_eq!(draft["revision"], 3);
    assert_eq!(
        admin
            .post(&format!("{path}/activate"), json!({"revision":3}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let input = json!({"mac":"805ec0aabbcc","model":"t41p","name":"Desk","provisioning":{"template_id":id}});
    let (status, phone) = admin.post("/api/v1/phones", input).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let (_, cfg) = fetch(&router, "/provisioning/805ec0aabbcc.cfg", &info).await;
    assert!(cfg.contains("phone_setting.test_marker = custom"));
    assert_eq!(
        admin
            .post(&format!("{path}/activate"), json!({"revision":1}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, cfg) = fetch(&router, "/provisioning/805ec0aabbcc.cfg", &info).await;
    assert!(!cfg.contains("phone_setting.test_marker"));
    let other = TenantId(Uuid::new_v4());
    sqlx::query("INSERT INTO tenants(id,name) VALUES ($1,'Other')")
        .bind(other)
        .execute(&db)
        .await
        .unwrap();
    assert!(
        store::template(&db, other, Uuid::parse_str(id).unwrap())
            .await
            .is_err()
    );
    assert!(
        store::revisions(&db, other, Uuid::parse_str(id).unwrap())
            .await
            .is_err()
    );
    assert!(
        store::add_revision(&db, other, Uuid::parse_str(id).unwrap(), &json!({}))
            .await
            .is_err()
    );
    assert!(store::templates(&db, other).await.unwrap().is_empty());
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn requests_and_options_are_tenant_scoped_and_bounded(db: PgPool) {
    let other = TenantId(Uuid::new_v4());
    sqlx::query("INSERT INTO tenants(id,name) VALUES ($1,'Other')")
        .bind(other)
        .execute(&db)
        .await
        .unwrap();
    store::set_options(&db, other, &json!({"endpoint":"http://other/provisioning"}))
        .await
        .unwrap();
    assert_ne!(
        store::options(&db, other).await.unwrap(),
        store::options(&db, TenantId::DEFAULT).await.unwrap()
    );
    store::record_request(&db, other, None, "y000000000036.cfg", "http", 401)
        .await
        .unwrap();
    sqlx::query("INSERT INTO provisioning_requests(tenant_id,filename,scheme,status) SELECT $1,'y000000000036.cfg','https',200 FROM generate_series(1,2010)").bind(TenantId::DEFAULT).execute(&db).await.unwrap();
    store::record_request(
        &db,
        TenantId::DEFAULT,
        Some("805ec0aabbcc"),
        "805ec0aabbcc.cfg",
        "http",
        200,
    )
    .await
    .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM provisioning_requests WHERE tenant_id=$1")
            .bind(TenantId::DEFAULT)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(count, 2000);
    assert_eq!(store::requests(&db, other, None).await.unwrap().len(), 1);
    assert_eq!(
        store::requests(&db, TenantId::DEFAULT, Some("805ec0aabbcc"))
            .await
            .unwrap()
            .len(),
        1
    );
}
