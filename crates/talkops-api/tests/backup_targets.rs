//! Backup targets (ADR 0031): transfers with real rclone servers (WebDAV,
//! FTP, SFTP, S3), a local target with retention, and restore from a target
//! before setup. Needs rclone (TALKOPS_RCLONE or `PATH`); the run test also
//! needs `pg_dump` (see tests/backup.rs).

mod common;

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_api::backup::BackupConfig;
use talkops_api::backup_targets;
use talkops_api::rclone::Rclone;
use talkops_core::backup_targets::{FtpTls, TargetConfig, TargetConnection, TargetSecret};

/// Backups and target runs are exclusive per process; run those tests one by one.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn rclone_bin() -> PathBuf {
    std::env::var_os("TALKOPS_RCLONE").map_or_else(|| PathBuf::from("rclone"), PathBuf::from)
}

fn rclone_available() -> bool {
    let ok = Command::new(rclone_bin())
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        assert!(std::env::var_os("CI").is_none(), "CI must provide rclone");
        eprintln!("rclone not found, skipping");
    }
    ok
}

fn pg_bin_dir() -> Option<PathBuf> {
    std::env::var_os("TALKOPS_TEST_PG_BIN_DIR").map(PathBuf::from)
}

fn pg_dump_available() -> bool {
    let tool = pg_bin_dir().map_or("pg_dump".into(), |d| d.join("pg_dump"));
    let ok = Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        assert!(std::env::var_os("CI").is_none(), "CI must provide pg_dump");
        eprintln!("pg_dump not found, skipping");
    }
    ok
}

fn test_db_url(db: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let mut url = url::Url::parse(&base).unwrap();
    url.set_path(db.connect_options().get_database().expect("test database"));
    url.to_string()
}

/// An `rclone serve` process, stopped on drop.
struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn serve(protocol: &str, dir: &Path, extra: &[&str]) -> Server {
    let port = free_port();
    let child = Command::new(rclone_bin())
        .args(["serve", protocol, "--config", "", "--addr"])
        .arg(format!("127.0.0.1:{port}"))
        .args(extra)
        .arg(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("rclone serve");
    let server = Server { child, port };
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return server;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("rclone serve {protocol} did not start");
}

fn archive_bytes() -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    talkops_api::recovery::append_bytes(&mut tar, "database.dump", b"PGDMP-test").unwrap();
    talkops_api::recovery::append_bytes(
        &mut tar,
        "manifest.json",
        br#"{"format":1,"talkops_version":"1.7.5","created_at":"2026-10-12T00:00:00Z","schema_version":0,"volumes":[]}"#,
    )
    .unwrap();
    tar.into_inner().unwrap().finish().unwrap()
}

/// Upload, list, download and delete through one connection.
async fn round_trip(rclone: &Rclone, conn: &mut TargetConnection) {
    let report = rclone.test(conn).await.expect("connection test");
    if let TargetConfig::Sftp { host_keys, .. } = &mut conn.config {
        let pinned = report.host_keys.expect("pinned host key");
        assert!(report.fingerprint.unwrap().contains("SHA256:"));
        *host_keys = pinned;
        conn.validate(false).unwrap();
    }
    let local = tempfile::tempdir().unwrap();
    let name = "talkops-20261015-033000.tar.gz.age";
    let file = local.path().join(name);
    std::fs::write(&file, b"encrypted archive bytes").unwrap();
    rclone
        .upload(conn, &file, name, Some(10_000))
        .await
        .unwrap();
    std::fs::write(local.path().join("foreign.txt"), b"x").unwrap();

    let list = backup_targets::archives(rclone, conn).await.unwrap();
    assert_eq!(list.len(), 1, "{:?}", conn.config.kind());
    assert_eq!(list[0].name, name);
    assert_eq!(list[0].size, 23);
    // No upload leftovers.
    let (entries, cut) = rclone.list(conn, "").await.unwrap();
    assert!(!cut);
    assert!(entries.iter().all(|e| !e.name.ends_with(".partial")));

    let restore = tempfile::tempdir().unwrap();
    let path = backup_targets::fetch(rclone, conn, name, restore.path(), 1 << 20)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"encrypted archive bytes");
    assert!(
        backup_targets::fetch(rclone, conn, name, restore.path(), 1 << 20)
            .await
            .is_err(),
        "an existing local archive is never replaced"
    );
    assert!(
        backup_targets::fetch(rclone, conn, name, local.path().join("x").as_path(), 5)
            .await
            .is_err(),
        "size limit"
    );
    assert!(
        backup_targets::fetch(rclone, conn, "../etc/passwd", restore.path(), 1 << 20)
            .await
            .is_err()
    );
    rclone.delete(conn, name).await.unwrap();
    assert!(
        backup_targets::archives(rclone, conn)
            .await
            .unwrap()
            .is_empty()
    );
}

fn conn(config: TargetConfig, path: &str, secret: TargetSecret) -> TargetConnection {
    let mut c = TargetConnection {
        config,
        path: path.into(),
        allow_insecure: true,
        secret,
    };
    c.validate(true).unwrap();
    c
}

fn password(p: &str) -> TargetSecret {
    TargetSecret {
        password: Some(p.into()),
        ..TargetSecret::default()
    }
}

#[tokio::test]
async fn transfers_with_webdav_ftp_sftp_and_s3() {
    if !rclone_available() {
        return;
    }
    let rclone = Rclone::new(rclone_bin());
    let auth = ["--user", "backup", "--pass", "secret pass"];

    let dir = tempfile::tempdir().unwrap();
    let server = serve("webdav", dir.path(), &auth);
    let mut c = conn(
        TargetConfig::Webdav {
            url: format!("http://127.0.0.1:{}", server.port),
            vendor: "other".into(),
            user: "backup".into(),
        },
        "talkops/server1",
        password("secret pass"),
    );
    round_trip(&rclone, &mut c).await;
    assert!(dir.path().join("talkops/server1").is_dir());
    let mut wrong = c.clone();
    wrong.secret = password("wrong");
    assert!(rclone.test(&wrong).await.is_err());
    drop(server);

    let dir = tempfile::tempdir().unwrap();
    let server = serve("ftp", dir.path(), &auth);
    let mut c = conn(
        TargetConfig::Ftp {
            host: "127.0.0.1".into(),
            port: Some(server.port),
            user: "backup".into(),
            tls: FtpTls::None,
        },
        "talkops",
        password("secret pass"),
    );
    round_trip(&rclone, &mut c).await;
    drop(server);

    let dir = tempfile::tempdir().unwrap();
    let server = serve("sftp", dir.path(), &auth);
    let mut c = conn(
        TargetConfig::Sftp {
            host: "127.0.0.1".into(),
            port: Some(server.port),
            user: "backup".into(),
            host_keys: String::new(),
        },
        "talkops",
        password("secret pass"),
    );
    round_trip(&rclone, &mut c).await;
    // A different pinned key is refused.
    let mut other = c.clone();
    if let TargetConfig::Sftp { host_keys, .. } = &mut other.config {
        *host_keys =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIMVlYXzmXZ0Kc7x4G2s2+3+0fLzqk9t1QW4F2NmYxW9g"
                .into();
    }
    assert!(rclone.list(&other, "").await.is_err());
    drop(server);

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("bucket")).unwrap();
    let server = serve("s3", dir.path(), &["--auth-key", "access,secret-key"]);
    let mut c = conn(
        TargetConfig::S3 {
            provider: "Rclone".into(),
            endpoint: format!("http://127.0.0.1:{}", server.port),
            region: String::new(),
            access_key_id: "access".into(),
            path_style: true,
        },
        "bucket/talkops",
        TargetSecret {
            secret_access_key: Some("secret-key".into()),
            ..TargetSecret::default()
        },
    );
    round_trip(&rclone, &mut c).await;
    drop(server);
}

fn base_cfg(root: &Path, db_url: String, password: bool) -> BackupConfig {
    BackupConfig {
        database_url: db_url,
        dir: root.join("backups"),
        pg_bin_dir: pg_bin_dir(),
        volumes: vec![("voicemail", root.join("voicemail"))],
        password: password.then(|| "separate-long-backup-password".to_owned().into()),
        deployment_dir: None,
        deployment_files: vec![],
        deployment_key: Some(KEY.to_owned().into()),
        project_name: "talkops".into(),
        profiles: String::new(),
        restore_max_bytes: 1 << 30,
        rclone: Rclone::new(rclone_bin()),
        target_roots: vec![root.join("targets")],
    }
}

async fn wait_done(admin: &Client) -> Value {
    for _ in 0..600 {
        let (status, body) = admin.get("/api/v1/backup-targets").await;
        assert_eq!(status, StatusCode::OK);
        let target = body["targets"][0].clone();
        if target["activity"]["done"] == true {
            return target;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("target run did not finish");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn local_target_runs_prunes_and_restores(db: PgPool) {
    let _serial = SERIAL.lock().await;
    if !rclone_available() || !pg_dump_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let usb = root.path().join("targets/usb");
    std::fs::create_dir_all(&usb).unwrap();
    std::fs::create_dir_all(root.path().join("voicemail")).unwrap();
    std::fs::write(root.path().join("voicemail/msg.wav"), b"RIFF").unwrap();
    // An old archive (removed by retention) and a foreign file (kept).
    std::fs::write(usb.join("talkops-20200101-000000.tar.gz.age"), b"old").unwrap();
    std::fs::write(usb.join("notes.txt"), b"keep me").unwrap();

    let cfg = base_cfg(root.path(), test_db_url(&db), true);
    let router = talkops_api::app(state(db.clone()).with_backup(cfg), None);
    let admin = setup_admin(&router).await;

    let mut input = json!({
        "name": "USB disk",
        "config": {"kind": "local"},
        "path": "/etc",
        "include_recordings": false,
        "schedule": {"weekdays": 0, "hour": 3, "minute": 30, "every_hours": 0},
        "retention": {"keep_last": 1, "keep_daily": 0, "keep_weekly": 0, "keep_monthly": 0, "keep_yearly": 0}
    });
    let (status, body) = admin.post("/api/v1/backup-targets", input.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    input["path"] = json!(usb.display().to_string());
    let (status, target) = admin.post("/api/v1/backup-targets", input.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{target}");
    let id = target["id"].as_str().unwrap().to_owned();
    assert_eq!(target["has_secret"], false);

    // Folder browser: the roots, then the folder.
    let (status, list) = admin
        .post(
            "/api/v1/backup-targets/browse",
            json!({"config": {"kind": "local"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(
        list["entries"][0]["name"],
        root.path().join("targets").display().to_string()
    );
    let (status, list) = admin
        .post(
            "/api/v1/backup-targets/browse",
            json!({"config": {"kind": "local"}, "path": usb.display().to_string()}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        list["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "notes.txt")
    );
    let (status, _) = admin
        .post(
            "/api/v1/backup-targets/folders",
            json!({"connection": {"config": {"kind": "local"}, "path": usb.display().to_string()}, "name": "sub"}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(usb.join("sub").is_dir());
    let (status, _) = admin
        .post(
            "/api/v1/backup-targets/folders",
            json!({"connection": {"config": {"kind": "local"}, "path": usb.display().to_string()}, "name": "../x"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .post(
            "/api/v1/backup-targets/test",
            json!({"config": {"kind": "local"}, "path": usb.display().to_string()}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = admin
        .post(&format!("/api/v1/backup-targets/{id}/run"), json!({}))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let done = wait_done(&admin).await;
    assert!(done["activity"]["error"].is_null(), "{done}");
    assert!(done["last_error"].is_null(), "{done}");
    let name = done["last_file"].as_str().unwrap().to_owned();
    assert!(name.ends_with(".tar.gz.age"));
    let mut files: Vec<String> = std::fs::read_dir(&usb)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    let mut expected = vec![name.clone(), "notes.txt".into(), "sub".into()];
    expected.sort();
    assert_eq!(files, expected);
    // The staging archive is gone; the local list stays empty.
    let (_, local) = admin.get("/api/v1/backups").await;
    assert_eq!(local["files"].as_array().unwrap().len(), 0, "{local}");

    let (status, archives) = admin
        .get(&format!("/api/v1/backup-targets/{id}/archives"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(archives.as_array().unwrap().len(), 1);

    // Download for a restore.
    let (status, _) = admin
        .post(
            &format!("/api/v1/backup-targets/{id}/archives/{name}/fetch"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let done = wait_done(&admin).await;
    assert!(done["activity"]["error"].is_null(), "{done}");
    assert!(root.path().join("backups").join(&name).is_file());
    let (_, local) = admin.get("/api/v1/backups").await;
    assert_eq!(local["files"][0]["name"], name);
    let (status, _) = admin
        .post(
            &format!("/api/v1/backup-targets/{id}/archives/{name}/fetch"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = admin
        .call(
            "DELETE",
            &format!("/api/v1/backup-targets/{id}/archives/{name}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!usb.join(&name).exists());
    let (status, _) = admin
        .call(
            "DELETE",
            &format!("/api/v1/backup-targets/{id}/archives/notes.txt"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/backup-targets/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE entity_type = 'backup_target' ORDER BY id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(actions, ["create", "run", "fetch", "delete"]);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn remote_targets_need_encryption_and_hide_secrets(db: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let input = json!({
        "name": "Nextcloud",
        "config": {"kind": "webdav", "url": "https://cloud.example.com/remote.php/dav/files/u", "vendor": "nextcloud", "user": "u"},
        "path": "Backups/TalkOps",
        "secret": {"password": "app-password-never-shown"}
    });
    let plain = talkops_api::app(
        state(db.clone()).with_backup(base_cfg(root.path(), String::new(), false)),
        None,
    );
    let admin = setup_admin(&plain).await;
    let (status, body) = admin.post("/api/v1/backup-targets", input.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("TALKOPS_BACKUP_PASSWORD")
    );

    let encrypted = talkops_api::app(
        state(db.clone()).with_backup(base_cfg(root.path(), String::new(), true)),
        None,
    );
    let admin = login(&encrypted, "admin", "correct-horse")
        .await
        .expect("admin login");
    let (status, target) = admin.post("/api/v1/backup-targets", input.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{target}");
    assert_eq!(target["has_secret"], true);
    let (_, list) = admin.get("/api/v1/backup-targets").await;
    assert!(!list.to_string().contains("app-password-never-shown"));
    assert_eq!(list["encryption_configured"], true);
    assert!(list["targets"][0]["next_run_at"].is_string());
    let audit: Vec<Value> = sqlx::query_scalar("SELECT details FROM audit_log")
        .fetch_all(&db)
        .await
        .unwrap();
    assert!(!format!("{audit:?}").contains("app-password-never-shown"));

    // Plain HTTP needs the explicit switch.
    let mut insecure = input.clone();
    insecure["name"] = json!("Plain");
    insecure["config"]["url"] = json!("http://cloud.local/dav");
    let (status, _) = admin.post("/api/v1/backup-targets", insecure.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    insecure["allow_insecure"] = json!(true);
    let (status, _) = admin.post("/api/v1/backup-targets", insecure).await;
    assert_eq!(status, StatusCode::CREATED);
}

fn setup_post(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .header("x-requested-with", "TalkOps")
        .header("x-talkops-setup-token", token)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn restore_source_before_setup(db: PgPool) {
    if !rclone_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let targets = root.path().join("targets/nas");
    std::fs::create_dir_all(&targets).unwrap();
    let name = "talkops-20261012-000000.tar.gz";
    std::fs::write(targets.join(name), archive_bytes()).unwrap();
    let token = "local-setup-token-with-32-characters";
    let router = talkops_api::app(
        state(db.clone())
            .with_backup(base_cfg(root.path(), String::new(), true))
            .with_setup_token(Some(token)),
        None,
    );
    let connection = json!({"config": {"kind": "local"}, "path": targets.display().to_string()});

    let res = raw(
        &router,
        setup_post(
            "/api/v1/setup/restore/target/browse",
            "wrong",
            connection.clone(),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = raw(
        &router,
        setup_post(
            "/api/v1/setup/restore/target/browse",
            token,
            connection.clone(),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let list = body_json(res).await;
    assert_eq!(list["entries"][0]["name"], name);

    let outside = json!({"config": {"kind": "local"}, "path": "/etc"});
    let res = raw(
        &router,
        setup_post("/api/v1/setup/restore/target/browse", token, outside),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let res = raw(
        &router,
        setup_post(
            "/api/v1/setup/restore/target/inspect",
            token,
            json!({"connection": connection, "name": "../../etc/passwd"}),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // A failed attempt releases the setup lock at once.
    for _ in 0..5 {
        let res = raw(
            &router,
            setup_post(
                "/api/v1/setup/restore/target/inspect",
                token,
                json!({"connection": connection, "name": "talkops-20990101-000000.tar.gz"}),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    let res = raw(
        &router,
        setup_post(
            "/api/v1/setup/restore/target/inspect",
            token,
            json!({"connection": connection, "name": name}),
        ),
    )
    .await;
    let status = res.status();
    let res = if status == StatusCode::OK {
        res
    } else {
        panic!("{status}: {}", body_string(res).await)
    };
    assert_eq!(res.headers()["cache-control"], "no-store");
    let result = body_json(res).await;
    assert_eq!(result["archive"], name);
    assert_eq!(result["manifest"]["talkops_version"], "1.7.5");
    assert!(root.path().join("backups").join(name).is_file());
    // Nothing about the connection is stored.
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM backup_targets")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 0);

    setup_admin(&router).await;
    let res = raw(
        &router,
        setup_post(
            "/api/v1/setup/restore/target/browse",
            token,
            connection.clone(),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // Wrong tokens are rate limited; then the correct token is blocked too.
    for _ in 0..9 {
        let res = raw(
            &router,
            setup_post(
                "/api/v1/setup/restore/target/browse",
                "wrong",
                connection.clone(),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
    let res = raw(
        &router,
        setup_post("/api/v1/setup/restore/target/browse", token, connection),
    )
    .await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn scheduler_reuses_the_daily_archive(db: PgPool) {
    let _serial = SERIAL.lock().await;
    if !rclone_available() || !pg_dump_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let usb = root.path().join("targets/usb");
    std::fs::create_dir_all(&usb).unwrap();
    let cfg = std::sync::Arc::new(base_cfg(root.path(), test_db_url(&db), true));
    let secrets = talkops_core::crypto::SecretBox::from_hex(KEY).unwrap();
    let tenant = talkops_core::tenant::TenantId::DEFAULT;
    let input: talkops_core::backup_targets::BackupTargetInput = serde_json::from_value(json!({
        "name": "USB",
        "config": {"kind": "local"},
        "path": usb.display().to_string(),
        "schedule": {"weekdays": 127, "hour": 0, "minute": 0, "every_hours": 0}
    }))
    .unwrap();
    let target = talkops_core::backup_targets::create(&db, tenant, &secrets, &input)
        .await
        .unwrap();

    // Not due: the target was created after today's planned time.
    backup_targets::tick(&cfg, &db, &secrets, tenant, None).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!backup_targets::is_running());
    assert!(
        talkops_core::backup_targets::get(&db, tenant, target.id)
            .await
            .unwrap()
            .last_run_at
            .is_none()
    );

    // Due after the creation time moves back; the fresh daily archive is used.
    sqlx::query("UPDATE backup_targets SET created_at = now() - interval '2 days'")
        .execute(&db)
        .await
        .unwrap();
    let daily = talkops_api::backup::create(&cfg, &db, true).await.unwrap();
    backup_targets::tick(&cfg, &db, &secrets, tenant, Some((daily.clone(), true))).await;
    let mut done = None;
    for _ in 0..300 {
        let t = talkops_core::backup_targets::get(&db, tenant, target.id)
            .await
            .unwrap();
        if t.last_run_at.is_some() {
            done = Some(t);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let done = done.expect("scheduled run");
    assert!(done.last_error.is_none(), "{:?}", done.last_error);
    assert_eq!(done.last_file.as_deref(), Some(daily.as_str()));
    assert!(usb.join(&daily).is_file());
    // Not due again on the same day.
    while backup_targets::is_running() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    backup_targets::tick(&cfg, &db, &secrets, tenant, None).await;
    assert!(!backup_targets::is_running());
}
