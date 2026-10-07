//! Bans for addresses that keep failing SIP authentication (password
//! guessing, scanners). The failures are counted by the server from
//! FreeSWITCH events; this module stores settings and bans.

use std::net::IpAddr;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct GuardSettings {
    pub enabled: bool,
    pub max_failures: i32,
    pub window_minutes: i32,
    pub ban_minutes: i32,
    /// Addresses or networks (CIDR) that are never banned.
    pub trusted_networks: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct GuardSettingsInput {
    pub enabled: bool,
    pub max_failures: i32,
    pub window_minutes: i32,
    pub ban_minutes: i32,
    #[serde(default)]
    pub trusted_networks: Vec<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Ban {
    pub ip: String,
    pub failures: i32,
    pub last_user: String,
    pub banned_at: DateTime<Utc>,
    pub banned_until: DateTime<Utc>,
}

const COLUMNS: &str = "enabled, max_failures, window_minutes, ban_minutes, \
     ARRAY(SELECT CASE WHEN masklen(n) = CASE WHEN family(n) = 4 THEN 32 ELSE 128 END \
                       THEN host(n) ELSE text(n) END \
           FROM unnest(trusted_networks) AS n) AS trusted_networks";

pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<GuardSettings> {
    let sql = format!("SELECT {COLUMNS} FROM sip_guard_settings WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_one(pool).await?)
}

/// Parses `192.168.1.0/24`, `10.0.0.5` or `2001:db8::/32`.
pub fn parse_network(s: &str) -> Option<(IpAddr, u8)> {
    let s = s.trim();
    let (addr, prefix) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    let ip: IpAddr = addr.parse().ok()?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    let prefix = match prefix {
        Some(p) => p.parse::<u8>().ok().filter(|p| *p <= max)?,
        None => max,
    };
    Some((ip, prefix))
}

/// Whether `ip` is inside the network `(addr, prefix)`.
pub fn network_contains(net: (IpAddr, u8), ip: IpAddr) -> bool {
    match (net.0, ip) {
        (IpAddr::V4(n), IpAddr::V4(a)) => {
            let bits = u32::from(net.1.min(32));
            let mask = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            u32::from(n) & mask == u32::from(a) & mask
        }
        (IpAddr::V6(n), IpAddr::V6(a)) => {
            let bits = u32::from(net.1.min(128));
            let mask = if bits == 0 {
                0
            } else {
                u128::MAX << (128 - bits)
            };
            u128::from(n) & mask == u128::from(a) & mask
        }
        _ => false,
    }
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    i: &GuardSettingsInput,
) -> CoreResult<GuardSettings> {
    let invalid = |m: &str| Err(CoreError::Validation(m.into()));
    if !(3..=1000).contains(&i.max_failures) {
        return invalid("allow 3 to 1000 failed attempts");
    }
    if !(1..=1440).contains(&i.window_minutes) {
        return invalid("the time window must be 1 to 1440 minutes");
    }
    if !(1..=525_600).contains(&i.ban_minutes) {
        return invalid("bans last 1 minute to 1 year");
    }
    let mut networks = Vec::new();
    for n in i
        .trusted_networks
        .iter()
        .map(|n| n.trim())
        .filter(|n| !n.is_empty())
    {
        let Some((ip, prefix)) = parse_network(n) else {
            return Err(CoreError::Validation(format!("invalid network: {n}")));
        };
        networks.push(format!("{ip}/{prefix}"));
    }
    if networks.len() > 100 {
        return invalid("at most 100 trusted networks");
    }
    let sql = format!(
        "UPDATE sip_guard_settings SET enabled = $2, max_failures = $3, window_minutes = $4,
             ban_minutes = $5, trusted_networks = $6::text[]::inet[], updated_at = now()
         WHERE tenant_id = $1 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(i.enabled)
        .bind(i.max_failures)
        .bind(i.window_minutes)
        .bind(i.ban_minutes)
        .bind(&networks)
        .fetch_one(pool)
        .await?)
}

/// Whether `ip` is in one of the trusted networks.
pub async fn is_trusted(pool: &PgPool, tenant: TenantId, ip: IpAddr) -> CoreResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT $2::inet <<= ANY(trusted_networks) FROM sip_guard_settings WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(ip.to_string())
    .fetch_optional(pool)
    .await?
    .unwrap_or(false))
}

/// Whether `ip` is banned right now (guard enabled, not trusted).
pub async fn is_banned(pool: &PgPool, tenant: TenantId, ip: IpAddr) -> CoreResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM sip_bans b JOIN sip_guard_settings s USING (tenant_id)
             WHERE b.tenant_id = $1 AND b.ip = $2::inet AND b.banned_until > now()
               AND s.enabled AND NOT (b.ip <<= ANY(s.trusted_networks)))",
    )
    .bind(tenant)
    .bind(ip.to_string())
    .fetch_one(pool)
    .await?)
}

/// Bans `ip` for `minutes` (extends an existing ban).
pub async fn ban(
    pool: &PgPool,
    tenant: TenantId,
    ip: IpAddr,
    failures: i32,
    last_user: &str,
    minutes: i32,
) -> CoreResult<Ban> {
    let until = Utc::now() + Duration::minutes(i64::from(minutes));
    let user: String = last_user.chars().take(64).collect();
    Ok(sqlx::query_as(
        "INSERT INTO sip_bans (tenant_id, ip, failures, last_user, banned_until)
         VALUES ($1, $2::inet, $3, $4, $5)
         ON CONFLICT (tenant_id, ip) DO UPDATE
             SET failures = EXCLUDED.failures, last_user = EXCLUDED.last_user,
                 banned_at = now(), banned_until = EXCLUDED.banned_until
         RETURNING host(ip) AS ip, failures, last_user, banned_at, banned_until",
    )
    .bind(tenant)
    .bind(ip.to_string())
    .bind(failures)
    .bind(user)
    .bind(until)
    .fetch_one(pool)
    .await?)
}

/// Active bans, newest first.
pub async fn list_bans(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<Ban>> {
    Ok(sqlx::query_as(
        "SELECT host(ip) AS ip, failures, last_user, banned_at, banned_until FROM sip_bans
         WHERE tenant_id = $1 AND banned_until > now() ORDER BY banned_at DESC LIMIT 1000",
    )
    .bind(tenant)
    .fetch_all(pool)
    .await?)
}

/// Lifts a ban.
pub async fn unban(pool: &PgPool, tenant: TenantId, ip: IpAddr) -> CoreResult<()> {
    let n = sqlx::query("DELETE FROM sip_bans WHERE tenant_id = $1 AND ip = $2::inet")
        .bind(tenant)
        .bind(ip.to_string())
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Deletes expired bans; returns how many.
pub async fn purge_expired(pool: &PgPool) -> CoreResult<u64> {
    Ok(
        sqlx::query("DELETE FROM sip_bans WHERE banned_until < now() - interval '1 day'")
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networks() {
        assert_eq!(
            parse_network("192.168.1.0/24"),
            Some(("192.168.1.0".parse().unwrap(), 24))
        );
        assert_eq!(
            parse_network(" 10.0.0.5 "),
            Some(("10.0.0.5".parse().unwrap(), 32))
        );
        assert_eq!(
            parse_network("2001:db8::/32"),
            Some(("2001:db8::".parse().unwrap(), 32))
        );
        assert_eq!(parse_network("10.0.0.0/33"), None);
        let lan = parse_network("192.168.1.0/24").unwrap();
        assert!(network_contains(lan, "192.168.1.200".parse().unwrap()));
        assert!(!network_contains(lan, "192.168.2.1".parse().unwrap()));
        assert!(!network_contains(lan, "::1".parse().unwrap()));
        let any = parse_network("0.0.0.0/0").unwrap();
        assert!(network_contains(any, "8.8.8.8".parse().unwrap()));
        let v6 = parse_network("fd00::/8").unwrap();
        assert!(network_contains(v6, "fd12::1".parse().unwrap()));
        assert_eq!(parse_network("example.com"), None);
    }
}
