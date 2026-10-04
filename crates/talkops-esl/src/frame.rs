//! ESL wire format: a block of `Name: value` header lines terminated by an
//! empty line, optionally followed by `Content-Length` bytes of body.

use percent_encoding::percent_decode_str;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

use crate::EslError;

/// Upper bound for a frame body; protects against a corrupt length header.
const MAX_BODY: usize = 16 * 1024 * 1024;

/// Ordered header list. Names are compared case-insensitively.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    fn parse_line(&mut self, line: &str, decode: bool) -> Result<(), EslError> {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| EslError::Protocol(format!("malformed header line `{line}`")))?;
        let value = value.trim_start();
        let value = if decode {
            percent_decode_str(value).decode_utf8_lossy().into_owned()
        } else {
            value.to_owned()
        };
        self.0.push((name.trim().to_owned(), value));
        Ok(())
    }
}

/// One message read from the socket.
#[derive(Debug, Clone)]
pub struct Frame {
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Frame {
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("Content-Type")
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Parses a `text/event-plain` body: URL-encoded headers, a blank line and
    /// an optional body of its own `Content-Length`.
    pub fn to_event(&self) -> Result<Event, EslError> {
        let text = String::from_utf8_lossy(&self.body);
        let (head, rest) = text.split_once("\n\n").unwrap_or((&text, ""));
        let mut headers = Headers::default();
        for line in head.lines().filter(|l| !l.is_empty()) {
            headers.parse_line(line, true)?;
        }
        let body = match headers.get("Content-Length") {
            Some(len) => {
                let len: usize = len
                    .parse()
                    .map_err(|_| EslError::Protocol(format!("bad event Content-Length `{len}`")))?;
                Some(rest.get(..len).unwrap_or(rest).to_owned())
            }
            None => None,
        };
        Ok(Event { headers, body })
    }
}

/// A FreeSWITCH event (e.g. `CHANNEL_ANSWER`).
#[derive(Debug, Clone)]
pub struct Event {
    pub headers: Headers,
    pub body: Option<String>,
}

impl Event {
    pub fn name(&self) -> Option<&str> {
        self.headers.get("Event-Name")
    }
}

/// Reads one frame. Returns `Ok(None)` on a clean EOF between frames.
pub async fn read_frame<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<Option<Frame>, EslError> {
    let mut headers = Headers::default();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return if headers.0.is_empty() {
                Ok(None)
            } else {
                Err(EslError::Closed)
            };
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            if headers.0.is_empty() {
                continue; // tolerate stray blank lines between frames
            }
            break;
        }
        headers.parse_line(trimmed, false)?;
    }

    let body = match headers.get("Content-Length") {
        Some(len) => {
            let len: usize = len
                .parse()
                .map_err(|_| EslError::Protocol(format!("bad Content-Length `{len}`")))?;
            if len > MAX_BODY {
                return Err(EslError::Protocol(format!(
                    "frame body too large ({len} bytes)"
                )));
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).await?;
            body
        }
        None => Vec::new(),
    };
    Ok(Some(Frame { headers, body }))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn parse(input: &str) -> Vec<Frame> {
        let mut reader = tokio::io::BufReader::new(input.as_bytes());
        let mut frames = Vec::new();
        while let Some(frame) = read_frame(&mut reader).await.unwrap() {
            frames.push(frame);
        }
        frames
    }

    #[tokio::test]
    async fn parses_consecutive_frames_with_bodies() {
        let frames = parse(
            "Content-Type: auth/request\n\n\
             Content-Type: api/response\nContent-Length: 5\n\n+OK\n\n\
             Content-Type: command/reply\nReply-Text: +OK\n\n",
        )
        .await;
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].content_type(), Some("auth/request"));
        assert_eq!(frames[1].body_text(), "+OK\n\n");
        assert_eq!(frames[2].headers.get("reply-text"), Some("+OK"));
    }

    #[tokio::test]
    async fn decodes_event_headers_and_body() {
        let event_text = "Event-Name: CUSTOM\nCaller-Caller-ID-Name: M%C3%BCller%20GmbH\nContent-Length: 4\n\nbody";
        let raw = format!(
            "Content-Type: text/event-plain\nContent-Length: {}\n\n{event_text}",
            event_text.len()
        );
        let frames = parse(&raw).await;
        let event = frames[0].to_event().unwrap();
        assert_eq!(event.name(), Some("CUSTOM"));
        assert_eq!(
            event.headers.get("Caller-Caller-ID-Name"),
            Some("Müller GmbH")
        );
        assert_eq!(event.body.as_deref(), Some("body"));
    }

    #[tokio::test]
    async fn rejects_truncated_frame() {
        let mut reader = tokio::io::BufReader::new("Content-Type: x\n".as_bytes());
        assert!(matches!(
            read_frame(&mut reader).await,
            Err(EslError::Closed)
        ));
    }
}
