//! Audit log of administrative changes: recording, filtered and paged
//! queries, CSV/JSON export and retention (ADR 0037).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
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

/// The newest entries, without filters.
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

/// Filters, sort order and page of an audit query.
#[derive(Debug, Clone, Default, Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
pub struct AuditQuery {
    /// Entries at or after this time.
    pub since: Option<DateTime<Utc>>,
    /// Entries before this time.
    pub until: Option<DateTime<Utc>>,
    /// Only entries of this user.
    pub user_id: Option<Uuid>,
    /// Exact action, e.g. `update`.
    pub action: Option<String>,
    /// Exact object type, e.g. `extension`.
    pub entity_type: Option<String>,
    /// Text in the object ID or the details (substring, case-insensitive).
    pub search: Option<String>,
    /// `time` (default), `user`, `action` or `entity`.
    pub sort: Option<String>,
    /// `desc` (default) or `asc`.
    pub order: Option<String>,
    /// Page size, 1 to 500 (default 50).
    pub limit: Option<i64>,
    /// Rows to skip.
    pub offset: Option<i64>,
}

const SELECT: &str = "SELECT a.id, a.user_id, u.username, a.action, a.entity_type, a.entity_id,
    a.details, a.ip, a.created_at FROM audit_log a LEFT JOIN users u ON u.id = a.user_id";

fn text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn push_filters(b: &mut QueryBuilder<'_, Postgres>, tenant: TenantId, q: &AuditQuery) {
    b.push(" WHERE a.tenant_id = ").push_bind(tenant);
    if let Some(t) = q.since {
        b.push(" AND a.created_at >= ").push_bind(t);
    }
    if let Some(t) = q.until {
        b.push(" AND a.created_at < ").push_bind(t);
    }
    if let Some(u) = q.user_id {
        b.push(" AND a.user_id = ").push_bind(u);
    }
    if let Some(a) = text(q.action.as_deref()) {
        b.push(" AND a.action = ").push_bind(a);
    }
    if let Some(e) = text(q.entity_type.as_deref()) {
        b.push(" AND a.entity_type = ").push_bind(e);
    }
    if let Some(s) = text(q.search.as_deref()).map(|s| s.replace(['%', '_', '\\'], ""))
        && !s.is_empty()
    {
        b.push(" AND (a.entity_id ILIKE '%' || ")
            .push_bind(s.clone())
            .push(" || '%' OR a.details::text ILIKE '%' || ")
            .push_bind(s)
            .push(" || '%')");
    }
}

fn push_order(b: &mut QueryBuilder<'_, Postgres>, q: &AuditQuery) {
    let dir = if q.order.as_deref() == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let column = match q.sort.as_deref() {
        Some("user") => "u.username",
        Some("action") => "a.action",
        Some("entity") => "a.entity_type",
        _ => "a.id",
    };
    // The ID breaks ties and keeps pages stable.
    b.push(format!(" ORDER BY {column} {dir} NULLS LAST, a.id {dir}"));
}

/// One page of entries and the number of all matching entries.
pub async fn search(
    pool: &PgPool,
    tenant: TenantId,
    q: &AuditQuery,
) -> CoreResult<(Vec<AuditEntry>, i64)> {
    let mut b = QueryBuilder::new(SELECT);
    push_filters(&mut b, tenant, q);
    push_order(&mut b, q);
    b.push(" LIMIT ")
        .push_bind(q.limit.unwrap_or(50).clamp(1, 500))
        .push(" OFFSET ")
        .push_bind(q.offset.unwrap_or(0).max(0));
    let rows = b.build_query_as().fetch_all(pool).await?;
    let mut c = QueryBuilder::new("SELECT count(*) FROM audit_log a");
    push_filters(&mut c, tenant, q);
    let total: i64 = c.build_query_scalar().fetch_one(pool).await?;
    Ok((rows, total))
}

/// Age in seconds before the export takes an entry.
pub const SETTLE_SECS: i64 = 10;

/// The most rows of one download.
pub const EXPORT_MAX: i64 = 100_000;

/// All matching entries in the sort order of the query (no paging), at most
/// [`EXPORT_MAX`]. Fails when more entries match.
pub async fn export(
    pool: &PgPool,
    tenant: TenantId,
    q: &AuditQuery,
) -> CoreResult<Vec<AuditEntry>> {
    let mut b = QueryBuilder::new(SELECT);
    push_filters(&mut b, tenant, q);
    push_order(&mut b, q);
    b.push(" LIMIT ").push_bind(EXPORT_MAX + 1);
    let rows: Vec<AuditEntry> = b.build_query_as().fetch_all(pool).await?;
    if rows.len() as i64 > EXPORT_MAX {
        return Err(CoreError::Validation(format!(
            "more than {EXPORT_MAX} entries match; narrow the filter"
        )));
    }
    Ok(rows)
}

/// Entries with an ID above `after_id`, oldest first (incremental export).
/// Entries younger than [`SETTLE_SECS`] wait for the next run: an entry of a
/// transaction that is still open can get a lower ID than a committed one.
pub async fn after(
    pool: &PgPool,
    tenant: TenantId,
    after_id: i64,
    limit: i64,
) -> CoreResult<Vec<AuditEntry>> {
    Ok(sqlx::query_as(&format!(
        "{SELECT} WHERE a.tenant_id = $1 AND a.id > $2
           AND a.created_at < now() - make_interval(secs => {SETTLE_SECS})
         ORDER BY a.id LIMIT $3"
    ))
    .bind(tenant)
    .bind(after_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?)
}

/// Values for the filter lists of the UI.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AuditFacets {
    pub actions: Vec<String>,
    pub entity_types: Vec<String>,
    /// Users with entries: `(id, username)`.
    pub users: Vec<AuditUser>,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct AuditUser {
    pub id: Uuid,
    pub username: String,
}

pub async fn facets(pool: &PgPool, tenant: TenantId) -> CoreResult<AuditFacets> {
    let actions = sqlx::query_scalar(
        "SELECT DISTINCT action FROM audit_log WHERE tenant_id = $1 ORDER BY action",
    )
    .bind(tenant)
    .fetch_all(pool)
    .await?;
    let entity_types = sqlx::query_scalar(
        "SELECT DISTINCT entity_type FROM audit_log WHERE tenant_id = $1 ORDER BY entity_type",
    )
    .bind(tenant)
    .fetch_all(pool)
    .await?;
    let users = sqlx::query_as(
        "SELECT u.id, u.username FROM users u
         WHERE u.tenant_id = $1
           AND EXISTS (SELECT 1 FROM audit_log a WHERE a.tenant_id = $1 AND a.user_id = u.id)
         ORDER BY u.username",
    )
    .bind(tenant)
    .fetch_all(pool)
    .await?;
    Ok(AuditFacets {
        actions,
        entity_types,
        users,
    })
}

/// File format of an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// RFC 4180 CSV with a header line.
    Csv,
    /// A JSON array (download) or one JSON object per line (automatic export).
    Json,
}

impl ExportFormat {
    pub fn key(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "csv" => Some(Self::Csv),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

const CSV_HEADER: &str = "id,created_at,user_id,username,ip,action,entity_type,entity_id,details";

/// One CSV cell. A cell that a spreadsheet would read as a formula gets a
/// leading `'`.
fn csv_cell(value: &str) -> String {
    let guarded;
    let value = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        guarded = format!("'{value}");
        guarded.as_str()
    } else {
        value
    };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn csv_line(e: &AuditEntry) -> String {
    let user_id = e.user_id.map(|u| u.to_string()).unwrap_or_default();
    let cells = [
        e.id.to_string(),
        e.created_at.to_rfc3339(),
        user_id,
        e.username.clone().unwrap_or_default(),
        e.ip.clone().unwrap_or_default(),
        e.action.clone(),
        e.entity_type.clone(),
        e.entity_id.clone().unwrap_or_default(),
        e.details.to_string(),
    ];
    cells
        .iter()
        .map(|c| csv_cell(c))
        .collect::<Vec<_>>()
        .join(",")
}

/// CSV with a header line and CRLF line ends (RFC 4180).
pub fn to_csv(entries: &[AuditEntry]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push_str("\r\n");
    for e in entries {
        out.push_str(&csv_line(e));
        out.push_str("\r\n");
    }
    out
}

/// A JSON array.
pub fn to_json(entries: &[AuditEntry]) -> String {
    serde_json::to_string_pretty(entries).expect("audit JSON")
}

/// One JSON object per line (JSON Lines). Files of several runs can be
/// joined line by line.
pub fn to_json_lines(entries: &[AuditEntry]) -> String {
    let mut out = String::new();
    for e in entries {
        out.push_str(&serde_json::to_string(e).expect("audit JSON"));
        out.push('\n');
    }
    out
}

/// Deletes entries older than `days` days. With `exported_up_to`, only
/// entries that the automatic export already wrote are deleted. Returns the
/// number of deleted entries.
pub async fn prune(
    pool: &PgPool,
    tenant: TenantId,
    days: i32,
    exported_up_to: Option<i64>,
) -> CoreResult<u64> {
    if days <= 0 {
        return Ok(0);
    }
    let done = sqlx::query(
        "DELETE FROM audit_log WHERE tenant_id = $1
           AND created_at < now() - make_interval(days => $2)
           AND ($3::bigint IS NULL OR id <= $3)",
    )
    .bind(tenant)
    .bind(days)
    .bind(exported_up_to)
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, action: &str, details: serde_json::Value) -> AuditEntry {
        AuditEntry {
            id,
            user_id: None,
            username: Some("admin".into()),
            action: action.into(),
            entity_type: "extension".into(),
            entity_id: Some("e1".into()),
            details,
            ip: Some("192.0.2.1".into()),
            created_at: DateTime::parse_from_rfc3339("2026-10-16T09:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    #[test]
    fn csv_quotes_and_guards_formulas() {
        assert_eq!(csv_cell("plain"), "plain");
        assert_eq!(csv_cell("a,b"), "\"a,b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("=1+1"), "'=1+1");
        assert_eq!(csv_cell("@cmd"), "'@cmd");
        let csv = to_csv(&[entry(7, "update", serde_json::json!({"name": "A, B"}))]);
        let mut lines = csv.split("\r\n");
        assert_eq!(lines.next(), Some(CSV_HEADER));
        assert_eq!(
            lines.next(),
            Some(
                "7,2026-10-16T09:00:00+00:00,,admin,192.0.2.1,update,extension,e1,\"{\"\"name\"\":\"\"A, B\"\"}\""
            )
        );
    }

    #[test]
    fn json_lines_has_one_object_per_line() {
        let out = to_json_lines(&[
            entry(1, "create", serde_json::json!({})),
            entry(2, "delete", serde_json::json!({"x": "multi\nline"})),
        ]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["id"], 2);
        assert_eq!(second["details"]["x"], "multi\nline");
        let array: Vec<serde_json::Value> =
            serde_json::from_str(&to_json(&[entry(1, "create", serde_json::json!({}))])).unwrap();
        assert_eq!(array.len(), 1);
    }
}
