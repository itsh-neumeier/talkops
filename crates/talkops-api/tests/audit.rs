//! Audit log (ADR 0037): filters, paging, downloads, permissions, the
//! automatic export to a local storage target and the retention. The export
//! test needs rclone (TALKOPS_RCLONE or `PATH`).

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::{Value, json};
use sha2::Digest;
use sqlx::PgPool;
use talkops_api::backup::BackupConfig;
use talkops_api::rclone::Rclone;
use talkops_core::tenant::TenantId;

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

fn cfg(root: &Path) -> BackupConfig {
    BackupConfig {
        database_url: String::new(),
        dir: root.join("backups"),
        pg_bin_dir: None,
        volumes: vec![],
        password: None,
        deployment_dir: None,
        deployment_files: vec![],
        deployment_key: None,
        project_name: "talkops".into(),
        profiles: String::new(),
        restore_max_bytes: 1 << 30,
        rclone: Rclone::new(rclone_bin()),
        target_roots: vec![root.join("targets")],
    }
}

/// Adds audit entries with fixed times: `(action, entity_type, entity_id, days ago)`.
async fn seed(db: &PgPool, rows: &[(&str, &str, &str, i32)]) {
    for (action, entity, id, days) in rows {
        sqlx::query(
            "INSERT INTO audit_log (tenant_id, action, entity_type, entity_id, details, ip, created_at)
             VALUES ($1, $2, $3, $4, $5, '192.0.2.7', now() - make_interval(days => $6))",
        )
        .bind(TenantId::DEFAULT)
        .bind(action)
        .bind(entity)
        .bind(id)
        .bind(json!({"name": format!("=cmd {id}")}))
        .bind(days)
        .execute(db)
        .await
        .unwrap();
    }
}

async fn download(client: &Client, query: &str) -> (StatusCode, String, String) {
    let res = raw(
        &client.router,
        Request::get(format!("/api/v1/audit/export?{query}"))
            .header(header::COOKIE, &client.cookie)
            .header("x-requested-with", "TalkOps")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = res.status();
    let disposition = res
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    (status, disposition, body_string(res).await)
}

async fn total(client: &Client, query: &str) -> (i64, Vec<Value>) {
    let res = raw(
        &client.router,
        Request::get(format!("/api/v1/audit?{query}"))
            .header(header::COOKIE, &client.cookie)
            .header("x-requested-with", "TalkOps")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let count = res.headers()["x-total-count"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let rows = body_json(res).await.as_array().unwrap().clone();
    (count, rows)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn filters_pages_and_downloads(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    sqlx::query("DELETE FROM audit_log")
        .execute(&db)
        .await
        .unwrap();
    seed(
        &db,
        &[
            ("create", "extension", "ext-100", 10),
            ("update", "extension", "ext-100", 5),
            ("delete", "extension", "ext-101", 3),
            ("update", "trunk", "trunk-a", 2),
            ("update", "trunk", "trunk-b", 1),
        ],
    )
    .await;

    let (count, rows) = total(&admin, "limit=2").await;
    assert_eq!(count, 5);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["entity_id"], "trunk-b", "newest first");
    let (_, page2) = total(&admin, "limit=2&offset=2").await;
    assert_eq!(page2[0]["entity_id"], "ext-101");
    let (count, _) = total(&admin, "action=update").await;
    assert_eq!(count, 3);
    let (count, _) = total(&admin, "entity_type=trunk&action=update").await;
    assert_eq!(count, 2);
    let (count, rows) = total(&admin, "search=EXT-10&sort=time&order=asc").await;
    assert_eq!(count, 3);
    assert_eq!(rows[0]["action"], "create");
    let since = (chrono::Utc::now() - chrono::Duration::days(4)).to_rfc3339();
    let (count, _) = total(&admin, &format!("since={}", urlencode(&since))).await;
    assert_eq!(count, 3);
    // `%` and `_` are no wildcards.
    let (count, _) = total(&admin, "search=%25").await;
    assert_eq!(count, 5);

    let (status, facets) = admin.get("/api/v1/audit/facets").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(facets["actions"], json!(["create", "delete", "update"]));
    assert_eq!(facets["entity_types"], json!(["extension", "trunk"]));

    // CSV: header, filter, formula guard. The download itself is recorded.
    let (status, disposition, csv) = download(&admin, "format=csv&entity_type=trunk").await;
    assert_eq!(status, StatusCode::OK);
    assert!(disposition.contains(".csv"), "{disposition}");
    let lines: Vec<&str> = csv.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3, "{csv}");
    assert!(lines[0].starts_with("id,created_at,"));
    assert!(
        lines[1].contains("\"{\"\"name\"\":\"\"=cmd trunk-b\"\"}\""),
        "{}",
        lines[1]
    );
    let (count, rows) = total(&admin, "action=export").await;
    assert_eq!(count, 1);
    assert_eq!(rows[0]["details"]["entries"], 2);
    assert_eq!(rows[0]["details"]["format"], "csv");

    let (status, disposition, body) = download(&admin, "format=json&action=delete").await;
    assert_eq!(status, StatusCode::OK);
    assert!(disposition.contains(".json"));
    let json: Vec<Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(json.len(), 1);
    assert_eq!(json[0]["entity_id"], "ext-101");
    let (status, _, _) = download(&admin, "format=xml").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // audit.view reads and downloads; only audit.manage changes the settings.
    let (_, role) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "Auditors", "permissions": ["audit.view"]}),
        )
        .await;
    let (status, user) = admin
        .post(
            "/api/v1/users",
            json!({"username": "rita", "display_name": "Rita", "role": "user",
                   "role_id": role["id"], "password": "correct-horse"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{user}");
    let rita = login(&router, "rita", "correct-horse").await.unwrap();
    // 5 seeded, 2 downloads, the new role and the new user.
    assert_eq!(total(&rita, "").await.0, 9);
    assert_eq!(download(&rita, "format=csv").await.0, StatusCode::OK);
    assert_eq!(
        rita.get("/api/v1/audit/settings").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        rita.put("/api/v1/audit/settings", json!({"retention_days": 1}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let nobody = {
        admin
            .post(
                "/api/v1/users",
                json!({"username": "otto", "display_name": "Otto", "role": "user",
                       "password": "correct-horse"}),
            )
            .await;
        login(&router, "otto", "correct-horse").await.unwrap()
    };
    assert_eq!(nobody.get("/api/v1/audit").await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        download(&nobody, "format=csv").await.0,
        StatusCode::FORBIDDEN
    );
}

fn urlencode(s: &str) -> String {
    s.replace('+', "%2B").replace(':', "%3A")
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn retention_keeps_entries_that_are_not_exported(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    sqlx::query("DELETE FROM audit_log")
        .execute(&db)
        .await
        .unwrap();
    seed(
        &db,
        &[
            ("create", "extension", "old", 400),
            ("update", "extension", "new", 1),
        ],
    )
    .await;
    let (status, view) = admin
        .put("/api/v1/audit/settings", json!({"retention_days": 365}))
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["retention_days"], 365);
    assert_eq!(view["export_enabled"], false);
    let deleted = talkops_api::audit_export::apply_retention(&db, TenantId::DEFAULT)
        .await
        .unwrap();
    assert_eq!(deleted, 1);
    let (_, rows) = total(&admin, "entity_type=extension").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["entity_id"], "new");

    // Validation: an enabled export needs a storage target; no backup config.
    let (status, _) = admin
        .put(
            "/api/v1/audit/settings",
            json!({"retention_days": 30, "export_enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .put("/api/v1/audit/settings", json!({"retention_days": -1}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        admin.post("/api/v1/audit/export/run", json!({})).await.0,
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn export_writes_new_entries_with_checksums(db: PgPool) {
    if !rclone_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let usb = root.path().join("targets/usb");
    std::fs::create_dir_all(&usb).unwrap();
    let cfg = cfg(root.path());
    let state = state(db.clone()).with_backup(cfg.clone());
    let secrets = state.secrets.clone();
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;
    sqlx::query("DELETE FROM audit_log")
        .execute(&db)
        .await
        .unwrap();
    seed(
        &db,
        &[
            ("create", "extension", "a", 3),
            ("update", "extension", "b", 2),
        ],
    )
    .await;

    let (status, storage) = admin
        .post(
            "/api/v1/storage-targets",
            json!({"name": "USB", "config": {"kind": "local"}, "path": usb.display().to_string()}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{storage}");
    let storage_id = storage["id"].as_str().unwrap().to_owned();
    let (status, view) = admin
        .put(
            "/api/v1/audit/settings",
            json!({
                "retention_days": 1,
                "export_enabled": true,
                "storage_target_id": storage_id,
                "folder": "audit",
                "format": "json",
                "schedule": {"weekdays": 0, "hour": 2, "minute": 15, "every_hours": 0},
                "retention": {"keep_last": 1, "keep_daily": 0, "keep_weekly": 0, "keep_monthly": 0, "keep_yearly": 0}
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["transfers_available"], true);

    // The storage target shows the export as a user and cannot be deleted.
    let (_, list) = admin.get("/api/v1/storage-targets").await;
    assert_eq!(list["targets"][0]["used_by"][0]["purpose"], "audit_export");
    assert_eq!(
        admin
            .call(
                "DELETE",
                &format!("/api/v1/storage-targets/{storage_id}"),
                None
            )
            .await
            .0,
        StatusCode::CONFLICT
    );

    // Retention waits for the export while the export is on.
    let tenant = TenantId::DEFAULT;
    assert_eq!(
        talkops_api::audit_export::apply_retention(&db, tenant)
            .await
            .unwrap(),
        0
    );

    // The export takes only entries older than a few seconds.
    let settle = "UPDATE audit_log SET created_at = created_at - interval '1 minute'";
    sqlx::query(settle).execute(&db).await.unwrap();
    let file = talkops_api::audit_export::run(&cfg, &db, &secrets, tenant)
        .await
        .unwrap()
        .expect("a file");
    let dir = usb.join("audit");
    let content = std::fs::read(dir.join(&file.file)).unwrap();
    assert_eq!(file.sha256, hex::encode(sha2::Sha256::digest(&content)));
    let sum = std::fs::read_to_string(dir.join(format!("{}.sha256", file.file))).unwrap();
    assert_eq!(sum, format!("{}  {}\n", file.sha256, file.file));
    let first_count = String::from_utf8(content).unwrap().lines().count();
    assert!(first_count >= 2, "{first_count}");

    // Exported entries older than the retention period are deleted now.
    assert!(
        talkops_api::audit_export::apply_retention(&db, tenant)
            .await
            .unwrap()
            >= 2
    );

    // The next run writes only new entries; file retention keeps one file.
    assert!(
        talkops_api::audit_export::run(&cfg, &db, &secrets, tenant)
            .await
            .unwrap()
            .is_none()
    );
    seed(&db, &[("delete", "extension", "c", 0)]).await;
    assert!(
        talkops_api::audit_export::run(&cfg, &db, &secrets, tenant)
            .await
            .unwrap()
            .is_none(),
        "a new entry waits until it is settled"
    );
    sqlx::query(settle).execute(&db).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let second = talkops_api::audit_export::run(&cfg, &db, &secrets, tenant)
        .await
        .unwrap()
        .expect("a second file");
    assert!(second.first_id > file.last_id);
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&second.file), "{names:?}");
    assert!(!names.contains(&file.file), "{names:?}");
    assert_eq!(names.len(), 2, "{names:?}");

    let (_, view) = admin.get("/api/v1/audit/settings").await;
    assert_eq!(view["last_exported_id"], second.last_id);
    assert!(view["files"].as_array().unwrap().len() >= 2);
}
