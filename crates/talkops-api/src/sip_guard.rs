//! Counts failed SIP authentications per source address from FreeSWITCH's
//! `sofia::register_failure` events (wrong password or unknown user, for
//! REGISTER and INVITE alike) and bans addresses that fail too often.
//! Banned addresses get no directory entries, so every authentication from
//! them fails (see `routes::fs`).

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use talkops_core::sip_guard::{self, GuardSettings};
use talkops_core::tenant::TenantId;
use tokio::sync::broadcast::error::RecvError;

use crate::esl::EslHandle;

const EVENT: &str = "sofia::register_failure";
/// Addresses tracked at once; beyond that the oldest are forgotten.
const MAX_TRACKED: usize = 10_000;
const SETTINGS_TTL: Duration = Duration::from_secs(30);

/// Recent failures per address.
#[derive(Default)]
pub struct Tracker {
    failures: Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
}

impl Tracker {
    /// Records a failure at `now`; returns the number of failures within
    /// `window` once it reaches `max` (and forgets them).
    pub fn record(&self, ip: IpAddr, now: Instant, window: Duration, max: usize) -> Option<usize> {
        let mut map = self.failures.lock().expect("tracker lock");
        if map.len() >= MAX_TRACKED && !map.contains_key(&ip) {
            map.retain(|_, v| v.back().is_some_and(|t| now.duration_since(*t) < window));
            if map.len() >= MAX_TRACKED {
                return None;
            }
        }
        let list = map.entry(ip).or_default();
        list.push_back(now);
        while list
            .front()
            .is_some_and(|t| now.duration_since(*t) >= window)
        {
            list.pop_front();
        }
        if list.len() >= max {
            let n = list.len();
            map.remove(&ip);
            Some(n)
        } else {
            None
        }
    }
}

/// Never banned: FreeSWITCH itself and the WebRTC relay connect from here.
fn always_trusted(ip: IpAddr) -> bool {
    ip.is_loopback() || ip.is_unspecified()
}

struct Guard {
    db: PgPool,
    tracker: Tracker,
    settings: Mutex<Option<(Instant, GuardSettings)>>,
}

impl Guard {
    async fn settings(&self) -> Option<GuardSettings> {
        if let Some((at, s)) = self.settings.lock().expect("settings lock").as_ref()
            && at.elapsed() < SETTINGS_TTL
        {
            return Some(s.clone());
        }
        let s = sip_guard::get(&self.db, TenantId::DEFAULT).await.ok()?;
        *self.settings.lock().expect("settings lock") = Some((Instant::now(), s.clone()));
        Some(s)
    }

    async fn failure(&self, ip: IpAddr, user: &str) {
        if always_trusted(ip) {
            return;
        }
        let Some(s) = self.settings().await else {
            return;
        };
        if !s.enabled {
            return;
        }
        let window = Duration::from_secs(s.window_minutes.max(1) as u64 * 60);
        let Some(n) =
            self.tracker
                .record(ip, Instant::now(), window, s.max_failures.max(1) as usize)
        else {
            return;
        };
        let tenant = TenantId::DEFAULT;
        if sip_guard::is_trusted(&self.db, tenant, ip)
            .await
            .unwrap_or(true)
        {
            return;
        }
        match sip_guard::ban(&self.db, tenant, ip, n as i32, user, s.ban_minutes).await {
            Ok(ban) => tracing::warn!(
                %ip,
                failures = n,
                user,
                until = %ban.banned_until,
                "too many failed SIP logins, address banned"
            ),
            Err(err) => tracing::error!(%ip, error = %err, "cannot store SIP ban"),
        }
    }
}

/// Listens for failed SIP logins while FreeSWITCH is connected.
pub fn spawn(db: PgPool, esl: EslHandle) {
    let guard = Guard {
        db,
        tracker: Tracker::default(),
        settings: Mutex::new(None),
    };
    tokio::spawn(async move {
        loop {
            if let Some(client) = esl.get().await {
                let mut events = client.events();
                match client.subscribe(&["CUSTOM", EVENT]).await {
                    Ok(()) => loop {
                        // The channel outlives a dropped connection: check it
                        // regularly and resubscribe on the new one.
                        let Ok(next) =
                            tokio::time::timeout(Duration::from_secs(5), events.recv()).await
                        else {
                            if client.is_connected() {
                                continue;
                            }
                            break;
                        };
                        match next {
                            Ok(ev) => {
                                if ev.headers.get("Event-Subclass") != Some(EVENT) {
                                    continue;
                                }
                                let ip = ev.headers.get("network-ip").and_then(|v| v.parse().ok());
                                let user = ev.headers.get("to-user").unwrap_or_default();
                                if let Some(ip) = ip {
                                    guard.failure(ip, user).await;
                                }
                            }
                            Err(RecvError::Lagged(n)) => {
                                tracing::warn!(skipped = n, "SIP guard missed events");
                            }
                            Err(RecvError::Closed) => break,
                        }
                    },
                    Err(err) => {
                        tracing::warn!(error = %err, "cannot subscribe to SIP login failures")
                    }
                }
            }
            if let Err(err) = sip_guard::purge_expired(&guard.db).await {
                tracing::warn!(error = %err, "cannot purge expired SIP bans");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_failures_in_window() {
        let t = Tracker::default();
        let ip: IpAddr = "203.0.113.7".parse().unwrap();
        let other: IpAddr = "203.0.113.8".parse().unwrap();
        let start = Instant::now();
        let w = Duration::from_secs(600);
        for i in 0..4 {
            assert_eq!(t.record(ip, start + Duration::from_secs(i), w, 5), None);
        }
        assert_eq!(t.record(other, start, w, 5), None);
        // The first failures fall out of the window.
        assert_eq!(t.record(ip, start + Duration::from_secs(601), w, 5), None);
        // Window now holds 601 and the 605s.
        let late = start + Duration::from_secs(605);
        assert_eq!(t.record(ip, late, w, 5), None);
        assert_eq!(t.record(ip, late, w, 5), None);
        assert_eq!(t.record(ip, late, w, 5), None);
        assert_eq!(t.record(ip, late, w, 5), Some(5));
        // Counting starts over after a ban.
        assert_eq!(t.record(ip, late, w, 5), None);
        assert!(always_trusted("127.0.0.1".parse().unwrap()));
        assert!(always_trusted("::1".parse().unwrap()));
        assert!(!always_trusted(ip));
    }
}
