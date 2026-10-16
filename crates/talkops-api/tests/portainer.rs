//! Portainer API connection (ADR 0032) against a fake Portainer server, and
//! automatic discovery of mounted deployment files.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_api::backup::BackupConfig;

fn cfg(root: &std::path::Path, mounted: bool) -> BackupConfig {
    BackupConfig {
        database_url: String::new(),
        dir: root.join("backups"),
        pg_bin_dir: None,
        volumes: vec![],
        password: Some("separate-long-backup-password".to_owned().into()),
        deployment_dir: mounted.then(|| root.join("deployment")),
        deployment_files: vec!["auto".into()],
        deployment_key: Some(KEY.to_owned().into()),
        project_name: "talkops".into(),
        profiles: String::new(),
        restore_max_bytes: 1 << 20,
    }
}

/// A fake Portainer with one stack; records the received tokens.
async fn fake_portainer(tokens: Arc<Mutex<Vec<String>>>) -> String {
    let check = move |headers: HeaderMap| {
        let token = headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        tokens.lock().unwrap().push(token.clone());
        token == "ptr_valid"
    };
    let (c1, c2, c3) = (check.clone(), check.clone(), check);
    let app = Router::new()
        .route(
            "/api/stacks",
            get(move |h: HeaderMap| async move {
                if !c1(h) {
                    return (StatusCode::UNAUTHORIZED, Json(json!({})));
                }
                (
                    StatusCode::OK,
                    Json(json!([{"Id": 7, "Name": "talkops"}, {"Id": 2, "Name": "caddy"}])),
                )
            }),
        )
        .route(
            "/api/stacks/7",
            get(move |h: HeaderMap| async move {
                if !c2(h) {
                    return (StatusCode::UNAUTHORIZED, Json(json!({})));
                }
                (
                    StatusCode::OK,
                    Json(json!({"Id": 7, "Name": "talkops", "Env": [
                        {"name": "POSTGRES_PASSWORD", "value": "pg"},
                        {"name": "TALKOPS_SECRET_KEY", "value": KEY},
                        {"name": "TALKOPS_ESL_PASSWORD", "value": "esl"},
                        {"name": "TALKOPS_XMLCURL_PASSWORD", "value": "xml"},
                        {"name": "TALKOPS_BACKUP_PASSWORD", "value": "must-not-be-captured"}
                    ]})),
                )
            }),
        )
        .route(
            "/api/stacks/7/file",
            get(move |h: HeaderMap| async move {
                if !c3(h) {
                    return (StatusCode::UNAUTHORIZED, Json(json!({})));
                }
                (
                    StatusCode::OK,
                    Json(
                        json!({"StackFileContent": "services:\n  talkops:\n    image: talkops\n"}),
                    ),
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://127.0.0.1:{}", addr.port())
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn portainer_stack_is_captured_for_recovery(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let tokens = Arc::new(Mutex::new(Vec::new()));
    let url = fake_portainer(tokens.clone()).await;
    let router = talkops_api::app(state(db.clone()).with_backup(cfg(root.path(), false)), None);
    let admin = setup_admin(&router).await;

    let (_, overview) = admin.get("/api/v1/backups").await;
    assert!(overview["portainer"].is_null());
    assert_eq!(overview["mounted"], false);
    assert_eq!(overview["auto_discovery"], true);

    // http:// only to the local host.
    let (status, _) = admin
        .put(
            "/api/v1/backups/portainer",
            json!({"url": "http://portainer.lan:9000", "stack_id": 7, "token": "ptr_valid"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .put(
            "/api/v1/backups/portainer",
            json!({"url": url, "stack_id": 7}),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a token is required"
    );

    let (status, cert) = admin
        .post("/api/v1/backups/portainer/certificate", json!({"url": url}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(cert["cert_sha256"].is_null());

    let (status, stacks) = admin
        .post(
            "/api/v1/backups/portainer/stacks",
            json!({"url": url, "token": "ptr_valid"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{stacks}");
    assert_eq!(stacks[0]["name"], "caddy");
    assert_eq!(stacks[1]["id"], 7);
    let (status, _) = admin
        .post(
            "/api/v1/backups/portainer/stacks",
            json!({"url": url, "token": "ptr_wrong"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);

    let (status, info) = admin
        .put(
            "/api/v1/backups/portainer",
            json!({"url": url, "stack_id": 7, "token": "ptr_valid"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{info}");
    assert_eq!(info["source"], "portainer");
    assert_eq!(info["complete"], true, "{info}");
    assert_eq!(info["files"], json!([".env", "docker-compose.yml"]));

    let (_, overview) = admin.get("/api/v1/backups").await;
    assert_eq!(overview["configuration"]["source"], "portainer");
    assert_eq!(overview["portainer"]["stack_name"], "talkops");
    assert!(!overview.to_string().contains("ptr_valid"));

    // The encrypted copy has the stack, without the backup password.
    let (input, _) = talkops_api::recovery::capture(&cfg(root.path(), false)).unwrap();
    assert!(input.files["docker-compose.yml"].contains("image: talkops"));
    assert!(!input.files[".env"].contains("must-not-be-captured"));
    assert!(input.files[".env"].contains("TALKOPS_ESL_PASSWORD=esl"));
    let stored = std::fs::read_to_string(
        root.path()
            .join("backups")
            .join(talkops_api::recovery::IMPORT_FILE),
    )
    .unwrap();
    assert!(!stored.contains("POSTGRES_PASSWORD"));

    // A change without a token keeps it; a refused token is recorded.
    let (status, _) = admin
        .post("/api/v1/backups/portainer/sync", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        tokens
            .lock()
            .unwrap()
            .iter()
            .all(|t| t == "ptr_valid" || t == "ptr_wrong" || t.is_empty())
    );
    let (status, _) = admin
        .put(
            "/api/v1/backups/portainer",
            json!({"url": url, "stack_id": 7, "token": "ptr_wrong"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let (_, overview) = admin.get("/api/v1/backups").await;
    assert_eq!(
        overview["portainer"]["last_error"],
        "Portainer refused the access token"
    );
    // The last good copy stays.
    assert_eq!(overview["configuration"]["source"], "portainer");

    let audit: Vec<Value> = sqlx::query_scalar("SELECT details FROM audit_log")
        .fetch_all(&db)
        .await
        .unwrap();
    assert!(!format!("{audit:?}").contains("ptr_"));

    let (status, _) = admin
        .call("DELETE", "/api/v1/backups/portainer", None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, overview) = admin.get("/api/v1/backups").await;
    assert!(overview["portainer"].is_null());
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn mounted_compose_files_take_precedence(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("deployment");
    std::fs::create_dir_all(&dir).unwrap();
    let router = talkops_api::app(state(db.clone()).with_backup(cfg(root.path(), true)), None);
    let admin = setup_admin(&router).await;

    // An empty bind (Portainer) allows imports.
    let (_, overview) = admin.get("/api/v1/backups").await;
    assert_eq!(overview["mounted"], false);
    let import = json!({"files": {"docker-compose.yml": "services: {}\n"}, "project_name": "talkops", "profiles": ""});
    let (status, info) = admin
        .put("/api/v1/backups/deployment", import.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{info}");
    assert_eq!(info["source"], "imported");

    // Files in the Compose directory are found and take precedence.
    std::fs::write(
        dir.join(".env"),
        format!("TALKOPS_SECRET_KEY={KEY}\nPOSTGRES_PASSWORD=pg\nTALKOPS_ESL_PASSWORD=e\nTALKOPS_XMLCURL_PASSWORD=x\n"),
    )
    .unwrap();
    std::fs::write(dir.join("docker-compose.yml"), "services: {}\n").unwrap();
    std::fs::write(dir.join("docker-compose.override.yml"), "services: {}\n").unwrap();
    let (_, overview) = admin.get("/api/v1/backups").await;
    assert_eq!(overview["mounted"], true);
    assert_eq!(overview["configuration"]["source"], "mounted");
    assert_eq!(overview["configuration"]["complete"], true, "{overview}");
    assert_eq!(
        overview["configuration"]["files"],
        json!([".env", "docker-compose.override.yml", "docker-compose.yml"])
    );
    let (status, _) = admin.put("/api/v1/backups/deployment", import).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = admin
        .put(
            "/api/v1/backups/portainer",
            json!({"url": "https://127.0.0.1:9443", "stack_id": 1, "token": "ptr_x"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

/// Certificate pinning against `openssl s_server` with a self-signed certificate.
#[tokio::test]
async fn self_signed_certificate_is_pinned() {
    use std::process::{Command, Stdio};
    use talkops_api::portainer::{Api, PortainerError, probe_certificate};
    let dir = tempfile::tempdir().unwrap();
    let (key, crt) = (dir.path().join("key.pem"), dir.path().join("crt.pem"));
    let made = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=portainer.local",
            "-keyout",
        ])
        .arg(&key)
        .arg("-out")
        .arg(&crt)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !made.is_ok_and(|s| s.success()) {
        assert!(std::env::var_os("CI").is_none(), "CI must provide openssl");
        eprintln!("openssl not found, skipping");
        return;
    }
    let der = Command::new("openssl")
        .args(["x509", "-outform", "der", "-in"])
        .arg(&crt)
        .output()
        .unwrap()
        .stdout;
    use sha2::Digest;
    let expected = hex::encode(sha2::Sha256::digest(&der));
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut server = Command::new("openssl")
        .args(["s_server", "-quiet", "-www", "-accept"])
        .arg(port.to_string())
        .arg("-cert")
        .arg(&crt)
        .arg("-key")
        .arg(&key)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let url = format!("https://127.0.0.1:{port}");
    let probed = probe_certificate(&url).await;
    let wrong = "00".repeat(32);
    let pinned = Api {
        url: &url,
        token: "t",
        cert_sha256: &expected,
    };
    let unpinned = Api {
        url: &url,
        token: "t",
        cert_sha256: "",
    };
    let other = Api {
        url: &url,
        token: "t",
        cert_sha256: &wrong,
    };
    let (ok, public, mismatch) = (
        pinned.stacks().await,
        unpinned.stacks().await,
        other.stacks().await,
    );
    let _ = server.kill();
    let _ = server.wait();
    assert_eq!(probed.unwrap().as_deref(), Some(expected.as_str()));
    // TLS succeeds with the pin; the s_server page is not Portainer JSON.
    assert!(matches!(ok, Err(PortainerError::Invalid(_))), "{ok:?}");
    // Without a pin the self-signed certificate is refused.
    assert!(matches!(public, Err(PortainerError::Http(_))), "{public:?}");
    assert!(
        matches!(mismatch, Err(PortainerError::CertificateMismatch)),
        "{mismatch:?}"
    );
}
