mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use sqlx::PgPool;

use common::*;

async fn prov_get(router: &axum::Router, path: &str, creds: (&str, &str)) -> (StatusCode, String) {
    let auth = STANDARD.encode(format!("{}:{}", creds.0, creds.1));
    let req = Request::get(path)
        .header(header::AUTHORIZATION, format!("Basic {auth}"))
        .body(Body::empty())
        .unwrap();
    let res = raw(router, req).await;
    let status = res.status();
    (status, body_string(res).await)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn http_files_replace_or_add_and_are_encrypted(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (_, info) = admin.get("/api/v1/provisioning").await;
    let creds = (
        info["username"].as_str().unwrap().to_owned(),
        info["password"].as_str().unwrap().to_owned(),
    );
    let creds = (creds.0.as_str(), creds.1.as_str());

    // An extra file is served with the provisioning credentials only.
    let (status, file) = admin
        .put(
            "/api/v1/provisioning-files",
            json!({"transport":"http","path":"custom/extra.xml","content":"<a>secret-marker</a>"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{file}");
    let (status, body) = prov_get(&router, "/provisioning/custom/extra.xml", creds).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, "<a>secret-marker</a>")
    );
    let res = raw(
        &router,
        Request::get("/provisioning/custom/extra.xml")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // A file with the name of a generated file replaces it until it is removed.
    let (status, generated) = prov_get(&router, "/provisioning/y000000000044.cfg", creds).await;
    assert_eq!(status, StatusCode::OK);
    let (_, replaced) = admin
        .put(
            "/api/v1/provisioning-files",
            json!({"transport":"http","path":"y000000000044.cfg","content":"#!version:1.0.0.1\n"}),
        )
        .await;
    assert_eq!(
        prov_get(&router, "/provisioning/y000000000044.cfg", creds)
            .await
            .1,
        "#!version:1.0.0.1\n"
    );
    let id = replaced["id"].as_str().unwrap();
    let (status, view) = admin.get(&format!("/api/v1/provisioning-files/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["content"], "#!version:1.0.0.1\n");
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/provisioning-files/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        prov_get(&router, "/provisioning/y000000000044.cfg", creds)
            .await
            .1,
        generated
    );

    // Contents are not stored as plaintext.
    let stored: Vec<String> = sqlx::query_scalar("SELECT content FROM provisioning_files")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert!(!stored[0].contains("secret-marker"));
    assert!(!stored[0].contains(&STANDARD.encode("<a>secret-marker</a>")));

    // Invalid names, transports and sizes.
    for bad in [
        json!({"transport":"http","path":"../x.xml","content":"x"}),
        json!({"transport":"http","path":"firmware/x.rom","content":"x"}),
        json!({"transport":"tftp","path":"German_Germany/sp-sip.jar","content":"x"}),
        json!({"transport":"http","path":"a.xml","phone_id":uuid::Uuid::new_v4(),"content":"x"}),
        json!({"transport":"http","path":"a.xml"}),
        json!({"transport":"http","path":"a.xml","content_base64":"%%%"}),
        json!({"transport":"http","path":"a.xml","content":"x".repeat(1024 * 1024 + 1)}),
    ] {
        let (status, body) = admin.put("/api/v1/provisioning-files", bad.clone()).await;
        assert!(status.is_client_error(), "{bad}: {status} {body}");
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn tftp_files_for_all_phones_and_one_phone(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let state = state(db).with_cisco_tftp(root.path().to_owned());
    let router = talkops_api::app(state.clone(), None);
    let admin = setup_admin(&router).await;
    let input = json!({"mac":"00451d6be327","name":"Cisco desk","model":"cisco-7861-enterprise",
        "provisioning":{"sip_host":"192.0.2.1","endpoint":"http://192.0.2.1:8080/provisioning",
            "vendor_settings":{"cisco_enterprise_tftp_ip":"127.0.0.2"}}});
    let (status, phone) = admin.post("/api/v1/phones", input).await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let id = phone["id"].as_str().unwrap();

    let tones = STANDARD.encode(b"<tones/>");
    let (status, body) = admin
        .put(
            "/api/v1/provisioning-files",
            json!({"transport":"tftp","path":"g3-tones.xml","content_base64":tones}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, own) = admin
        .put(
            "/api/v1/provisioning-files",
            json!({"transport":"tftp","phone_id":id,"path":"g3-tones.xml","content":"<own/>"}),
        )
        .await;
    let (_, sep) = admin
        .put(
            "/api/v1/provisioning-files",
            json!({"transport":"tftp","phone_id":id,"path":"SEP00451D6BE327.cnf.xml",
                "content":"<device>edited</device>"}),
        )
        .await;
    let resync = format!("/api/v1/phones/{id}/resync");
    assert_eq!(admin.post(&resync, json!({})).await.0, StatusCode::OK);
    let dir = root.path().join("active/127.0.0.2");
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    // The phone's own file wins over the file for every phone.
    assert_eq!(read("g3-tones.xml"), "<own/>");
    assert_eq!(read("SEP00451D6BE327.cnf.xml"), "<device>edited</device>");
    assert!(read("dialplan.xml").contains("<DIALTEMPLATE>"));

    for file in [&own, &sep] {
        let path = format!(
            "/api/v1/provisioning-files/{}",
            file["id"].as_str().unwrap()
        );
        assert_eq!(
            admin.call("DELETE", &path, None).await.0,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(admin.post(&resync, json!({})).await.0, StatusCode::OK);
    assert_eq!(read("g3-tones.xml"), "<tones/>");
    assert!(read("SEP00451D6BE327.cnf.xml").contains("<deviceProtocol>SIP</deviceProtocol>"));

    let (status, list) = admin.get("/api/v1/provisioning-files").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
}
