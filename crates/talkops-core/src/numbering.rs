//! The internal number space shared by extensions, ring groups, menus and
//! queues, and validation of call destinations.

use sqlx::PgExecutor;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;
use crate::trunks::NumberDestination;

/// Tables with an internal `number` column and the destination they represent.
const NUMBERED: &[(&str, &str)] = &[
    ("extensions", "extension"),
    ("ring_groups", "ring_group"),
    ("time_conditions", "time_condition"),
];

/// Table holding the targets of a destination kind.
fn table(kind: NumberDestination) -> Option<&'static str> {
    match kind {
        NumberDestination::None => None,
        NumberDestination::Extension | NumberDestination::Voicemail => Some("extensions"),
        NumberDestination::RingGroup => Some("ring_groups"),
        NumberDestination::TimeCondition => Some("time_conditions"),
        NumberDestination::Ivr => Some("ivr_menus"),
        NumberDestination::Queue => Some("queues"),
    }
}

fn union_sql() -> String {
    NUMBERED
        .iter()
        .map(|(table, kind)| {
            format!(
                "SELECT '{kind}'::number_destination AS kind, id FROM {table} \
                 WHERE tenant_id = $1 AND number = $2"
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ")
}

/// What an internal number belongs to.
pub async fn resolve<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    number: &str,
) -> CoreResult<Option<(NumberDestination, Uuid)>> {
    let sql = format!("{} LIMIT 1", union_sql());
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(number)
        .fetch_optional(db)
        .await?)
}

/// Fails if `number` is used by anything but `except`.
pub async fn ensure_free<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    number: &str,
    except: Option<Uuid>,
) -> CoreResult<()> {
    let sql = format!(
        "SELECT kind FROM ({}) used WHERE id IS DISTINCT FROM $3 LIMIT 1",
        union_sql()
    );
    let taken: Option<NumberDestination> = sqlx::query_scalar(&sql)
        .bind(tenant)
        .bind(number)
        .bind(except)
        .fetch_optional(db)
        .await?;
    match taken {
        Some(kind) => Err(CoreError::Conflict(format!(
            "number {number} is already used ({})",
            serde_json::to_value(kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        ))),
        None => Ok(()),
    }
}

/// Rules for internal numbers: 2–8 digits, no leading 0 (trunk prefix), no
/// collision with emergency or 11x service numbers.
pub fn validate_number(n: &str, emergency: &[String]) -> CoreResult<()> {
    if !(2..=8).contains(&n.len()) || !n.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CoreError::Validation(
            "internal numbers must have 2-8 digits".into(),
        ));
    }
    if n.starts_with('0') {
        return Err(CoreError::Validation(
            "internal numbers must not start with 0 (trunk prefix)".into(),
        ));
    }
    if emergency
        .iter()
        .any(|e| n.starts_with(e.as_str()) || e.starts_with(n))
        || n.starts_with("11")
    {
        return Err(CoreError::Validation(
            "number collides with emergency or service numbers".into(),
        ));
    }
    Ok(())
}

/// Checks that a destination points to an existing target of the tenant.
pub async fn check_destination<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    kind: NumberDestination,
    id: Option<Uuid>,
) -> CoreResult<()> {
    let Some(table) = table(kind) else {
        return Ok(());
    };
    let id = id.ok_or_else(|| CoreError::Validation("destination is required".into()))?;
    let sql = format!("SELECT EXISTS (SELECT 1 FROM {table} WHERE tenant_id = $1 AND id = $2)");
    let exists: bool = sqlx::query_scalar(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(CoreError::Validation("unknown destination".into()))
    }
}
