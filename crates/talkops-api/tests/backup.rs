//! Backup and restore round trip (needs `pg_dump`/`pg_restore` matching the
//! server version; set TALKOPS_TEST_PG_BIN_DIR to pick them).

mod common;

use std::path::PathBuf;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::json;
use sqlx::PgPool;
use talkops_api::backup::{self, BackupConfig};

#[test]
fn backup_cli_uses_the_configured_call_record_spool() {
    use clap::Parser;

    #[derive(Parser)]
    struct BackupArgs {
        #[command(flatten)]
        storage: talkops_api::config::Storage,
    }

    let args = BackupArgs::try_parse_from([
        "backup",
        "--database-url",
        "postgres://localhost/talkops",
        "--cdr-spool-dir",
        "/custom/call-record-spool",
    ])
    .unwrap();
    let config = BackupConfig::from_storage(&args.storage).unwrap();
    assert!(config.volumes.iter().any(|(name, path)| {
        *name == "cdr-spool" && path == &PathBuf::from("/custom/call-record-spool")
    }));
}

fn test_db_url(db: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let mut url = url::Url::parse(&base).unwrap();
    url.set_path(db.connect_options().get_database().expect("test database"));
    url.to_string()
}

fn pg_bin_dir() -> Option<PathBuf> {
    std::env::var_os("TALKOPS_TEST_PG_BIN_DIR").map(PathBuf::from)
}

fn tools_available() -> bool {
    let tool = pg_bin_dir().map_or("pg_dump".into(), |d| d.join("pg_dump"));
    std::process::Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn backup_and_restore(db: PgPool) {
    if !tools_available() {
        assert!(std::env::var_os("CI").is_none(), "CI must provide pg_dump");
        eprintln!("pg_dump not found, skipping");
        return;
    }
    let root = std::env::temp_dir().join(format!("talkops-backup-test-{}", uuid::Uuid::new_v4()));
    let vol = |n: &str| root.join(n);
    let cfg = BackupConfig {
        database_url: test_db_url(&db),
        dir: vol("backups"),
        pg_bin_dir: pg_bin_dir(),
        password: Some("separate-long-backup-password".to_owned().into()),
        deployment_dir: Some(vol("deployment")),
        deployment_files: vec![".env".into(), "docker-compose.yml".into()],
        deployment_key: Some(KEY.to_owned().into()),
        project_name: "talkops".into(),
        profiles: String::new(),
        restore_max_bytes: 1 << 30,
        volumes: vec![
            ("voicemail", vol("voicemail")),
            ("recordings", vol("recordings")),
            ("cdr-spool", vol("cdr-spool")),
        ],
    };
    std::fs::create_dir_all(vol("deployment")).unwrap();
    std::fs::write(vol("deployment/.env"),format!("TALKOPS_SECRET_KEY={KEY}\nPOSTGRES_PASSWORD=test\nTALKOPS_ESL_PASSWORD=test\nTALKOPS_XMLCURL_PASSWORD=test\n")).unwrap();
    std::fs::write(vol("deployment/docker-compose.yml"), "services: {}\n").unwrap();
    std::fs::create_dir_all(vol("voicemail/ext1")).unwrap();
    std::fs::create_dir_all(vol("recordings")).unwrap();
    std::fs::write(vol("voicemail/ext1/msg.wav"), b"RIFF-message").unwrap();
    std::fs::write(vol("recordings/call.wav"), b"RIFF-call").unwrap();
    std::fs::create_dir_all(vol("cdr-spool/rejected")).unwrap();
    std::fs::write(vol("cdr-spool/pending.xml"), b"<cdr>pending</cdr>").unwrap();
    std::fs::write(vol("cdr-spool/rejected/invalid.xml"), b"invalid record").unwrap();

    let router = talkops_api::app(state(db.clone()).with_backup(cfg.clone()), None);
    let admin = setup_admin(&router).await;
    let settings = json!({"enabled": true, "hour": 5, "keep": 3, "include_recordings": true});
    let (status, _) = admin.put("/api/v1/backups/settings", settings).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin
        .put(
            "/api/v1/backups/settings",
            json!({"enabled": true, "hour": 24, "keep": 3, "include_recordings": true}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let encrypted = talkops_core::crypto::SecretBox::from_hex(KEY)
        .unwrap()
        .encrypt("saved API password")
        .unwrap();
    sqlx::query(
        "INSERT INTO transcription_providers (tenant_id, name, kind, url, key_enc)
         VALUES ($1, 'OpenAI', 'openai', 'https://api.openai.com/v1', $2)",
    )
    .bind(talkops_core::tenant::TenantId::DEFAULT)
    .bind(&encrypted)
    .execute(&db)
    .await
    .unwrap();
    // Run a backup through the API and wait for it.
    let (status, _) = admin.post("/api/v1/backups", json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let mut name = String::new();
    for _ in 0..600 {
        let (_, o) = admin.get("/api/v1/backups").await;
        assert!(o["settings"]["last_error"].is_null(), "backup failed: {o}");
        if !o["running"].as_bool().unwrap() && o["files"].as_array().unwrap().len() == 1 {
            assert!(o["settings"]["last_error"].is_null(), "{o}");
            name = o["files"][0]["name"].as_str().unwrap().to_owned();
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(backup::valid_name(&name), "backup finished: {name:?}");

    // Download (and no path tricks).
    let res = raw(
        &router,
        Request::get(format!("/api/v1/backups/{name}"))
            .header(header::COOKIE, &admin.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = body_bytes(res).await;
    assert!(
        bytes.starts_with(b"age-encryption.org/v1"),
        "age encryption"
    );
    let prepared = talkops_api::recovery::prepare(
        &cfg.dir.join(&name),
        cfg.password.as_ref(),
        cfg.restore_max_bytes,
    )
    .unwrap();
    assert!(prepared.manifest.deployment.as_ref().unwrap().complete);
    let export = root.join("recovered");
    prepared.export(&export).unwrap();
    let exported_key = talkops_api::recovery::env_value(
        &std::fs::read_to_string(export.join(".env")).unwrap(),
        "TALKOPS_SECRET_KEY",
    )
    .unwrap();

    assert_eq!(
        talkops_core::crypto::SecretBox::from_hex(&exported_key)
            .unwrap()
            .decrypt(&encrypted)
            .unwrap(),
        "saved API password"
    );
    let (status, _) = admin.get("/api/v1/backups/..%2Fsecret.tar.gz").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Change everything, then restore.
    sqlx::query("UPDATE backup_settings SET hour = 7")
        .execute(&db)
        .await
        .unwrap();
    std::fs::remove_file(vol("voicemail/ext1/msg.wav")).unwrap();
    std::fs::write(vol("voicemail/stray.wav"), b"x").unwrap();
    std::fs::write(vol("recordings/call.wav"), b"changed").unwrap();
    std::fs::remove_file(vol("cdr-spool/pending.xml")).unwrap();
    std::fs::write(vol("cdr-spool/stray.xml"), b"post-backup record").unwrap();
    db.close().await;

    let pool = talkops_core::db::connect_lazy(&cfg.database_url, 1).unwrap();
    let mut wrong_key = cfg.clone();
    wrong_key.deployment_key = Some("ff".repeat(32).into());
    assert!(
        backup::restore(&wrong_key, &pool, &cfg.dir.join(&name))
            .await
            .is_err()
    );
    let unchanged: i16 = sqlx::query_scalar("SELECT hour FROM backup_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        unchanged, 7,
        "wrong-key preflight leaves the database unchanged"
    );
    assert!(
        vol("voicemail/stray.wav").exists(),
        "wrong-key preflight leaves media unchanged"
    );
    let manifest = backup::restore(&cfg, &pool, &cfg.dir.join(&name))
        .await
        .expect("restore");
    assert_eq!(manifest.volumes, ["voicemail", "recordings", "cdr-spool"]);
    let hour: i16 = sqlx::query_scalar("SELECT hour FROM backup_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hour, 5, "database restored");
    let saved: String = sqlx::query_scalar("SELECT key_enc FROM transcription_providers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        talkops_core::crypto::SecretBox::from_hex(&exported_key)
            .unwrap()
            .decrypt(&saved)
            .unwrap(),
        "saved API password"
    );
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0, "restored sessions invalidated");
    assert_eq!(
        std::fs::read(vol("voicemail/ext1/msg.wav")).unwrap(),
        b"RIFF-message"
    );
    assert!(!vol("voicemail/stray.wav").exists(), "volume replaced");
    assert_eq!(
        std::fs::read(vol("recordings/call.wav")).unwrap(),
        b"RIFF-call"
    );
    pool.close().await;

    assert_eq!(
        std::fs::read(vol("cdr-spool/pending.xml")).unwrap(),
        b"<cdr>pending</cdr>"
    );
    assert_eq!(
        std::fs::read(vol("cdr-spool/rejected/invalid.xml")).unwrap(),
        b"invalid record"
    );
    assert!(
        !vol("cdr-spool/stray.xml").exists(),
        "spool volume replaced"
    );

    // Retention keeps the newest archives only.
    std::fs::write(vol("backups/talkops-20200101-000000.tar.gz"), b"old").unwrap();
    assert_eq!(backup::prune(&cfg.dir, 1).await.unwrap(), 1);
    assert!(cfg.dir.join(&name).exists());
    let _ = std::fs::remove_dir_all(&root);
}

async fn body_bytes(res: axum::response::Response) -> Vec<u8> {
    use http_body_util::BodyExt;
    res.into_body().collect().await.unwrap().to_bytes().to_vec()
}
