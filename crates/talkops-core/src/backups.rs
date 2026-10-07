//! Backup schedule and the outcome of the last run. The archives themselves
//! are written by the server (`talkops_api::backup`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct BackupSettings {
    pub enabled: bool,
    /// Local hour (0–23) of the daily backup.
    pub hour: i16,
    /// Archives kept; older ones are deleted.
    pub keep: i16,
    pub include_recordings: bool,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_file: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct BackupSettingsInput {
    pub enabled: bool,
    pub hour: i16,
    pub keep: i16,
    pub include_recordings: bool,
}

const COLUMNS: &str = "enabled, hour, keep, include_recordings, last_run_at, last_file, last_error";

pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<BackupSettings> {
    let sql = format!("SELECT {COLUMNS} FROM backup_settings WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_one(pool).await?)
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    i: &BackupSettingsInput,
) -> CoreResult<BackupSettings> {
    if !(0..=23).contains(&i.hour) {
        return Err(CoreError::Validation("the hour must be 0–23".into()));
    }
    if !(1..=365).contains(&i.keep) {
        return Err(CoreError::Validation(
            "keep between 1 and 365 backups".into(),
        ));
    }
    let sql = format!(
        "UPDATE backup_settings SET enabled = $2, hour = $3, keep = $4, include_recordings = $5,
             updated_at = now()
         WHERE tenant_id = $1 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(i.enabled)
        .bind(i.hour)
        .bind(i.keep)
        .bind(i.include_recordings)
        .fetch_one(pool)
        .await?)
}

/// Records the outcome of a backup run (`Ok(file)` or `Err(message)`).
pub async fn record_run(
    pool: &PgPool,
    tenant: TenantId,
    result: Result<&str, &str>,
) -> CoreResult<()> {
    let (file, error) = match result {
        Ok(f) => (Some(f), None),
        Err(e) => (None, Some(e)),
    };
    sqlx::query(
        "UPDATE backup_settings SET last_run_at = now(),
             last_file = COALESCE($2, last_file), last_error = $3
         WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(file)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}
