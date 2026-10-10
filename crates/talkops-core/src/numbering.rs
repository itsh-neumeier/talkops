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
    ("ivr_menus", "ivr"),
    ("queues", "queue"),
];

/// Feature codes of a fixed length, as handled by the FreeSWITCH dialplan
/// (`talkops-api` `fsxml::dialplan::feature_code`): park slots, call
/// forwarding off, DND on/off, voicemail.
pub const FIXED_FEATURE_CODES: &[&str] = &[
    "*51", "*52", "*53", "*54", "*55", "*56", "*57", "*58", "*59", "*73", "*78", "*79", "*97",
    "*98",
];

/// Feature codes followed by more digits: pickup `**<ext>`, time condition
/// `*30<n>`, hide caller ID `*31…`/`#31#…`, forwarding `*72<n>`, door
/// opener `*85[<n>]`/`*86[<n>]`.
pub const FEATURE_CODE_PREFIXES: &[&str] = &["**", "*30", "*31", "*72", "*85", "*86", "#31#"];

/// All internal numbers of a tenant.
pub async fn all_numbers<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<String>> {
    let sql = NUMBERED
        .iter()
        .map(|(table, _)| format!("SELECT number FROM {table} WHERE tenant_id = $1"))
        .collect::<Vec<_>>()
        .join(" UNION ")
        + " ORDER BY 1";
    Ok(sqlx::query_scalar(&sql).bind(tenant).fetch_all(db).await?)
}

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

/// Smallest number for new internal numbers: `*1`–`*99` are system codes
/// and `*<number>` reaches internal numbers from `*100`.
pub const MIN_INTERNAL_NUMBER: u32 = 100;

/// Fails if `number` is used by anything but `except`, or if a number below
/// [`MIN_INTERNAL_NUMBER`] is newly assigned (existing ones may be kept).
pub async fn ensure_free<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    number: &str,
    except: Option<Uuid>,
) -> CoreResult<()> {
    let sql = format!("SELECT kind, id FROM ({}) used", union_sql());
    let used: Vec<(NumberDestination, Uuid)> = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(number)
        .fetch_all(db)
        .await?;
    if let Some((kind, _)) = used.iter().find(|(_, id)| Some(*id) != except) {
        return Err(CoreError::Conflict(format!(
            "number {number} is already used ({})",
            serde_json::to_value(kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        )));
    }
    let kept = used.iter().any(|(_, id)| Some(*id) == except);
    if !kept && number.parse::<u32>().is_ok_and(|n| n < MIN_INTERNAL_NUMBER) {
        return Err(CoreError::Validation(format!(
            "internal numbers start at {MIN_INTERNAL_NUMBER} (1-99 are reserved for system codes)"
        )));
    }
    Ok(())
}

/// Rules for internal numbers: 2–8 digits (new numbers from 100, see
/// [`ensure_free`]; 2-digit numbers of older installations stay valid), no
/// leading 0 (trunk prefix), not the start or an extension of an emergency
/// number (110, 112, 1120 …: phones may dial those as soon as they are
/// typed), not the authority number 115.
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
        || n == "115"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_number_rules() {
        let emergency = vec!["110".to_owned(), "112".to_owned()];
        for ok in [
            "20", "100", "116", "118", "119", "610", "1150", "1180", "9999", "12345678",
        ] {
            assert!(validate_number(ok, &emergency).is_ok(), "{ok}");
        }
        for bad in [
            "1",
            "0815",
            "110",
            "112",
            "1120",
            "1105",
            "11",
            "115",
            "123456789",
            "6a0",
        ] {
            assert!(validate_number(bad, &emergency).is_err(), "{bad}");
        }
    }
}
