//! Settings and history of the automatic audit export and the audit
//! retention (ADR 0037). The export writes new audit entries to a storage
//! target (ADR 0036) on a schedule (`talkops_api::audit_export`).

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::audit::ExportFormat;
use crate::backup_targets::{Retention, Schedule};
use crate::error::{CoreError, CoreResult};
use crate::storage_targets::normalize_path;
use crate::tenant::TenantId;

/// Audit settings of a tenant.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AuditSettings {
    /// Delete entries older than N days; 0 = keep forever.
    pub retention_days: i32,
    pub export_enabled: bool,
    pub storage_target_id: Option<Uuid>,
    /// Sub-folder below the folder of the storage target.
    pub folder: String,
    pub format: ExportFormat,
    pub schedule: Schedule,
    /// Retention of the export files on the storage target.
    pub retention: Retention,
    /// Highest audit entry ID in an export file.
    pub last_exported_id: i64,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct AuditSettingsInput {
    #[serde(default)]
    pub retention_days: i32,
    #[serde(default)]
    pub export_enabled: bool,
    #[serde(default)]
    pub storage_target_id: Option<Uuid>,
    #[serde(default = "default_folder")]
    pub folder: String,
    #[serde(default = "default_format")]
    pub format: ExportFormat,
    #[serde(default = "default_schedule")]
    pub schedule: Schedule,
    #[serde(default = "default_retention")]
    pub retention: Retention,
}

fn default_folder() -> String {
    "audit".into()
}
fn default_format() -> ExportFormat {
    ExportFormat::Json
}
fn default_schedule() -> Schedule {
    Schedule {
        weekdays: 127,
        hour: 2,
        minute: 15,
        every_hours: 0,
    }
}
fn default_retention() -> Retention {
    Retention {
        keep_last: 30,
        keep_daily: 0,
        keep_weekly: 0,
        keep_monthly: 0,
        keep_yearly: 0,
    }
}

impl Default for AuditSettings {
    fn default() -> Self {
        Self {
            retention_days: 0,
            export_enabled: false,
            storage_target_id: None,
            folder: default_folder(),
            format: default_format(),
            schedule: default_schedule(),
            retention: default_retention(),
            last_exported_id: 0,
            last_run_at: None,
            last_success_at: None,
            last_error: None,
            created_at: None,
        }
    }
}

#[derive(FromRow)]
struct Row {
    retention_days: i32,
    export_enabled: bool,
    storage_target_id: Option<Uuid>,
    folder: String,
    format: String,
    weekdays: i16,
    hour: i16,
    minute: i16,
    every_hours: i16,
    keep_last: i16,
    keep_daily: i16,
    keep_weekly: i16,
    keep_monthly: i16,
    keep_yearly: i16,
    last_exported_id: i64,
    last_run_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<Row> for AuditSettings {
    fn from(r: Row) -> Self {
        Self {
            retention_days: r.retention_days,
            export_enabled: r.export_enabled,
            storage_target_id: r.storage_target_id,
            folder: r.folder,
            format: ExportFormat::from_key(&r.format).unwrap_or(ExportFormat::Json),
            schedule: Schedule {
                weekdays: r.weekdays,
                hour: r.hour,
                minute: r.minute,
                every_hours: r.every_hours,
            },
            retention: Retention {
                keep_last: r.keep_last,
                keep_daily: r.keep_daily,
                keep_weekly: r.keep_weekly,
                keep_monthly: r.keep_monthly,
                keep_yearly: r.keep_yearly,
            },
            last_exported_id: r.last_exported_id,
            last_run_at: r.last_run_at,
            last_success_at: r.last_success_at,
            last_error: r.last_error,
            created_at: Some(r.created_at),
        }
    }
}

const COLUMNS: &str = "retention_days, export_enabled, storage_target_id, folder, format,
    weekdays, hour, minute, every_hours, keep_last, keep_daily, keep_weekly, keep_monthly,
    keep_yearly, last_exported_id, last_run_at, last_success_at, last_error, created_at";

/// The settings of a tenant; defaults when none are stored.
pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<AuditSettings> {
    let row: Option<Row> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM audit_settings WHERE tenant_id = $1"
    ))
    .bind(tenant)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(Into::into).unwrap_or_default())
}

/// Checks an input; returns the normalized folder.
pub fn check(input: &AuditSettingsInput) -> CoreResult<String> {
    if !(0..=36500).contains(&input.retention_days) {
        return Err(CoreError::Validation("invalid retention period".into()));
    }
    input.schedule.validate()?;
    input.retention.validate()?;
    if input.export_enabled && input.storage_target_id.is_none() {
        return Err(CoreError::Validation("select a storage target".into()));
    }
    normalize_path(&input.folder, false)
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    input: &AuditSettingsInput,
) -> CoreResult<AuditSettings> {
    let folder = check(input)?;
    sqlx::query(
        "INSERT INTO audit_settings (tenant_id, retention_days, export_enabled,
             storage_target_id, folder, format, weekdays, hour, minute, every_hours,
             keep_last, keep_daily, keep_weekly, keep_monthly, keep_yearly)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
         ON CONFLICT (tenant_id) DO UPDATE SET retention_days = $2, export_enabled = $3,
             storage_target_id = $4, folder = $5, format = $6, weekdays = $7, hour = $8,
             minute = $9, every_hours = $10, keep_last = $11, keep_daily = $12,
             keep_weekly = $13, keep_monthly = $14, keep_yearly = $15, updated_at = now()",
    )
    .bind(tenant)
    .bind(input.retention_days)
    .bind(input.export_enabled)
    .bind(input.storage_target_id)
    .bind(&folder)
    .bind(input.format.key())
    .bind(input.schedule.weekdays)
    .bind(input.schedule.hour)
    .bind(input.schedule.minute)
    .bind(input.schedule.every_hours)
    .bind(input.retention.keep_last)
    .bind(input.retention.keep_daily)
    .bind(input.retention.keep_weekly)
    .bind(input.retention.keep_monthly)
    .bind(input.retention.keep_yearly)
    .execute(pool)
    .await?;
    get(pool, tenant).await
}

/// An export file on a storage target.
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct ExportFile {
    pub id: i64,
    pub storage_target_id: Option<Uuid>,
    pub file: String,
    pub first_id: i64,
    pub last_id: i64,
    pub entries: i32,
    pub bytes: i64,
    /// SHA-256 of the file, lowercase hex.
    pub sha256: String,
    pub created_at: DateTime<Utc>,
}

/// Records a written file and moves `last_exported_id` forward, in one
/// transaction.
pub async fn record_file(pool: &PgPool, tenant: TenantId, file: &ExportFile) -> CoreResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO audit_exports (tenant_id, storage_target_id, file, first_id, last_id,
             entries, bytes, sha256)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(tenant)
    .bind(file.storage_target_id)
    .bind(&file.file)
    .bind(file.first_id)
    .bind(file.last_id)
    .bind(file.entries)
    .bind(file.bytes)
    .bind(&file.sha256)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE audit_settings SET last_exported_id = GREATEST(last_exported_id, $2)
         WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(file.last_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Records the outcome of a run.
pub async fn record_run(pool: &PgPool, tenant: TenantId, error: Option<&str>) -> CoreResult<()> {
    let error = error.map(|e| e.chars().take(1000).collect::<String>());
    sqlx::query(
        "UPDATE audit_settings SET last_run_at = now(),
             last_success_at = CASE WHEN $2::text IS NULL THEN now() ELSE last_success_at END,
             last_error = $2
         WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// The newest export files.
pub async fn files(pool: &PgPool, tenant: TenantId, limit: i64) -> CoreResult<Vec<ExportFile>> {
    Ok(sqlx::query_as(
        "SELECT id, storage_target_id, file, first_id, last_id, entries, bytes, sha256, created_at
         FROM audit_exports WHERE tenant_id = $1 ORDER BY id DESC LIMIT $2",
    )
    .bind(tenant)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?)
}

/// File name of an export: `talkops-audit-YYYYMMDD-HHMMSS-FIRST-LAST.EXT`.
pub fn file_name(
    stamp: NaiveDateTime,
    first_id: i64,
    last_id: i64,
    format: ExportFormat,
) -> String {
    let ext = match format {
        ExportFormat::Csv => "csv",
        ExportFormat::Json => "jsonl",
    };
    format!(
        "talkops-audit-{}-{first_id}-{last_id}.{ext}",
        stamp.format("%Y%m%d-%H%M%S")
    )
}

/// The time stamp of an export file name; `None` for other files.
pub fn file_stamp(name: &str) -> Option<NaiveDateTime> {
    let rest = name.strip_prefix("talkops-audit-")?;
    let (base, ext) = rest.rsplit_once('.')?;
    if ext != "csv" && ext != "jsonl" {
        return None;
    }
    let mut parts = base.splitn(4, '-');
    let (date, time, first, last) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if [first, last]
        .iter()
        .any(|n| n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    NaiveDateTime::parse_from_str(&format!("{date}-{time}"), "%Y%m%d-%H%M%S").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        let stamp =
            NaiveDateTime::parse_from_str("2026-10-16 02:15:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let name = file_name(stamp, 101, 250, ExportFormat::Json);
        assert_eq!(name, "talkops-audit-20261016-021500-101-250.jsonl");
        assert_eq!(file_stamp(&name), Some(stamp));
        assert_eq!(
            file_stamp(&file_name(stamp, 1, 2, ExportFormat::Csv)),
            Some(stamp)
        );
        for other in [
            "talkops-20261016-021500.tar.gz.age",
            "talkops-audit-20261016-021500-1-2.jsonl.sha256",
            "talkops-audit-20261016-021500-1-x.csv",
            "talkops-audit-2026-1016-1-2.csv",
            "notes.txt",
        ] {
            assert_eq!(file_stamp(other), None, "{other}");
        }
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn settings_and_files(pool: PgPool) {
        let tenant = TenantId::DEFAULT;
        let defaults = get(&pool, tenant).await.unwrap();
        assert_eq!(defaults.retention_days, 0);
        assert!(!defaults.export_enabled && defaults.created_at.is_none());

        let mut input = AuditSettingsInput {
            retention_days: 365,
            export_enabled: true,
            storage_target_id: None,
            folder: "/audit/".into(),
            format: ExportFormat::Csv,
            schedule: default_schedule(),
            retention: default_retention(),
        };
        assert!(matches!(
            update(&pool, tenant, &input).await,
            Err(CoreError::Validation(_))
        ));
        let target: Uuid = sqlx::query_scalar(
            "INSERT INTO storage_targets (tenant_id, name, kind, settings, path)
             VALUES ($1, 'USB', 'local', '{\"kind\":\"local\"}', '/var/lib/talkops/targets/usb')
             RETURNING id",
        )
        .bind(tenant)
        .fetch_one(&pool)
        .await
        .unwrap();
        input.storage_target_id = Some(target);
        let saved = update(&pool, tenant, &input).await.unwrap();
        assert_eq!(saved.folder, "audit");
        assert_eq!(saved.format, ExportFormat::Csv);
        assert!(saved.export_enabled);

        // The storage target lists the export as a user and cannot be deleted.
        let users = crate::storage_targets::usage(&pool, tenant, target)
            .await
            .unwrap();
        assert_eq!(
            users[0].purpose,
            crate::storage_targets::StoragePurpose::AuditExport
        );
        assert!(
            crate::storage_targets::delete(&pool, tenant, target)
                .await
                .is_err()
        );

        let file = ExportFile {
            id: 0,
            storage_target_id: Some(target),
            file: "talkops-audit-20261016-021500-1-5.csv".into(),
            first_id: 1,
            last_id: 5,
            entries: 5,
            bytes: 400,
            sha256: "ab".repeat(32),
            created_at: Utc::now(),
        };
        record_file(&pool, tenant, &file).await.unwrap();
        record_run(&pool, tenant, None).await.unwrap();
        let after = get(&pool, tenant).await.unwrap();
        assert_eq!(after.last_exported_id, 5);
        assert!(after.last_success_at.is_some() && after.last_error.is_none());
        record_run(&pool, tenant, Some("connection refused"))
            .await
            .unwrap();
        let failed = get(&pool, tenant).await.unwrap();
        assert_eq!(failed.last_error.as_deref(), Some("connection refused"));
        let listed = files(&pool, tenant, 10).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].sha256, "ab".repeat(32));
        // An invalid checksum is rejected.
        let mut bad = file.clone();
        bad.sha256 = "xyz".into();
        assert!(record_file(&pool, tenant, &bad).await.is_err());
    }
}
