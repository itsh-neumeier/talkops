//! Call detail records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use crate::error::CoreResult;
use crate::tenant::TenantId;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "call_direction", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Inbound,
    Outbound,
    Internal,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Cdr {
    #[serde(default)]
    pub id: Uuid,
    pub call_uuid: String,
    pub direction: Direction,
    pub caller_number: String,
    pub caller_name: String,
    pub destination: String,
    pub extension_id: Option<Uuid>,
    #[serde(default)]
    pub dest_extension_id: Option<Uuid>,
    pub trunk_id: Option<Uuid>,
    pub number_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub answered_at: Option<DateTime<Utc>>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: i32,
    pub billsec: i32,
    pub hangup_cause: String,
    /// Recording of the call, if any.
    #[sqlx(default)]
    #[serde(default)]
    pub recording_id: Option<Uuid>,
}

const COLUMNS: &str = "id, call_uuid, direction, caller_number, caller_name, destination, extension_id, \
                       dest_extension_id, trunk_id, number_id, started_at, answered_at, ended_at, duration_secs, billsec, hangup_cause";

/// Inserts a CDR; duplicates (same call UUID, e.g. retried posts) are ignored.
pub async fn insert<'e>(db: impl PgExecutor<'e>, tenant: TenantId, c: &Cdr) -> CoreResult<bool> {
    let res = sqlx::query(
        "INSERT INTO cdr (tenant_id, call_uuid, direction, caller_number, caller_name, destination,
                          extension_id, dest_extension_id, trunk_id, number_id, started_at, answered_at,
                          ended_at, duration_secs, billsec, hangup_cause)
         VALUES ($1, $2, $3, $4, $5, $6,
                 (SELECT id FROM extensions WHERE id = $7 AND tenant_id = $1),
                 (SELECT id FROM extensions WHERE id = $16 AND tenant_id = $1),
                 (SELECT id FROM trunks WHERE id = $8 AND tenant_id = $1),
                 (SELECT id FROM numbers WHERE id = $9 AND tenant_id = $1),
                 $10, $11, $12, $13, $14, $15)
         ON CONFLICT (call_uuid) DO NOTHING",
    )
    .bind(tenant)
    .bind(&c.call_uuid)
    .bind(c.direction)
    .bind(&c.caller_number)
    .bind(&c.caller_name)
    .bind(&c.destination)
    .bind(c.extension_id)
    .bind(c.trunk_id)
    .bind(c.number_id)
    .bind(c.started_at)
    .bind(c.answered_at)
    .bind(c.ended_at)
    .bind(c.duration_secs)
    .bind(c.billsec)
    .bind(&c.hangup_cause)
    .bind(c.dest_extension_id)
    .execute(db)
    .await?;
    Ok(res.rows_affected() == 1)
}

#[derive(Debug, Clone, Default, Deserialize, utoipa::IntoParams)]
pub struct CdrQuery {
    /// Only calls from or to this extension.
    pub extension_id: Option<Uuid>,
    /// Matches caller or destination number (substring).
    pub search: Option<String>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

pub async fn list<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    q: &CdrQuery,
) -> CoreResult<Vec<Cdr>> {
    let sql = format!(
        "SELECT {COLUMNS},
                (SELECT r.id FROM recordings r WHERE r.call_uuid = cdr.call_uuid) AS recording_id
         FROM cdr
         WHERE tenant_id = $1
           AND ($2::uuid IS NULL OR extension_id = $2 OR dest_extension_id = $2)
           AND ($3::text IS NULL OR caller_number ILIKE '%' || $3 || '%' OR destination ILIKE '%' || $3 || '%')
           AND ($4::timestamptz IS NULL OR started_at < $4)
         ORDER BY started_at DESC
         LIMIT $5"
    );
    let search = q
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace(['%', '_'], ""));
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(q.extension_id)
        .bind(search)
        .bind(q.before)
        .bind(q.limit.unwrap_or(100).clamp(1, 500))
        .fetch_all(db)
        .await?)
}
