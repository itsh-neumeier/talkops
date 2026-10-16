//! First-setup authorization and Portainer import integration tests.
mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use serde_json::json;
use sqlx::PgPool;
use std::collections::BTreeMap;
use talkops_api::backup::BackupConfig;

fn cfg(root: &std::path::Path) -> BackupConfig {
    BackupConfig {
        database_url: String::new(),
        dir: root.join("backups"),
        pg_bin_dir: None,
        volumes: vec![],
        password: Some("separate-long-backup-password".to_owned().into()),
        deployment_dir: None,
        deployment_files: vec![],
        deployment_key: Some(KEY.to_owned().into()),
        project_name: "talkops".into(),
        profiles: String::new(),
        restore_max_bytes: 1 << 20,
        rclone: talkops_api::rclone::Rclone::from_env(),
        target_roots: vec![],
    }
}
fn archive() -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    talkops_api::recovery::append_bytes(&mut tar, "database.dump", b"PGDMP-test").unwrap();
    talkops_api::recovery::append_bytes(&mut tar,"manifest.json",br#"{"format":1,"talkops_version":"1.7.5","created_at":"2026-10-12T00:00:00Z","schema_version":0,"volumes":[]}"#).unwrap();
    tar.into_inner().unwrap().finish().unwrap()
}
fn upload(data: &[u8], token: &str, csrf: bool) -> Request<Body> {
    let mut bytes = b"--test\r\nContent-Disposition: form-data; name=\"archive\"; filename=\"backup.tar.gz\"\r\nContent-Type: application/gzip\r\n\r\n".to_vec();
    bytes.extend_from_slice(data);
    bytes.extend_from_slice(b"\r\n--test--\r\n");
    let mut req = Request::post("/api/v1/setup/restore/inspect")
        .header("content-type", "multipart/form-data; boundary=test")
        .header("x-talkops-setup-token", token);
    if csrf {
        req = req.header("x-requested-with", "TalkOps");
    }
    req.body(Body::from(bytes)).unwrap()
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn first_setup_inspection_is_local_authorized_and_closes_after_setup(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let token = "local-setup-token-with-32-characters";
    let router = talkops_api::app(
        state(db.clone())
            .with_backup(cfg(root.path()))
            .with_setup_token(Some(token)),
        None,
    );
    assert_eq!(
        raw(&router, upload(&archive(), "wrong", true))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        raw(&router, upload(&archive(), token, false))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = raw(&router, upload(&archive(), token, true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let result = body_json(response).await;
    assert_eq!(result["manifest"]["format"], 1);
    assert!(result["deployment_bundle"].is_null());
    let admin = setup_admin(&router).await;
    assert_eq!(
        raw(&router, upload(&archive(), token, true)).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(admin.get("/api/v1/backups").await.0, StatusCode::OK);
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn concurrent_setup_is_serialized(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let token = "local-setup-token-with-32-characters";
    let router = talkops_api::app(
        state(db.clone())
            .with_backup(cfg(root.path()))
            .with_setup_token(Some(token)),
        None,
    );
    let mut tx = db.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(7231001)")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(
        raw(&router, upload(&archive(), token, true)).await.status(),
        StatusCode::CONFLICT
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        raw(&router, upload(b"broken", token, true)).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    setup_admin(&router).await; // A failed inspection must release the setup lock.
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn deployment_import_is_admin_only_encrypted_and_audited(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let config = cfg(root.path());
    let router = talkops_api::app(state(db).with_backup(config.clone()), None);
    let admin = setup_admin(&router).await;
    let input = json!({"files": {".env":format!("TALKOPS_SECRET_KEY={KEY}\nPOSTGRES_PASSWORD=test\nTALKOPS_ESL_PASSWORD=test\nTALKOPS_XMLCURL_PASSWORD=test\n"), "docker-compose.yml":"services: {}\n"},"project_name":"talkops","profiles":"caddy"});
    let response = raw(
        &router,
        Request::put("/api/v1/backups/deployment")
            .header("content-type", "application/json")
            .header("x-requested-with", "TalkOps")
            .body(Body::from(input.to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let (status,_) = admin.post("/api/v1/users",json!({"username":"operator","display_name":"Operator","role":"operator","password":"correct-horse","email":null})).await;
    assert_eq!(status, StatusCode::OK);
    let operator = login(&router, "operator", "correct-horse").await.unwrap();
    assert_eq!(
        operator
            .put("/api/v1/backups/deployment", input.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, result) = admin.put("/api/v1/backups/deployment", input).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["complete"], true);
    let disk =
        std::fs::read_to_string(config.dir.join(talkops_api::recovery::IMPORT_FILE)).unwrap();
    assert!(!disk.contains(KEY));
    assert!(disk.starts_with("v1:"));
    let (captured, info) = talkops_api::recovery::capture(&config).unwrap();
    assert!(captured.files[".env"].contains(KEY));
    assert!(info.complete);
    assert_eq!(info.source, "imported");
    let (status, _) = admin
        .put(
            "/api/v1/backups/deployment",
            json!({"files":{"../escape.env":"x"}}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .put(
            "/api/v1/backups/deployment",
            json!({"files":{".env":"TALKOPS_BACKUP_PASSWORD=do-not-copy\n"}}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let _: BTreeMap<String, String> = captured.files;
}
