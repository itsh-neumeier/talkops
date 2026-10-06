//! Parser for the `eventManager.cgi?action=attach` stream: a never-ending
//! `multipart/x-mixed-replace` body whose parts are text such as
//!
//! ```text
//! Code=AccessControl;action=Pulse;index=0;data={ "Method" : 4, ... }
//! ```
//!
//! or `Heartbeat`. Part headers and boundaries are skipped; a part may hold
//! several events, each starting with `Code=`.

use serde_json::Value;

/// One device event.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Event code, e.g. `Invite`, `AccessControl`, `DoorStatus`.
    pub code: String,
    /// `Start`, `Stop` or `Pulse`.
    pub action: String,
    pub index: i64,
    /// The `data` JSON object (`Null` if absent or unparsable).
    pub data: Value,
}

/// Boundary from a `multipart/x-mixed-replace; boundary=…` content type.
pub fn boundary_of(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|p| {
        let v = p.trim().strip_prefix("boundary=")?;
        let v = v.trim_matches('"').trim_start_matches("--");
        (!v.is_empty()).then(|| v.to_owned())
    })
}

/// Incremental parser; feed it the body chunks as they arrive.
pub struct EventParser {
    marker: Vec<u8>,
    buf: Vec<u8>,
}

/// Upper bound for a part, protects against a stream without boundaries.
const MAX_PART: usize = 256 * 1024;

impl EventParser {
    pub fn new(boundary: &str) -> Self {
        Self {
            marker: format!("--{boundary}").into_bytes(),
            buf: Vec::new(),
        }
    }

    /// Adds received bytes and returns the events of all completed parts.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Event> {
        self.buf.extend_from_slice(chunk);
        let mut events = Vec::new();
        // A part is complete once the next boundary has arrived.
        while let Some(start) = find(&self.buf, &self.marker) {
            let after = start + self.marker.len();
            let Some(next) = find(&self.buf[after..], &self.marker).map(|n| after + n) else {
                if start > 0 {
                    self.buf.drain(..start);
                }
                break;
            };
            let part = String::from_utf8_lossy(&self.buf[after..next]).into_owned();
            events.extend(parse_part(&part));
            self.buf.drain(..next);
        }
        if self.buf.len() > MAX_PART {
            self.buf.clear();
        }
        events
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Parses one part: skips its headers, then reads every `Code=` entry.
fn parse_part(part: &str) -> Vec<Event> {
    // Headers end with an empty line; parts without headers start directly.
    let body = match part.find("\r\n\r\n").or_else(|| part.find("\n\n")) {
        Some(i)
            if part[..i]
                .lines()
                .any(|l| l.contains(':') && !l.contains("Code=")) =>
        {
            &part[i..]
        }
        _ => part,
    };
    let mut events = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest.find("Code=") {
        let entry = &rest[pos..];
        let end = entry[5..]
            .find("Code=")
            .map(|e| e + 5)
            .unwrap_or(entry.len());
        if let Some(event) = parse_entry(&entry[..end]) {
            events.push(event);
        }
        rest = &entry[end..];
    }
    events
}

fn parse_entry(entry: &str) -> Option<Event> {
    let (head, data) = match entry.find(";data=") {
        Some(i) => (&entry[..i], Some(entry[i + 6..].trim())),
        None => (entry.trim(), None),
    };
    let mut code = None;
    let mut action = String::new();
    let mut index = 0;
    for field in head.split(';') {
        let (k, v) = field.split_once('=')?;
        match k.trim() {
            "Code" => code = Some(v.trim().to_owned()),
            "action" => action = v.trim().to_owned(),
            "index" => index = v.trim().parse().unwrap_or(0),
            _ => {}
        }
    }
    let data = data
        .and_then(|d| {
            // The JSON object may be followed by line breaks or garbage.
            let end = d.rfind('}')?;
            serde_json::from_str(&d[..=end]).ok()
        })
        .unwrap_or(Value::Null);
    Some(Event {
        code: code?,
        action,
        index,
        data,
    })
}
