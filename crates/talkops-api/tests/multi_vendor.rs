mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn fetch(router: &axum::Router, path: &str, info: &Value) -> (StatusCode, String) {
    let auth = STANDARD.encode(format!(
        "{}:{}",
        info["username"].as_str().unwrap(),
        info["password"].as_str().unwrap()
    ));
    let response = raw(
        router,
        Request::get(path)
            .header(header::HOST, "pbx.example")
            .header("x-forwarded-proto", "https")
            .header(header::AUTHORIZATION, format!("Basic {auth}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    (response.status(), body_string(response).await)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn manufacturer_parameters_are_scoped_encrypted_redacted_and_copied_by_metadata(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let model = "cisco-spa504g";
    let (status, schema) = admin
        .get(&format!("/api/v1/phone-models/{model}/parameters"))
        .await;
    assert_eq!(status, StatusCode::OK, "{schema}");
    let fields = schema["fields"].as_array().unwrap();
    assert!(fields.len() > 100);
    let normal = fields
        .iter()
        .find(|f| {
            f["managed"] == false
                && f["kind"] == "text"
                && f["copyable"] == true
                && f["parameter"]
                    .as_str()
                    .unwrap()
                    .to_lowercase()
                    .contains("contrast")
        })
        .unwrap();
    let secret = fields
        .iter()
        .find(|f| f["managed"] == false && f["kind"] == "secret")
        .unwrap();
    let managed = fields.iter().find(|f| f["managed"] == true).unwrap();
    let url = fields
        .iter()
        .find(|f| {
            f["managed"] == false
                && f["kind"] == "text"
                && f["parameter"]
                    .as_str()
                    .unwrap()
                    .to_lowercase()
                    .contains("url")
        })
        .unwrap();
    let url_key = url["key"].as_str().unwrap();
    let normal_key = normal["key"].as_str().unwrap();
    let secret_key = secret["key"].as_str().unwrap();
    let mut input = json!({"mac":"805ec0010203","name":"Office","model":model,"provisioning":{"parameters":{normal_key:"11",secret_key:"private-value-9"},"language":"de","ntp_server":"time.example.org"}});
    input["provisioning"]["parameters"][url_key] =
        json!("https://user:private-url-secret@example.org/asset");
    let (status, phone) = admin.post("/api/v1/phones", input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    assert_eq!(phone["provisioning"]["parameters"][secret_key], "REDACTED");
    assert_eq!(phone["provisioning"]["parameters"][url_key], "REDACTED");
    let id = phone["id"].as_str().unwrap();
    let path = format!("/api/v1/phones/{id}");
    let stored: Value = sqlx::query_scalar("SELECT provisioning FROM phones WHERE id=$1")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(
        stored["parameters"][secret_key]
            .as_str()
            .unwrap()
            .starts_with("v1:")
    );
    assert!(!stored.to_string().contains("private-value-9"));
    assert!(!stored.to_string().contains("private-url-secret"));
    input["provisioning"]["parameters"][secret_key] = json!("REDACTED");
    let (status, saved) = admin.put(&path, input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, preview) = admin.post(&format!("{path}/preview"), input.clone()).await;
    assert!(!preview.to_string().contains("private-value-9"));
    let (status, clone_preview) = admin
        .post(&format!("{path}/clone-preview"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{clone_preview}");
    assert_eq!(
        clone_preview["configuration"]["provisioning"]["parameters"][normal_key],
        "11"
    );
    assert!(
        clone_preview["configuration"]["provisioning"]["parameters"]
            .get(secret_key)
            .is_none()
    );
    assert!(
        clone_preview["configuration"]["provisioning"]["parameters"]
            .get(url_key)
            .is_none()
    );
    input["provisioning"]["parameters"][managed["key"].as_str().unwrap()] = json!("wrong");
    assert_eq!(
        admin.put(&path, input.clone()).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    input["provisioning"]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove(managed["key"].as_str().unwrap());
    input["provisioning"]["parameters"]["unknown"] = json!("x");
    assert_eq!(
        admin.put(&path, input).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let (status, cfg) = fetch(
        &router,
        "/provisioning/devices/805ec0010203/805ec0010203.xml",
        &info,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cfg}");
    assert!(cfg.contains("private-value-9"));
    assert!(cfg.contains(">11</"));
    for path in [
        "/provisioning/devices/805ec0010203/SIPDefault.cnf",
        "/provisioning/devices/805ec0010203/other.xml",
        "/provisioning/805ec0010203.cfg",
    ] {
        assert_eq!(
            fetch(&router, path, &info).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    let audit: Value = sqlx::query_scalar("SELECT jsonb_agg(details) FROM audit_log")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!audit.to_string().contains("private-value-9"));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn native_files_require_auth_and_app_ids_are_independent_of_hardware_macs(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, models) = admin.get("/api/v1/phone-models").await;
    assert!(models.as_array().unwrap().len() > 300);
    assert!(
        !models
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "grandstream-wave")
    );
    let (status, phone) = admin
        .post(
            "/api/v1/phones",
            json!({"mac":"","name":"Mobile","model":"groundwire-default"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    assert_eq!(phone["mac"].as_str().unwrap().len(), 12);
    let id = phone["id"].as_str().unwrap();
    let (_, preview) = admin
        .post(&format!("/api/v1/phones/{id}/clone-preview"), json!({}))
        .await;
    let (status, copy) = admin
        .post(
            &format!("/api/v1/phones/{id}/clone"),
            json!({"source_revision":preview["source_revision"],"name":"Second app"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    assert_ne!(copy["mac"], phone["mac"]);
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let mac = phone["mac"].as_str().unwrap();
    let public = format!("/provisioning/devices/{mac}/{mac}.xml");
    let response = raw(&router, Request::get(&public).body(Body::empty()).unwrap()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        fetch(&router, &public, &info).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"101","display_name":"Office & Mobile"}),
        )
        .await;
    let (status, creds) = admin
        .post(
            &format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap()),
            json!({"name":"Mobile","kind":"softphone","phone_id":id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{creds}");
    let (status, body) = fetch(&router, &public, &info).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(creds["sip_username"].as_str().unwrap()));
    assert!(body.contains(creds["sip_password"].as_str().unwrap()));
    assert!(!body.contains("/app/provision"));
    assert_eq!(
        admin
            .post(&format!("/api/v1/phones/{id}/resync"), json!({}))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn source_baseline_account_count_is_used_by_assignment_render_clone_and_shrink_checks(
    db: PgPool,
) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let input = json!({"mac":"805ec0010203","name":"Base station","model":"aastra-480i","provisioning":{"account_slots":3}});
    let (status, phone) = admin.post("/api/v1/phones", input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let id = phone["id"].as_str().unwrap();
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"103","display_name":"Third account"}),
        )
        .await;
    let extension_path = format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap());
    let (status, credentials) = admin
        .post(
            &extension_path,
            json!({"name":"Third","kind":"desk","phone_id":id,"account_index":3}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{credentials}");
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let (status, config) = fetch(
        &router,
        "/provisioning/devices/805ec0010203/805ec0010203.cfg",
        &info,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{config}");
    assert!(config.contains(credentials["sip_username"].as_str().unwrap()));
    assert!(config.contains(credentials["sip_password"].as_str().unwrap()));
    let mut shrink = input;
    shrink["provisioning"]["account_slots"] = json!(2);
    assert_eq!(
        admin.put(&format!("/api/v1/phones/{id}"), shrink).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, preview) = admin
        .post(&format!("/api/v1/phones/{id}/clone-preview"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let (status,copy)=admin.post(&format!("/api/v1/phones/{id}/clone"),json!({"source_revision":preview["source_revision"],"name":"Other base","mac":"805ec0040506","assignments":[{"account_index":3,"extension_id":ext["id"]}]})).await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    assert_eq!(copy["provisioning"]["account_slots"], 3);
    let (_, detail) = admin
        .get(&format!("/api/v1/phones/{}", copy["id"].as_str().unwrap()))
        .await;
    assert_eq!(detail["accounts"][0]["account_index"], 3);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn native_function_codes_survive_copy_and_wait_for_a_new_account(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let (_, models) = admin.get("/api/v1/phone-models").await;
    let model = models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "fanvil-x7c")
        .unwrap();
    let function = model["provisioning_capabilities"]["key_functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["type"] == "blf_new")
        .unwrap();
    let (status,phone)=admin.post("/api/v1/phones",json!({"mac":"805ec0010203","name":"Fanvil source","model":"fanvil-x7c","provisioning":{"keys":[{"group":"line","key":1,"type":"none","native_function":function["uuid"],"value":"101","label":"Office","account":1}]}})).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let (status, config) = fetch(
        &router,
        "/provisioning/devices/805ec0010203/805ec0010203.cfg",
        &info,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{config}");
    assert!(config.contains("101@1/bc"));
    let id = phone["id"].as_str().unwrap();
    let (_, preview) = admin
        .post(&format!("/api/v1/phones/{id}/clone-preview"), json!({}))
        .await;
    let (status,copy)=admin.post(&format!("/api/v1/phones/{id}/clone"),json!({"source_revision":preview["source_revision"],"name":"Fanvil copy","mac":"805ec0040506"})).await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    let source_overrides: talkops_provisioning::keys::Overrides =
        serde_json::from_value(phone["provisioning"].clone()).unwrap();
    let copied_overrides: talkops_provisioning::keys::Overrides =
        serde_json::from_value(copy["provisioning"].clone()).unwrap();
    assert_eq!(copied_overrides.keys, source_overrides.keys);
    let public = "/provisioning/devices/805ec0040506/805ec0040506.cfg";
    let (status, config) = fetch(&router, public, &info).await;
    assert_eq!(status, StatusCode::OK, "{config}");
    assert!(!config.contains("101@1/bc"));
    let (_, extension) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"101","display_name":"Office"}),
        )
        .await;
    let (status, credentials) = admin
        .post(
            &format!(
                "/api/v1/extensions/{}/devices",
                extension["id"].as_str().unwrap()
            ),
            json!({"name":"Copy","kind":"desk","phone_id":copy["id"],"account_index":1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{credentials}");
    let (status, config) = fetch(&router, public, &info).await;
    assert_eq!(status, StatusCode::OK, "{config}");
    assert!(config.contains("101@1/bc"));
    assert!(config.contains(credentials["sip_password"].as_str().unwrap()));
}
