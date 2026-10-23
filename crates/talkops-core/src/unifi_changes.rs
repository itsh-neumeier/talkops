//! History of changes to the UniFi voice SSID (ADR 0042) and to the Wi-Fi
//! settings of handsets (ADR 0043).
//!
//! An SSID row stores only the values of the allowlist fields before and
//! after the change. It never contains the passphrase or another secret. A
//! handset row stores the phone setting values before and after.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

pub mod state {
    pub const APPLYING: &str = "applying";
    /// Handset: stored in TalkOps, not fetched by the handset yet.
    pub const WAITING: &str = "waiting";
    pub const APPLIED: &str = "applied";
    pub const FAILED: &str = "failed";
    pub const ROLLED_BACK: &str = "rolled_back";
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct WifiChange {
    pub id: Uuid,
    /// `ssid` (UniFi voice SSID) or `handset` (phone setting).
    pub target: String,
    pub site_id: Option<String>,
    pub wifi_id: Option<String>,
    pub wifi_name: String,
    pub phone_id: Option<Uuid>,
    pub phone_name: String,
    /// `apply` or `rollback`.
    pub kind: String,
    pub rollback_of: Option<Uuid>,
    pub survey_id: Option<Uuid>,
    /// `applying`, `waiting`, `applied`, `failed` or `rolled_back`.
    pub state: String,
    pub fields: Vec<String>,
    #[schema(value_type = Option<Object>)]
    pub before: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub after: Option<serde_json::Value>,
    pub network_version: Option<String>,
    /// Error class, e.g. `forbidden` or `verify_mismatch`.
    pub error: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl WifiChange {
    /// A change with an "after" snapshot that is not reverted yet.
    pub fn can_roll_back(&self) -> bool {
        self.kind == "apply"
            && self.after.is_some()
            && self.before.is_some()
            && matches!(
                self.state.as_str(),
                state::APPLIED | state::FAILED | state::WAITING
            )
    }
}

pub struct NewChange<'a> {
    /// `ssid` or `handset`.
    pub target: &'a str,
    pub site_id: Option<&'a str>,
    pub wifi_id: Option<&'a str>,
    pub wifi_name: &'a str,
    pub phone_id: Option<Uuid>,
    pub phone_name: &'a str,
    pub kind: &'a str,
    pub rollback_of: Option<Uuid>,
    pub survey_id: Option<Uuid>,
    pub fields: Vec<String>,
    pub network_version: Option<&'a str>,
    pub created_by: Option<Uuid>,
}

const SELECT: &str = "SELECT c.id, c.target, c.site_id, c.wifi_id, c.wifi_name, c.phone_id,
        c.phone_name, c.kind, c.rollback_of,
        c.survey_id, c.state, c.fields, c.before, c.after, c.network_version, c.error,
        c.created_by, u.display_name AS created_by_name, c.created_at, c.finished_at
    FROM unifi_wifi_changes c LEFT JOIN users u ON u.id = c.created_by";

/// Starts a change in the state `applying`. Only one change of a tenant can
/// be in this state; a row older than five minutes counts as interrupted.
pub async fn begin(pool: &PgPool, tenant: TenantId, new: &NewChange<'_>) -> CoreResult<Uuid> {
    sqlx::query(
        "UPDATE unifi_wifi_changes SET state = 'failed', error = 'interrupted', finished_at = now()
         WHERE tenant_id = $1 AND state = 'applying' AND created_at < now() - interval '5 minutes'",
    )
    .bind(tenant)
    .execute(pool)
    .await?;
    let id = sqlx::query_scalar(
        "INSERT INTO unifi_wifi_changes (tenant_id, site_id, wifi_id, wifi_name, kind,
             rollback_of, survey_id, state, fields, network_version, created_by,
             target, phone_id, phone_name)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'applying', $8, $9, $10, $11, $12, $13)
         RETURNING id",
    )
    .bind(tenant)
    .bind(new.site_id)
    .bind(new.wifi_id)
    .bind(new.wifi_name)
    .bind(new.kind)
    .bind(new.rollback_of)
    .bind(new.survey_id)
    .bind(&new.fields)
    .bind(new.network_version)
    .bind(new.created_by)
    .bind(new.target)
    .bind(new.phone_id)
    .bind(new.phone_name)
    .fetch_one(pool)
    .await
    .map_err(|e| match CoreError::from(e) {
        CoreError::Conflict(_) => CoreError::Conflict("change_in_progress".into()),
        other => other,
    })?;
    Ok(id)
}

/// True while a change of the tenant is in progress.
pub async fn applying(pool: &PgPool, tenant: TenantId) -> CoreResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM unifi_wifi_changes WHERE tenant_id = $1
             AND state = 'applying' AND created_at >= now() - interval '5 minutes')",
    )
    .bind(tenant)
    .fetch_one(pool)
    .await?)
}

/// Result of a change.
pub struct Outcome<'a> {
    /// `applied`, `waiting` or `failed`.
    pub state: &'a str,
    pub wifi_name: Option<&'a str>,
    pub before: Option<&'a serde_json::Value>,
    pub after: Option<&'a serde_json::Value>,
    pub error: Option<&'a str>,
}

/// Ends a change with its snapshots.
pub async fn finish(pool: &PgPool, tenant: TenantId, id: Uuid, o: &Outcome<'_>) -> CoreResult<()> {
    sqlx::query(
        "UPDATE unifi_wifi_changes SET state = $3, wifi_name = COALESCE($4, wifi_name),
             before = $5, after = $6, error = $7, finished_at = now()
         WHERE tenant_id = $1 AND id = $2 AND state = 'applying'",
    )
    .bind(tenant)
    .bind(id)
    .bind(o.state)
    .bind(o.wifi_name)
    .bind(o.before)
    .bind(o.after)
    .bind(o.error.map(|e| e.chars().take(80).collect::<String>()))
    .execute(pool)
    .await?;
    Ok(())
}

/// The handset fetched its configuration: its waiting changes are applied.
pub async fn handset_fetched(pool: &PgPool, phone: Uuid) -> CoreResult<u64> {
    Ok(sqlx::query(
        "UPDATE unifi_wifi_changes SET state = 'applied', finished_at = now()
         WHERE phone_id = $1 AND state = 'waiting'",
    )
    .bind(phone)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Marks a change as reverted by a successful rollback.
pub async fn mark_rolled_back(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    sqlx::query(
        "UPDATE unifi_wifi_changes SET state = 'rolled_back'
         WHERE tenant_id = $1 AND id = $2 AND kind = 'apply' AND state IN ('applied', 'failed', 'waiting')",
    )
    .bind(tenant)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<WifiChange> {
    let sql = format!("{SELECT} WHERE c.tenant_id = $1 AND c.id = $2");
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(CoreError::NotFound)
}

/// Newest changes first.
pub async fn list(pool: &PgPool, tenant: TenantId, limit: i64) -> CoreResult<Vec<WifiChange>> {
    let sql = format!("{SELECT} WHERE c.tenant_id = $1 ORDER BY c.created_at DESC, c.id LIMIT $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?)
}
