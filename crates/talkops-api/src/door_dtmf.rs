//! DTMF unlock codes of door stations. The dialplan marks the phone's leg of
//! a ring call (and of a direct call to the station) with [`MARK`]. This task
//! follows the DTMF digits of the marked legs over the Event Socket and opens
//! the door through the station's HTTP API when the digits end with the code
//! of door 1 or door 2. The station's own DTMF unlock is not affected.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::json;
use talkops_core::doors;
use talkops_core::tenant::TenantId;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::doors::DoorCtx;
use crate::esl::EslHandle;

/// Channel variable with the door station whose codes a leg listens for.
pub const MARK: &str = "talkops_door_dtmf";
/// A longer pause starts a new code.
const GAP: Duration = Duration::from_secs(4);
/// Digits evaluated per call: limits guessing within one call.
const MAX_DIGITS: usize = 40;
/// Longest code (see `doors::valid_unlock_code`).
const MAX_CODE: usize = 8;

struct Call {
    tenant: TenantId,
    station: Uuid,
    codes: [Option<String>; 2],
    /// Who presses the keys (logged with the opening).
    by: serde_json::Value,
    digits: String,
    last: Option<Instant>,
    total: usize,
}

/// A code was entered: open `door` of `station`.
#[derive(Debug, Clone, PartialEq)]
pub struct Unlock {
    pub tenant: TenantId,
    pub station: Uuid,
    pub door: i16,
    pub by: serde_json::Value,
}

/// The answered marked legs and their digits.
#[derive(Default)]
pub struct Matcher {
    calls: HashMap<String, Call>,
}

impl Matcher {
    /// A marked leg was answered; `codes` are the station's codes of door 1/2.
    pub fn answered(
        &mut self,
        uuid: &str,
        tenant: TenantId,
        station: Uuid,
        codes: [Option<String>; 2],
        by: serde_json::Value,
    ) {
        if codes.iter().all(Option::is_none) {
            return;
        }
        self.calls.insert(
            uuid.to_owned(),
            Call {
                tenant,
                station,
                codes,
                by,
                digits: String::new(),
                last: None,
                total: 0,
            },
        );
    }

    pub fn hangup(&mut self, uuid: &str) {
        self.calls.remove(uuid);
    }

    /// Adds a digit of leg `uuid`; returns the door to open when the digits
    /// end with one of the codes.
    pub fn digit(&mut self, uuid: &str, digit: char, now: Instant) -> Option<Unlock> {
        let call = self.calls.get_mut(uuid)?;
        if call.total >= MAX_DIGITS {
            return None;
        }
        call.total += 1;
        if call.total == MAX_DIGITS {
            tracing::warn!(station = %call.station, "too many DTMF digits, unlock codes ignored for this call");
        }
        if call.last.is_some_and(|t| now.duration_since(t) > GAP) {
            call.digits.clear();
        }
        call.last = Some(now);
        call.digits.push(digit);
        if call.digits.len() > MAX_CODE {
            call.digits.remove(0);
        }
        let door = call
            .codes
            .iter()
            .position(|c| c.as_deref().is_some_and(|c| call.digits.ends_with(c)))?;
        call.digits.clear();
        Some(Unlock {
            tenant: call.tenant,
            station: call.station,
            door: door as i16 + 1,
            by: call.by.clone(),
        })
    }
}

/// The device part of a channel name: `sofia/internal/8001vth@host` → `8001vth`.
fn device(channel: &str) -> &str {
    let user = channel.rsplit('/').next().unwrap_or(channel);
    user.split('@').next().unwrap_or(user)
}

/// Loads the codes of a newly answered marked leg.
async fn answered(ctx: &DoorCtx, matcher: &mut Matcher, ev: &talkops_esl::Event) {
    let h = &ev.headers;
    let (Some(uuid), Some(station), Some(tenant)) = (
        h.get("Unique-ID"),
        h.get(&format!("variable_{MARK}"))
            .and_then(|v| v.parse::<Uuid>().ok()),
        h.get("variable_talkops_tenant_id")
            .and_then(|v| v.parse::<Uuid>().ok())
            .map(TenantId),
    ) else {
        return;
    };
    let codes = match doors::get(&ctx.db, tenant, station).await {
        Ok(s) if s.detects_unlock_codes() => {
            doors::unlock_codes(&ctx.db, &ctx.secrets, tenant, station).await
        }
        Ok(_) => return,
        Err(err) => Err(err),
    };
    let codes = match codes {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(%station, error = %err, "cannot load door unlock codes");
            return;
        }
    };
    let mut by = json!({
        "dtmf": true,
        "device": device(h.get("Channel-Name").unwrap_or_default()),
    });
    if let Some(ext) = h.get("variable_talkops_extension_id") {
        by["extension_id"] = json!(ext);
    }
    matcher.answered(uuid, tenant, station, codes, by);
}

async fn open(ctx: DoorCtx, unlock: Unlock) {
    match doors::get(&ctx.db, unlock.tenant, unlock.station).await {
        // `open` logs the result in the event log.
        Ok(station) => {
            let _ = ctx
                .open(unlock.tenant, &station, unlock.door, unlock.by)
                .await;
        }
        Err(err) => tracing::warn!(station = %unlock.station, error = %err, "door station gone"),
    }
}

/// Handles one event; returns an opening to start.
pub async fn handle(
    ctx: &DoorCtx,
    matcher: &mut Matcher,
    ev: &talkops_esl::Event,
) -> Option<Unlock> {
    let uuid = ev.headers.get("Unique-ID")?;
    match ev.name()? {
        "CHANNEL_ANSWER" => {
            answered(ctx, matcher, ev).await;
            None
        }
        "CHANNEL_HANGUP_COMPLETE" => {
            matcher.hangup(uuid);
            None
        }
        "DTMF" => {
            let digit = ev.headers.get("DTMF-Digit")?.chars().next()?;
            matcher.digit(uuid, digit, Instant::now())
        }
        _ => None,
    }
}

/// Follows DTMF digits of marked legs while FreeSWITCH is connected.
pub fn spawn(ctx: DoorCtx, esl: EslHandle) {
    tokio::spawn(async move {
        let mut matcher = Matcher::default();
        loop {
            if let Some(client) = esl.get().await {
                let mut events = client.events();
                match client
                    .subscribe(&["CHANNEL_ANSWER", "CHANNEL_HANGUP_COMPLETE", "DTMF"])
                    .await
                {
                    Ok(()) => loop {
                        // Check the connection regularly and resubscribe on a new one.
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
                                if let Some(unlock) = handle(&ctx, &mut matcher, &ev).await {
                                    tokio::spawn(open(ctx.clone(), unlock));
                                }
                            }
                            Err(RecvError::Lagged(n)) => {
                                tracing::warn!(skipped = n, "door DTMF watcher missed events");
                            }
                            Err(RecvError::Closed) => break,
                        }
                    },
                    Err(err) => tracing::warn!(error = %err, "cannot subscribe to DTMF events"),
                }
                // Calls of the old connection are gone.
                matcher = Matcher::default();
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(codes: [Option<&str>; 2]) -> Matcher {
        let mut m = Matcher::default();
        m.answered(
            "leg",
            TenantId::DEFAULT,
            Uuid::nil(),
            codes.map(|c| c.map(Into::into)),
            json!({"device": "8001vth"}),
        );
        m
    }

    fn press(m: &mut Matcher, keys: &str, start: Instant) -> Vec<i16> {
        keys.chars()
            .enumerate()
            .filter_map(|(i, k)| m.digit("leg", k, start + Duration::from_millis(300 * i as u64)))
            .map(|u| u.door)
            .collect()
    }

    #[test]
    fn detects_codes() {
        let now = Instant::now();
        let mut m = matcher([Some("*1"), Some("#2")]);
        assert_eq!(press(&mut m, "*1", now), [1]);
        assert_eq!(press(&mut m, "#2", now), [2]);
        // Digits before the code do not matter; the code must be complete.
        assert_eq!(press(&mut m, "934*1", now), [1]);
        assert_eq!(press(&mut m, "*", now), Vec::<i16>::new());
        assert_eq!(press(&mut m, "3", now), Vec::<i16>::new());
        // Other legs and unmarked legs are ignored.
        assert_eq!(m.digit("other", '1', now), None);
        assert!(m.digit("leg", '*', now).is_none());
        let u = m.digit("leg", '1', now).unwrap();
        assert_eq!(u.by["device"], "8001vth");
        m.hangup("leg");
        assert_eq!(press(&mut m, "*1", now), Vec::<i16>::new());
    }

    #[test]
    fn pause_starts_a_new_code() {
        let now = Instant::now();
        let mut m = matcher([Some("1234"), None]);
        assert!(m.digit("leg", '1', now).is_none());
        assert!(m.digit("leg", '2', now).is_none());
        let late = now + GAP + Duration::from_secs(1);
        assert!(m.digit("leg", '3', late).is_none());
        assert!(m.digit("leg", '4', late).is_none());
        assert_eq!(press(&mut m, "1234", late + GAP * 2), [1]);
    }

    #[test]
    fn limits_guessing() {
        let now = Instant::now();
        let mut m = matcher([Some("9999"), None]);
        let wrong = "1".repeat(MAX_DIGITS - 2);
        assert!(press(&mut m, &wrong, now).is_empty());
        // The limit is reached during the code: ignored.
        assert!(press(&mut m, "9999", now + GAP * 2).is_empty());
        // A leg without codes is not followed.
        let mut m = matcher([None, None]);
        assert!(press(&mut m, "12", now).is_empty());
    }

    #[test]
    fn device_from_channel() {
        assert_eq!(device("sofia/internal/8001vth@192.0.2.1:5060"), "8001vth");
        assert_eq!(device("sofia/external/+4912345@sip.example"), "+4912345");
        assert_eq!(device(""), "");
    }
}
