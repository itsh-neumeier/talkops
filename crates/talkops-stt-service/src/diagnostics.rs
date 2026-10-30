//! Bounded, time-limited technical traces. Audio is never captured.
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const MAX_EVENTS: usize = 5000;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const RETENTION_MS: u64 = 3600 * 1000;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

struct Capture {
    until: u64,
    include_text: bool,
}

struct Event {
    scope: String,
    value: Value,
    bytes: usize,
    at: u64,
}

#[derive(Default)]
struct Inner {
    captures: HashMap<String, Capture>,
    events: VecDeque<Event>,
    bytes: usize,
    dropped: u64,
}

#[derive(Default)]
pub struct Diagnostics(Mutex<Inner>);

pub fn valid_scope(scope: &str) -> bool {
    !scope.is_empty()
        && scope.len() <= 64
        && scope
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

impl Inner {
    fn prune(&mut self, now: u64) {
        self.captures
            .retain(|_, capture| capture.until.saturating_add(RETENTION_MS) > now);
        while self
            .events
            .front()
            .is_some_and(|e| e.at.saturating_add(RETENTION_MS) <= now)
        {
            if let Some(event) = self.events.pop_front() {
                self.bytes -= event.bytes;
            }
        }
    }
}

impl Diagnostics {
    pub fn start(&self, scope: &str, minutes: u32, include_text: bool) -> Result<(), &'static str> {
        if !valid_scope(scope) || !(1..=30).contains(&minutes) {
            return Err("scope and 1–30 minutes required");
        }
        let mut inner = self.0.lock().expect("diagnostic lock");
        inner.prune(now_ms());
        if !inner.captures.contains_key(scope) && inner.captures.len() >= 16 {
            return Err("diagnostic capture capacity reached");
        }
        inner.captures.insert(
            scope.into(),
            Capture {
                until: now_ms() + u64::from(minutes) * 60_000,
                include_text,
            },
        );
        Ok(())
    }

    pub fn stop(&self, scope: &str) {
        if let Some(capture) = self
            .0
            .lock()
            .expect("diagnostic lock")
            .captures
            .get_mut(scope)
        {
            capture.until = now_ms();
        }
    }

    /// The caller constructs technical fields explicitly, never from a raw
    /// WebSocket request. Text requires explicit capture consent.
    pub fn record(
        &self,
        scope: &str,
        session: &str,
        event: &str,
        details: Value,
        text: Option<&str>,
    ) {
        let now = now_ms();
        let mut inner = self.0.lock().expect("diagnostic lock");
        inner.prune(now);
        let Some(capture) = inner.captures.get(scope).filter(|c| c.until > now) else {
            return;
        };
        let mut value =
            json!({"at_unix_ms": now, "session": session, "event": event, "details": details});
        if capture.include_text
            && let Some(text) = text
        {
            value["text"] = json!(text.chars().take(2000).collect::<String>());
        }
        let bytes = value.to_string().len();
        if bytes > MAX_BYTES {
            return;
        }
        inner.bytes += bytes;
        inner.events.push_back(Event {
            scope: scope.into(),
            value,
            bytes,
            at: now,
        });
        while inner.events.len() > MAX_EVENTS || inner.bytes > MAX_BYTES {
            if let Some(old) = inner.events.pop_front() {
                inner.bytes -= old.bytes;
                inner.dropped += 1;
            }
        }
    }

    pub fn snapshot(&self, scope: &str) -> Value {
        let mut inner = self.0.lock().expect("diagnostic lock");
        inner.prune(now_ms());
        let capture = inner.captures.get(scope);
        json!({"running": capture.is_some_and(|c| c.until > now_ms()),
            "until_unix_ms": capture.map(|c| c.until), "include_text": capture.is_some_and(|c| c.include_text),
            "retention_secs": 3600, "event_limit": MAX_EVENTS, "byte_limit": MAX_BYTES,
            "dropped_events_global": inner.dropped,
            "events": inner.events.iter().filter(|e| e.scope == scope).map(|e| &e.value).collect::<Vec<_>>()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_deadline_and_retention_are_enforced() {
        let d = Diagnostics::default();
        d.start("tenant", 1, false).unwrap();
        d.record("tenant", "s", "initial", json!({}), None);
        {
            let mut inner = d.0.lock().unwrap();
            inner.captures.get_mut("tenant").unwrap().until = now_ms() - 1;
        }
        d.record("tenant", "s", "after_deadline", json!({}), None);
        assert_eq!(d.snapshot("tenant")["events"].as_array().unwrap().len(), 1);
        assert_eq!(d.snapshot("tenant")["running"], false);
        d.0.lock().unwrap().events.front_mut().unwrap().at = now_ms() - RETENTION_MS - 1;
        assert!(
            d.snapshot("tenant")["events"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn traces_are_scoped_bounded_and_hide_text_by_default() {
        let d = Diagnostics::default();
        d.start("tenant-a", 1, false).unwrap();
        d.start("tenant-b", 1, true).unwrap();
        d.record(
            "tenant-a",
            "a",
            "completed",
            json!({"chars": 6}),
            Some("secret"),
        );
        d.record("tenant-b", "b", "completed", json!({}), Some("consented"));
        assert!(d.snapshot("tenant-a")["events"][0].get("text").is_none());
        assert_eq!(d.snapshot("tenant-b")["events"][0]["text"], "consented");
        assert_eq!(
            d.snapshot("tenant-a")["events"].as_array().unwrap().len(),
            1
        );
        for _ in 0..MAX_EVENTS + 2 {
            d.record("tenant-a", "a", "append", json!({}), None);
        }
        assert!(d.snapshot("tenant-a")["events"].as_array().unwrap().len() <= MAX_EVENTS);
        d.stop("tenant-a");
        let before = d.snapshot("tenant-a")["events"].as_array().unwrap().len();
        d.record("tenant-a", "a", "ignored", json!({}), None);
        assert_eq!(
            d.snapshot("tenant-a")["events"].as_array().unwrap().len(),
            before
        );
    }
}
