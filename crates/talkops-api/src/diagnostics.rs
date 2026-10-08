//! Troubleshooting from the web UI: a time-limited capture of FreeSWITCH's
//! log (optionally with SIP trace) into a ring buffer, and restarting the
//! services.
//!
//! The capture uses its own event socket connection with `log <level>`, the
//! same stream `fs_cli` shows, so a busy log never delays call control on
//! the shared connection. It ends by itself after the chosen minutes and
//! switches the SIP trace off again.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use talkops_esl::EslClient;
use tokio::sync::{Notify, broadcast::error::RecvError};

/// Lines and bytes kept; the oldest are dropped first.
const MAX_LINES: usize = 50_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;
/// Longest capture.
pub const MAX_MINUTES: u32 = 60;
/// Log levels offered (FreeSWITCH names).
pub const LEVELS: &[&str] = &["debug", "info", "notice", "warning"];

/// Postgres channel the media worker listens on for `restart`.
pub const CONTROL_CHANNEL: &str = "talkops_control";

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsError {
    #[error("FreeSWITCH event socket not configured")]
    NotConfigured,
    #[error("FreeSWITCH: {0}")]
    Esl(#[from] talkops_esl::EslError),
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CaptureStatus {
    pub running: bool,
    pub level: Option<String>,
    pub sip_trace: bool,
    pub started_at: Option<DateTime<Utc>>,
    /// When the capture stops by itself.
    pub until: Option<DateTime<Utc>>,
    /// Lines in the buffer and the sequence number of the newest.
    pub lines: usize,
    pub last_seq: u64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct LogLine {
    pub seq: u64,
    pub text: String,
}

struct Session {
    level: String,
    sip_trace: bool,
    started_at: DateTime<Utc>,
    until: DateTime<Utc>,
    stop: Arc<Notify>,
}

#[derive(Default)]
struct Inner {
    session: Option<Session>,
    lines: VecDeque<LogLine>,
    bytes: usize,
    next_seq: u64,
}

impl Inner {
    fn push(&mut self, text: String) {
        self.next_seq += 1;
        self.bytes += text.len();
        self.lines.push_back(LogLine {
            seq: self.next_seq,
            text,
        });
        while self.lines.len() > MAX_LINES || self.bytes > MAX_BYTES {
            match self.lines.pop_front() {
                Some(old) => self.bytes -= old.text.len(),
                None => break,
            }
        }
    }
}

pub struct Diagnostics {
    esl: Option<(String, String)>,
    inner: Mutex<Inner>,
    /// When this server process started (uptime).
    pub started_at: DateTime<Utc>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            esl: None,
            inner: Mutex::default(),
            started_at: Utc::now(),
        }
    }
}

impl Diagnostics {
    /// `addr` and `password` of FreeSWITCH's event socket.
    pub fn new(addr: String, password: String) -> Self {
        Self {
            esl: Some((addr, password)),
            ..Self::default()
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("diagnostics lock poisoned")
    }

    pub fn status(&self) -> CaptureStatus {
        let inner = self.lock();
        let s = inner.session.as_ref();
        CaptureStatus {
            running: s.is_some(),
            level: s.map(|s| s.level.clone()),
            sip_trace: s.is_some_and(|s| s.sip_trace),
            started_at: s.map(|s| s.started_at),
            until: s.map(|s| s.until),
            lines: inner.lines.len(),
            last_seq: inner.next_seq,
        }
    }

    /// Lines newer than `after` (at most `limit`).
    pub fn lines_after(&self, after: u64, limit: usize) -> Vec<LogLine> {
        let inner = self.lock();
        inner
            .lines
            .iter()
            .filter(|l| l.seq > after)
            .take(limit)
            .cloned()
            .collect()
    }

    /// The whole buffer as text (download).
    pub fn dump(&self) -> String {
        let inner = self.lock();
        let mut out = String::with_capacity(inner.bytes + inner.lines.len());
        for l in &inner.lines {
            out.push_str(&l.text);
            if !l.text.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }

    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.lines.clear();
        inner.bytes = 0;
    }

    /// Starts a new capture (ending a running one) for `minutes`.
    pub async fn start(
        self: &Arc<Self>,
        level: &str,
        sip_trace: bool,
        minutes: u32,
    ) -> Result<CaptureStatus, DiagnosticsError> {
        let (addr, password) = self.esl.clone().ok_or(DiagnosticsError::NotConfigured)?;
        self.stop();
        let client = EslClient::connect(&addr, &password, Duration::from_secs(5)).await?;
        let mut logs = client.logs();
        client.log(level).await?;
        if sip_trace {
            client.api("sofia global siptrace on").await?;
        }
        let minutes = minutes.clamp(1, MAX_MINUTES);
        let started_at = Utc::now();
        let until = started_at + chrono::Duration::minutes(minutes.into());
        let stop = Arc::new(Notify::new());
        {
            let mut inner = self.lock();
            inner.push(format!(
                "--- TalkOps: capture started {started_at} (level {level}, SIP trace {}) ---\n",
                if sip_trace { "on" } else { "off" }
            ));
            inner.session = Some(Session {
                level: level.to_owned(),
                sip_trace,
                started_at,
                until,
                stop: stop.clone(),
            });
        }
        let this = self.clone();
        tokio::spawn(async move {
            let deadline = tokio::time::sleep(Duration::from_secs(u64::from(minutes) * 60));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    line = logs.recv() => match line {
                        Ok(text) => this.lock().push(text),
                        Err(RecvError::Lagged(n)) => {
                            this.lock().push(format!("--- TalkOps: {n} lines dropped ---\n"));
                        }
                        Err(RecvError::Closed) => {
                            this.lock().push("--- TalkOps: FreeSWITCH connection lost ---\n".into());
                            break;
                        }
                    },
                    () = &mut deadline => break,
                    () = stop.notified() => break,
                }
            }
            if sip_trace {
                let _ = client.api("sofia global siptrace off").await;
            }
            let _ = client.send("nolog").await;
            let mut inner = this.lock();
            // A newer capture may already have replaced this one.
            if inner
                .session
                .as_ref()
                .is_some_and(|s| Arc::ptr_eq(&s.stop, &stop))
            {
                inner.session = None;
            }
            inner.push(format!("--- TalkOps: capture ended {} ---\n", Utc::now()));
        });
        Ok(self.status())
    }

    /// Ends the running capture, if any.
    pub fn stop(&self) {
        if let Some(s) = self.lock().session.take() {
            s.stop.notify_one();
        }
    }
}

/// A call leg from `show channels as json` (the fields useful for support).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Channel {
    pub uuid: String,
    pub created: String,
    pub name: String,
    pub state: String,
    pub callstate: String,
    pub cid_name: String,
    pub cid_num: String,
    pub dest: String,
    pub application: String,
    pub application_data: String,
    pub read_codec: String,
    pub write_codec: String,
    pub secure: String,
}

pub fn parse_channels(json: &str) -> Vec<Channel> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let rows = value
        .get("rows")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let f = |r: &serde_json::Value, k: &str| {
        r.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    rows.iter()
        .map(|r| Channel {
            uuid: f(r, "uuid"),
            created: f(r, "created"),
            name: f(r, "name"),
            state: f(r, "state"),
            callstate: f(r, "callstate"),
            cid_name: f(r, "cid_name"),
            cid_num: f(r, "cid_num"),
            dest: f(r, "dest"),
            application: f(r, "application"),
            application_data: f(r, "application_data"),
            read_codec: f(r, "read_codec"),
            write_codec: f(r, "write_codec"),
            secure: f(r, "secure"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_drops_oldest() {
        let mut inner = Inner::default();
        for i in 0..(MAX_LINES + 10) {
            inner.push(format!("line {i}\n"));
        }
        assert_eq!(inner.lines.len(), MAX_LINES);
        assert_eq!(inner.lines.front().unwrap().text, "line 10\n");
        assert_eq!(inner.next_seq, (MAX_LINES + 10) as u64);
    }

    #[test]
    fn reads_lines_and_dump() {
        let d = Diagnostics::default();
        d.lock().push("a\n".into());
        d.lock().push("b".into());
        assert_eq!(d.lines_after(1, 10).len(), 1);
        assert_eq!(d.dump(), "a\nb\n");
        assert!(!d.status().running);
        d.clear();
        assert_eq!(d.status().lines, 0);
        assert_eq!(d.status().last_seq, 2);
    }

    #[test]
    fn parses_channels() {
        let json = r#"{"row_count":1,"rows":[{"uuid":"u1","name":"sofia/internal/20-1@x","state":"CS_EXECUTE","read_codec":"opus","cid_num":"20"}]}"#;
        let c = parse_channels(json);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].read_codec, "opus");
        assert!(parse_channels("0 total.").is_empty());
    }
}
