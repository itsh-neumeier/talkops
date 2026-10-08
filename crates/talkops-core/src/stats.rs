//! Call statistics for the dashboard: calls per time slot by direction,
//! missed calls and answer rate over the last hour, day, week or month.

use chrono::{DateTime, Duration, DurationRound, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::error::CoreResult;
use crate::tenant::TenantId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
pub enum Range {
    /// Last hour in 5-minute slots.
    #[serde(rename = "1h")]
    Hour,
    /// Last 24 hours in hourly slots.
    #[serde(rename = "1d")]
    Day,
    /// Last 7 days (calendar days in the tenant's time zone).
    #[serde(rename = "1w")]
    Week,
    /// Last 30 days.
    #[serde(rename = "1m")]
    Month,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Bucket {
    pub start: DateTime<Utc>,
    pub inbound: i64,
    pub outbound: i64,
    pub internal: i64,
    /// Inbound calls nobody answered.
    pub missed: i64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CallStats {
    pub range: Range,
    /// Slot boundaries: `buckets[i]` covers `[start, next start)`; the
    /// last one ends at `end`.
    pub buckets: Vec<Bucket>,
    pub end: DateTime<Utc>,
    pub total: i64,
    pub inbound: i64,
    pub outbound: i64,
    pub internal: i64,
    pub missed: i64,
    /// Answered inbound calls in percent (`None` without inbound calls).
    pub answer_rate: Option<f64>,
    /// Average talk time of answered calls in seconds.
    pub avg_talk_secs: i64,
}

/// Slot boundaries for `range` ending with the slot containing `now`.
pub fn boundaries(range: Range, now: DateTime<Utc>, zone: &str) -> Vec<DateTime<Utc>> {
    let fixed = |step: Duration, count: i64| {
        let end = now.duration_trunc(step).unwrap_or(now) + step;
        (0..=count)
            .map(|i| end - step * (count - i) as i32)
            .collect::<Vec<_>>()
    };
    match range {
        Range::Hour => fixed(Duration::minutes(5), 12),
        Range::Day => fixed(Duration::hours(1), 24),
        Range::Week | Range::Month => {
            let days: u64 = if range == Range::Week { 7 } else { 30 };
            let tz: Tz = zone.parse().unwrap_or(chrono_tz::Europe::Berlin);
            let today = now.with_timezone(&tz).date_naive();
            (0..=days)
                .map(|i| {
                    let day = today + chrono::Days::new(1) - chrono::Days::new(days - i);
                    tz.from_local_datetime(&day.and_time(NaiveTime::MIN))
                        .earliest()
                        .map(|t| t.with_timezone(&Utc))
                        .unwrap_or_else(|| day.and_time(NaiveTime::MIN).and_utc())
                })
                .collect()
        }
    }
}

pub async fn calls(
    pool: &PgPool,
    tenant: TenantId,
    range: Range,
    now: DateTime<Utc>,
    zone: &str,
) -> CoreResult<CallStats> {
    let bounds = boundaries(range, now, zone);
    let buckets: Vec<Bucket> = sqlx::query_as(
        "WITH b AS (
             SELECT t, lead(t) OVER (ORDER BY t) AS e FROM unnest($2::timestamptz[]) AS t
         )
         SELECT b.t AS start,
                count(c.id) FILTER (WHERE c.direction = 'inbound') AS inbound,
                count(c.id) FILTER (WHERE c.direction = 'outbound') AS outbound,
                count(c.id) FILTER (WHERE c.direction = 'internal') AS internal,
                count(c.id) FILTER (WHERE c.direction = 'inbound' AND c.answered_at IS NULL)
                    AS missed
         FROM b
         LEFT JOIN cdr c ON c.tenant_id = $1 AND c.started_at >= b.t AND c.started_at < b.e
         WHERE b.e IS NOT NULL
         GROUP BY b.t
         ORDER BY b.t",
    )
    .bind(tenant)
    .bind(&bounds)
    .fetch_all(pool)
    .await?;
    let start = bounds[0];
    let end = *bounds.last().unwrap_or(&now);
    let avg: Option<f64> = sqlx::query_scalar(
        "SELECT avg(billsec)::float8 FROM cdr
         WHERE tenant_id = $1 AND started_at >= $2 AND started_at < $3
           AND answered_at IS NOT NULL",
    )
    .bind(tenant)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    let sum = |f: fn(&Bucket) -> i64| buckets.iter().map(f).sum::<i64>();
    let (inbound, outbound, internal, missed) = (
        sum(|b| b.inbound),
        sum(|b| b.outbound),
        sum(|b| b.internal),
        sum(|b| b.missed),
    );
    Ok(CallStats {
        range,
        end,
        total: inbound + outbound + internal,
        inbound,
        outbound,
        internal,
        missed,
        answer_rate: (inbound > 0)
            .then(|| ((inbound - missed) as f64 * 1000.0 / inbound as f64).round() / 10.0),
        avg_talk_secs: avg.unwrap_or(0.0).round() as i64,
        buckets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_boundaries() {
        let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 7, 30).unwrap();
        let h = boundaries(Range::Hour, now, "Europe/Berlin");
        assert_eq!(h.len(), 13);
        assert_eq!(
            *h.last().unwrap(),
            Utc.with_ymd_and_hms(2026, 3, 29, 10, 10, 0).unwrap()
        );
        assert_eq!(h[0], Utc.with_ymd_and_hms(2026, 3, 29, 9, 10, 0).unwrap());
        let d = boundaries(Range::Day, now, "Europe/Berlin");
        assert_eq!(d.len(), 25);
        assert_eq!(
            *d.last().unwrap(),
            Utc.with_ymd_and_hms(2026, 3, 29, 11, 0, 0).unwrap()
        );
        // Calendar days in Berlin; 29 March 2026 is the switch to summer time.
        let w = boundaries(Range::Week, now, "Europe/Berlin");
        assert_eq!(w.len(), 8);
        assert_eq!(w[0], Utc.with_ymd_and_hms(2026, 3, 22, 23, 0, 0).unwrap());
        assert_eq!(w[6], Utc.with_ymd_and_hms(2026, 3, 28, 23, 0, 0).unwrap());
        assert_eq!(w[7], Utc.with_ymd_and_hms(2026, 3, 29, 22, 0, 0).unwrap());
        assert_eq!(boundaries(Range::Month, now, "UTC").len(), 31);
    }
}
