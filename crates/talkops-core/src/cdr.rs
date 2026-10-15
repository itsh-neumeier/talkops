//! Call detail records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, Postgres, QueryBuilder};
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
    /// Rows to skip (pages of a sorted list).
    pub offset: Option<i64>,
    /// Calls that started at or after this time.
    pub since: Option<DateTime<Utc>>,
    /// Calls that started before this time.
    pub until: Option<DateTime<Utc>>,
    /// Comma-separated directions: `inbound`, `outbound`, `internal`.
    pub direction: Option<String>,
    /// Comma-separated results: `answered`, `missed`, `busy`.
    pub result: Option<String>,
    /// Caller: number, name or extension (substring).
    pub from: Option<String>,
    /// Called party: number, name, extension or used number (substring).
    pub to: Option<String>,
    /// `with` (a recording), `transcript` (a finished transcript) or `without`.
    pub recording: Option<String>,
    /// `time` (default) or `duration`.
    pub sort: Option<String>,
    /// `desc` (default) or `asc`.
    pub order: Option<String>,
}

/// Restrictions that the API adds to a [`CdrQuery`]; not request parameters.
#[derive(Debug, Clone, Default)]
pub struct CdrScope {
    /// Only calls from or to one of these extensions (users without access
    /// to all calls). `None`: no restriction.
    pub extensions: Option<Vec<Uuid>>,
    /// External numbers whose phone book name matches `from`.
    pub from_numbers: Vec<String>,
    /// External numbers whose phone book name matches `to`.
    pub to_numbers: Vec<String>,
}

/// Hangup causes shown as "busy or declined" instead of "missed".
pub const BUSY_CAUSES: &[&str] = &["USER_BUSY", "CALL_REJECTED"];

/// The user input of a substring filter, without `LIKE` wildcards.
fn pattern(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace(['%', '_', '\\'], ""))
        .filter(|s| !s.is_empty())
}

fn list_values(text: Option<&str>) -> Vec<&str> {
    text.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

fn push_filters(
    b: &mut QueryBuilder<'_, Postgres>,
    tenant: TenantId,
    q: &CdrQuery,
    scope: &CdrScope,
) {
    b.push(" WHERE cdr.tenant_id = ").push_bind(tenant);
    if let Some(ext) = q.extension_id {
        b.push(" AND (cdr.extension_id = ")
            .push_bind(ext)
            .push(" OR cdr.dest_extension_id = ")
            .push_bind(ext)
            .push(")");
    }
    if let Some(own) = &scope.extensions {
        b.push(" AND (cdr.extension_id = ANY(")
            .push_bind(own.clone())
            .push(") OR cdr.dest_extension_id = ANY(")
            .push_bind(own.clone())
            .push("))");
    }
    if let Some(s) = pattern(q.search.as_deref()) {
        b.push(" AND (cdr.caller_number ILIKE '%' || ")
            .push_bind(s.clone())
            .push(" || '%' OR cdr.destination ILIKE '%' || ")
            .push_bind(s)
            .push(" || '%')");
    }
    if let Some(t) = q.before {
        b.push(" AND cdr.started_at < ").push_bind(t);
    }
    if let Some(t) = q.since {
        b.push(" AND cdr.started_at >= ").push_bind(t);
    }
    if let Some(t) = q.until {
        b.push(" AND cdr.started_at < ").push_bind(t);
    }
    let directions: Vec<String> = list_values(q.direction.as_deref())
        .into_iter()
        .filter(|d| matches!(*d, "inbound" | "outbound" | "internal"))
        .map(str::to_owned)
        .collect();
    if !directions.is_empty() {
        b.push(" AND cdr.direction::text = ANY(")
            .push_bind(directions)
            .push(")");
    }
    let results = list_values(q.result.as_deref());
    if !results.is_empty() {
        let busy: Vec<String> = BUSY_CAUSES.iter().map(|c| (*c).to_owned()).collect();
        b.push(" AND (false");
        for r in results {
            match r {
                "answered" => {
                    b.push(" OR cdr.answered_at IS NOT NULL");
                }
                "missed" => {
                    b.push(" OR (cdr.answered_at IS NULL AND NOT cdr.hangup_cause = ANY(")
                        .push_bind(busy.clone())
                        .push("))");
                }
                "busy" => {
                    b.push(" OR (cdr.answered_at IS NULL AND cdr.hangup_cause = ANY(")
                        .push_bind(busy.clone())
                        .push("))");
                }
                _ => {}
            }
        }
        b.push(")");
    }
    if let Some(s) = pattern(q.from.as_deref()) {
        b.push(" AND (cdr.caller_number ILIKE '%' || ")
            .push_bind(s.clone())
            .push(" || '%' OR cdr.caller_name ILIKE '%' || ")
            .push_bind(s.clone())
            .push(
                " || '%' OR (cdr.direction <> 'inbound' AND EXISTS (SELECT 1 FROM extensions e \
                 WHERE e.id = cdr.extension_id AND (e.number ILIKE '%' || ",
            )
            .push_bind(s.clone())
            .push(" || '%' OR e.display_name ILIKE '%' || ")
            .push_bind(s)
            .push(" || '%'))) OR cdr.caller_number = ANY(")
            .push_bind(scope.from_numbers.clone())
            .push("))");
    }
    if let Some(s) = pattern(q.to.as_deref()) {
        // Inbound calls: the answering extension and the dialed own number.
        b.push(" AND (cdr.destination ILIKE '%' || ")
            .push_bind(s.clone())
            .push(
                " || '%' OR EXISTS (SELECT 1 FROM extensions e \
                 WHERE e.id = COALESCE(cdr.dest_extension_id, CASE WHEN cdr.direction = 'inbound' \
                 THEN cdr.extension_id END) AND (e.number ILIKE '%' || ",
            )
            .push_bind(s.clone())
            .push(" || '%' OR e.display_name ILIKE '%' || ")
            .push_bind(s.clone())
            .push(
                " || '%')) OR (cdr.direction = 'inbound' AND EXISTS (SELECT 1 FROM numbers n \
                 WHERE n.id = cdr.number_id AND n.e164 ILIKE '%' || ",
            )
            .push_bind(s)
            .push(" || '%')) OR cdr.destination = ANY(")
            .push_bind(scope.to_numbers.clone())
            .push("))");
    }
    match q.recording.as_deref() {
        Some("with") => {
            b.push(" AND EXISTS (SELECT 1 FROM recordings r WHERE r.call_uuid = cdr.call_uuid)");
        }
        Some("transcript") => {
            b.push(
                " AND EXISTS (SELECT 1 FROM recordings r JOIN transcripts t ON t.recording_id = r.id \
                 WHERE r.call_uuid = cdr.call_uuid)",
            );
        }
        Some("without") => {
            b.push(
                " AND NOT EXISTS (SELECT 1 FROM recordings r WHERE r.call_uuid = cdr.call_uuid)",
            );
        }
        _ => {}
    }
}

/// Lists call records, filtered and sorted by the query.
pub async fn list<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    q: &CdrQuery,
) -> CoreResult<Vec<Cdr>> {
    list_scoped(db, tenant, q, &CdrScope::default()).await
}

/// Like [`list`], with the restrictions of the API.
pub async fn list_scoped<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    q: &CdrQuery,
    scope: &CdrScope,
) -> CoreResult<Vec<Cdr>> {
    let mut b = QueryBuilder::new(format!(
        "SELECT {}, (SELECT r.id FROM recordings r WHERE r.call_uuid = cdr.call_uuid) AS recording_id
         FROM cdr",
        COLUMNS
            .split(", ")
            .map(|c| format!("cdr.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    push_filters(&mut b, tenant, q, scope);
    let dir = if q.order.as_deref() == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    match q.sort.as_deref() {
        Some("duration") => b.push(format!(
            " ORDER BY cdr.billsec {dir}, cdr.started_at DESC, cdr.id"
        )),
        _ => b.push(format!(" ORDER BY cdr.started_at {dir}, cdr.id")),
    };
    b.push(" LIMIT ")
        .push_bind(q.limit.unwrap_or(100).clamp(1, 500))
        .push(" OFFSET ")
        .push_bind(q.offset.unwrap_or(0).max(0));
    Ok(b.build_query_as().fetch_all(db).await?)
}

/// Number of call records that match the query (without limit and offset).
pub async fn count<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    q: &CdrQuery,
    scope: &CdrScope,
) -> CoreResult<i64> {
    let mut b = QueryBuilder::new("SELECT count(*) FROM cdr");
    push_filters(&mut b, tenant, q, scope);
    Ok(b.build_query_scalar().fetch_one(db).await?)
}
