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
        volumes: vec![
            ("voicemail", vol("voicemail")),
            ("recordings", vol("recordings")),
        ],
    };
    std::fs::create_dir_all(vol("voicemail/ext1")).unwrap();
    std::fs::create_dir_all(vol("recordings")).unwrap();
    std::fs::write(vol("voicemail/ext1/msg.wav"), b"RIFF-message").unwrap();
    std::fs::write(vol("recordings/call.wav"), b"RIFF-call").unwrap();

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

    // Run a backup through the API and wait for it.
    let (status, _) = admin.post("/api/v1/backups", json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let mut name = String::new();
    for _ in 0..100 {
        let (_, o) = admin.get("/api/v1/backups").await;
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
    assert!(bytes.starts_with(&[0x1f, 0x8b]), "gzip");
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
    db.close().await;

    let pool = talkops_core::db::connect_lazy(&cfg.database_url, 1).unwrap();
    let manifest = backup::restore(&cfg, &pool, &cfg.dir.join(&name))
        .await
        .expect("restore");
    assert_eq!(manifest.volumes, ["voicemail", "recordings"]);
    let hour: i16 = sqlx::query_scalar("SELECT hour FROM backup_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hour, 5, "database restored");
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1);
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
