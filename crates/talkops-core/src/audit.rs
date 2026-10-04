//! Audit log of administrative changes.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use crate::error::CoreResult;
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    #[schema(value_type = Object)]
    pub details: serde_json::Value,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Who performed an action.
#[derive(Debug, Clone)]
pub struct Actor {
    pub tenant: TenantId,
    pub user_id: Option<Uuid>,
    pub ip: Option<String>,
}

/// Records an action. `details` must never contain secrets.
pub async fn record<'e>(
    db: impl PgExecutor<'e>,
    actor: &Actor,
    action: &str,
    entity_type: &str,
    entity_id: Option<String>,
    details: serde_json::Value,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO audit_log (tenant_id, user_id, action, entity_type, entity_id, details, ip)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(actor.tenant)
    .bind(actor.user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(details)
    .bind(&actor.ip)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    limit: i64,
) -> CoreResult<Vec<AuditEntry>> {
    Ok(sqlx::query_as(
        "SELECT a.id, a.user_id, u.username, a.action, a.entity_type, a.entity_id, a.details, a.ip, a.created_at
         FROM audit_log a LEFT JOIN users u ON u.id = a.user_id
         WHERE a.tenant_id = $1 ORDER BY a.id DESC LIMIT $2",
    )
    .bind(tenant)
    .bind(limit.clamp(1, 1000))
    .fetch_all(db)
    .await?)
}
