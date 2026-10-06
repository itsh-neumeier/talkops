//! Time conditions: route calls by opening hours, public holidays and a
//! manual override ("closed now", e.g. for an emergency service).

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::holidays;
use crate::numbering;
use crate::tenant::TenantId;
use crate::trunks::NumberDestination;

/// Opening hours: weekday (`mon` … `sun`) → list of `["HH:MM", "HH:MM"]`.
pub type Schedule = BTreeMap<String, Vec<[String; 2]>>;

const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct TimeCondition {
    pub id: Uuid,
    pub number: Option<String>,
    pub name: String,
    #[schema(value_type = Object)]
    pub schedule: sqlx::types::Json<Schedule>,
    pub holiday_region: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub closed_dates: Vec<NaiveDate>,
    /// `auto`, `open` or `closed`.
    pub r#override: String,
    pub open_type: NumberDestination,
    pub open_id: Option<Uuid>,
    pub closed_type: NumberDestination,
    pub closed_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct TimeConditionInput {
    #[serde(default)]
    pub number: Option<String>,
    pub name: String,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub schedule: Schedule,
    #[serde(default)]
    pub holiday_region: Option<String>,
    #[serde(default)]
    #[schema(value_type = Vec<String>)]
    pub closed_dates: Vec<NaiveDate>,
    #[serde(default = "auto")]
    pub r#override: String,
    pub open_type: NumberDestination,
    #[serde(default)]
    pub open_id: Option<Uuid>,
    pub closed_type: NumberDestination,
    #[serde(default)]
    pub closed_id: Option<Uuid>,
}

fn auto() -> String {
    "auto".into()
}

/// Why a time condition is open or closed right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct State {
    pub open: bool,
    /// `override`, `holiday`, `closed_date`, `schedule`.
    pub reason: String,
    pub holiday: Option<String>,
}

fn parse_time(s: &str) -> Option<NaiveTime> {
    if s == "24:00" {
        return NaiveTime::from_hms_opt(23, 59, 59);
    }
    NaiveTime::parse_from_str(s, "%H:%M").ok()
}

fn weekday_key(day: Weekday) -> &'static str {
    DAYS[day.num_days_from_monday() as usize]
}

impl TimeCondition {
    /// Evaluates the condition at `now` in the tenant's time zone.
    pub fn state(&self, now: DateTime<Utc>, zone: &str) -> State {
        let tz: Tz = zone.parse().unwrap_or(chrono_tz::Europe::Berlin);
        let local = now.with_timezone(&tz);
        let date = local.date_naive();
        match self.r#override.as_str() {
            "open" | "closed" => {
                return State {
                    open: self.r#override == "open",
                    reason: "override".into(),
                    holiday: None,
                };
            }
            _ => {}
        }
        if let Some(h) = self
            .holiday_region
            .as_deref()
            .and_then(|r| holidays::holiday_on(r, date))
        {
            return State {
                open: false,
                reason: "holiday".into(),
                holiday: Some(h.name.to_owned()),
            };
        }
        if self.closed_dates.contains(&date) {
            return State {
                open: false,
                reason: "closed_date".into(),
                holiday: None,
            };
        }
        let time = local.time();
        let open = self
            .schedule
            .get(weekday_key(date.weekday()))
            .is_some_and(|ranges| {
                ranges.iter().any(|[from, to]| {
                    matches!((parse_time(from), parse_time(to)), (Some(f), Some(t)) if f <= time && time < t)
                })
            });
        State {
            open,
            reason: "schedule".into(),
            holiday: None,
        }
    }

    /// Destination for the current state.
    pub fn destination(&self, open: bool) -> (NumberDestination, Option<Uuid>) {
        if open {
            (self.open_type, self.open_id)
        } else {
            (self.closed_type, self.closed_id)
        }
    }
}

const COLUMNS: &str = "id, number, name, schedule, holiday_region, closed_dates, override, \
                       open_type, open_id, closed_type, closed_id";

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<TimeCondition>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM time_conditions WHERE tenant_id = $1 ORDER BY number NULLS LAST, name"
    );
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?)
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<TimeCondition> {
    let sql = format!("SELECT {COLUMNS} FROM time_conditions WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?)
}

async fn validate(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &TimeConditionInput,
    emergency: &[String],
) -> CoreResult<()> {
    if input.name.trim().is_empty() || input.name.chars().count() > 64 {
        return Err(CoreError::Validation(
            "name is required (max. 64 characters)".into(),
        ));
    }
    if let Some(n) = input.number.as_deref().filter(|n| !n.is_empty()) {
        numbering::validate_number(n, emergency)?;
        numbering::ensure_free(pool, tenant, n, id).await?;
    }
    for (day, ranges) in &input.schedule {
        if !DAYS.contains(&day.as_str()) {
            return Err(CoreError::Validation(format!("unknown weekday `{day}`")));
        }
        if ranges.len() > 8 {
            return Err(CoreError::Validation(
                "at most 8 time ranges per day".into(),
            ));
        }
        for [from, to] in ranges {
            match (parse_time(from), parse_time(to)) {
                (Some(f), Some(t)) if f < t => {}
                _ => {
                    return Err(CoreError::Validation(format!(
                        "invalid time range {from}–{to} on {day}"
                    )));
                }
            }
        }
    }
    if let Some(region) = input.holiday_region.as_deref() {
        if !holidays::is_region(region) {
            return Err(CoreError::Validation(format!(
                "unknown holiday region `{region}`"
            )));
        }
    }
    if input.closed_dates.len() > 366 {
        return Err(CoreError::Validation("too many closed dates".into()));
    }
    if !["auto", "open", "closed"].contains(&input.r#override.as_str()) {
        return Err(CoreError::Validation("invalid override".into()));
    }
    for (kind, target) in [
        (input.open_type, input.open_id),
        (input.closed_type, input.closed_id),
    ] {
        if kind == NumberDestination::TimeCondition && id.is_some() && target == id {
            return Err(CoreError::Validation(
                "a time condition cannot point to itself".into(),
            ));
        }
        numbering::check_destination(pool, tenant, kind, target).await?;
    }
    Ok(())
}

fn number(input: &TimeConditionInput) -> Option<&str> {
    input.number.as_deref().filter(|n| !n.is_empty())
}

fn target(kind: NumberDestination, id: Option<Uuid>) -> Option<Uuid> {
    id.filter(|_| kind != NumberDestination::None)
}

pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    input: &TimeConditionInput,
    emergency: &[String],
) -> CoreResult<TimeCondition> {
    validate(pool, tenant, None, input, emergency).await?;
    let sql = format!(
        "INSERT INTO time_conditions (tenant_id, number, name, schedule, holiday_region, closed_dates,
                                      override, open_type, open_id, closed_type, closed_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(number(input))
        .bind(input.name.trim())
        .bind(sqlx::types::Json(&input.schedule))
        .bind(&input.holiday_region)
        .bind(&input.closed_dates)
        .bind(&input.r#override)
        .bind(input.open_type)
        .bind(target(input.open_type, input.open_id))
        .bind(input.closed_type)
        .bind(target(input.closed_type, input.closed_id))
        .fetch_one(pool)
        .await?)
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &TimeConditionInput,
    emergency: &[String],
) -> CoreResult<TimeCondition> {
    get(pool, tenant, id).await?;
    validate(pool, tenant, Some(id), input, emergency).await?;
    let sql = format!(
        "UPDATE time_conditions SET number = $3, name = $4, schedule = $5, holiday_region = $6,
             closed_dates = $7, override = $8, open_type = $9, open_id = $10, closed_type = $11,
             closed_id = $12, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(number(input))
        .bind(input.name.trim())
        .bind(sqlx::types::Json(&input.schedule))
        .bind(&input.holiday_region)
        .bind(&input.closed_dates)
        .bind(&input.r#override)
        .bind(input.open_type)
        .bind(target(input.open_type, input.open_id))
        .bind(input.closed_type)
        .bind(target(input.closed_type, input.closed_id))
        .fetch_one(pool)
        .await?)
}

/// Sets the override (UI switch or feature code).
pub async fn set_override(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    value: &str,
) -> CoreResult<TimeCondition> {
    if !["auto", "open", "closed"].contains(&value) {
        return Err(CoreError::Validation("invalid override".into()));
    }
    let sql = format!(
        "UPDATE time_conditions SET override = $3, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(value)
        .fetch_one(pool)
        .await?)
}

pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM time_conditions WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn tc() -> TimeCondition {
        let mut schedule = Schedule::new();
        for day in ["mon", "tue", "wed", "thu", "fri"] {
            schedule.insert(
                day.into(),
                vec![
                    ["08:00".into(), "12:00".into()],
                    ["13:00".into(), "17:00".into()],
                ],
            );
        }
        TimeCondition {
            id: Uuid::nil(),
            number: None,
            name: "Office".into(),
            schedule: sqlx::types::Json(schedule),
            holiday_region: Some("DE-BY".into()),
            closed_dates: vec![NaiveDate::from_ymd_opt(2026, 12, 24).unwrap()],
            r#override: "auto".into(),
            open_type: NumberDestination::None,
            open_id: None,
            closed_type: NumberDestination::None,
            closed_id: None,
        }
    }

    /// UTC instant for a Berlin local time.
    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        chrono_tz::Europe::Berlin
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn evaluates_schedule_holidays_and_override() {
        let z = "Europe/Berlin";
        let t = tc();
        // Monday 5 Oct 2026.
        assert!(t.state(at(2026, 10, 5, 9, 0), z).open);
        assert!(!t.state(at(2026, 10, 5, 12, 30), z).open);
        assert!(t.state(at(2026, 10, 5, 16, 59), z).open);
        assert!(!t.state(at(2026, 10, 5, 17, 0), z).open);
        // Saturday.
        assert!(!t.state(at(2026, 10, 10, 10, 0), z).open);
        // Allerheiligen falls on a Sunday in 2026; Fronleichnam is a Thursday.
        let s = t.state(at(2026, 6, 4, 10, 0), z);
        assert_eq!(
            (s.open, s.holiday.as_deref()),
            (false, Some("Fronleichnam"))
        );
        assert_eq!(t.state(at(2026, 12, 24, 10, 0), z).reason, "closed_date");
        // Local time, not UTC: 08:30 Berlin is 06:30 UTC in summer.
        assert!(t.state(at(2026, 7, 6, 8, 30), z).open);

        let mut forced = tc();
        forced.r#override = "closed".into();
        let s = forced.state(at(2026, 10, 5, 9, 0), z);
        assert_eq!((s.open, s.reason.as_str()), (false, "override"));
        forced.r#override = "open".into();
        assert!(forced.state(at(2026, 10, 10, 3, 0), z).open);
    }
}
