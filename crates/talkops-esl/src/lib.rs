//! Minimal async client for the FreeSWITCH Event Socket Library (ESL) protocol
//! in inbound mode: TalkOps connects to FreeSWITCH, authenticates, sends
//! commands and receives events.
//!
//! A background task reads frames from the socket. Command replies are matched
//! to callers in FIFO order (FreeSWITCH answers commands strictly in order);
//! events are fanned out through a broadcast channel.

mod frame;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::{broadcast, oneshot};

pub use frame::{Event, Frame, Headers};

#[derive(Debug, thiserror::Error)]
pub enum EslError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("authentication rejected: {0}")]
    AuthFailed(String),
    #[error("command failed: {0}")]
    CommandFailed(String),
    #[error("connection closed")]
    Closed,
    #[error("timed out")]
    Timeout,
}

type Pending = Arc<Mutex<VecDeque<oneshot::Sender<Frame>>>>;

/// A connected and authenticated ESL session.
pub struct EslClient {
    writer: tokio::sync::Mutex<OwnedWriteHalf>,
    pending: Pending,
    events: broadcast::Sender<Event>,
    timeout: Duration,
}

impl EslClient {
    /// Connects to `addr` (e.g. `127.0.0.1:8021`) and authenticates.
    pub async fn connect(addr: &str, password: &str, timeout: Duration) -> Result<Self, EslError> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| EslError::Timeout)??;
        stream.set_nodelay(true)?;
        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        let greeting = tokio::time::timeout(timeout, frame::read_frame(&mut reader))
            .await
            .map_err(|_| EslError::Timeout)??
            .ok_or(EslError::Closed)?;
        if greeting.content_type() != Some("auth/request") {
            return Err(EslError::Protocol(format!(
                "expected auth/request, got {:?}",
                greeting.content_type()
            )));
        }

        write_half
            .write_all(format!("auth {password}\n\n").as_bytes())
            .await?;
        let reply = tokio::time::timeout(timeout, frame::read_frame(&mut reader))
            .await
            .map_err(|_| EslError::Timeout)??
            .ok_or(EslError::Closed)?;
        let text = reply
            .headers
            .get("Reply-Text")
            .unwrap_or_default()
            .to_owned();
        if !text.starts_with("+OK") {
            return Err(EslError::AuthFailed(text));
        }

        let pending: Pending = Arc::default();
        let (events, _) = broadcast::channel(1024);
        tokio::spawn(read_loop(reader, pending.clone(), events.clone()));

        Ok(Self {
            writer: tokio::sync::Mutex::new(write_half),
            pending,
            events,
            timeout,
        })
    }

    /// Sends a raw command (without the trailing blank line) and returns the reply frame.
    pub async fn send(&self, command: &str) -> Result<Frame, EslError> {
        if command.contains('\n') {
            return Err(EslError::Protocol("command must be a single line".into()));
        }
        let (tx, rx) = oneshot::channel();
        {
            // Register and write under the same lock so replies stay in order.
            let mut writer = self.writer.lock().await;
            self.pending
                .lock()
                .expect("pending lock poisoned")
                .push_back(tx);
            writer
                .write_all(format!("{command}\n\n").as_bytes())
                .await?;
        }
        tokio::time::timeout(self.timeout, rx)
            .await
            .map_err(|_| EslError::Timeout)?
            .map_err(|_| EslError::Closed)
    }

    /// Runs a synchronous FreeSWITCH API command (`api <cmd>`) and returns its output.
    pub async fn api(&self, command: &str) -> Result<String, EslError> {
        let reply = self.send(&format!("api {command}")).await?;
        let body = reply.body_text();
        if body.starts_with("-ERR") {
            return Err(EslError::CommandFailed(body.trim_end().to_owned()));
        }
        Ok(body)
    }

    /// Subscribes to events in plain format, e.g. `["CHANNEL_CREATE", "CUSTOM sofia::register"]`.
    pub async fn subscribe(&self, events: &[&str]) -> Result<(), EslError> {
        let reply = self
            .send(&format!("event plain {}", events.join(" ")))
            .await?;
        let text = reply.headers.get("Reply-Text").unwrap_or_default();
        if text.starts_with("+OK") {
            Ok(())
        } else {
            Err(EslError::CommandFailed(text.to_owned()))
        }
    }

    /// Receiver for events subscribed via [`EslClient::subscribe`].
    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    /// True while the background reader is alive.
    pub fn is_connected(&self) -> bool {
        // The reader holds a clone of the pending queue; once it exits only we hold it.
        Arc::strong_count(&self.pending) > 1
    }
}

async fn read_loop<R>(mut reader: R, pending: Pending, events: broadcast::Sender<Event>)
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    loop {
        let frame = match frame::read_frame(&mut reader).await {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(err) => {
                tracing::warn!(error = %err, "ESL read failed");
                break;
            }
        };
        match frame.content_type() {
            Some("command/reply") | Some("api/response") => {
                let waiter = pending.lock().expect("pending lock poisoned").pop_front();
                match waiter {
                    Some(tx) => {
                        let _ = tx.send(frame);
                    }
                    None => tracing::warn!("ESL reply without pending command"),
                }
            }
            Some("text/event-plain") => match frame.to_event() {
                Ok(event) => {
                    let _ = events.send(event);
                }
                Err(err) => tracing::warn!(error = %err, "malformed ESL event"),
            },
            Some("text/disconnect-notice") => break,
            other => tracing::debug!(content_type = ?other, "ignoring ESL frame"),
        }
    }
    // Dropping the queued senders wakes waiting callers with `Closed`.
    pending.lock().expect("pending lock poisoned").clear();
    tracing::info!("ESL connection closed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    use tokio::net::TcpListener;

    /// Fake FreeSWITCH that accepts `secret`, answers `api status` and emits one event.
    async fn fake_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (r, mut w) = stream.into_split();
            let mut r = BufReader::new(r);
            w.write_all(b"Content-Type: auth/request\n\n")
                .await
                .unwrap();
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                let mut blank = [0u8; 1];
                r.read_exact(&mut blank).await.unwrap();
                let cmd = line.trim_end();
                let out = if cmd == "auth secret" {
                    "Content-Type: command/reply\nReply-Text: +OK accepted\n\n".to_string()
                } else if cmd.starts_with("auth ") {
                    "Content-Type: command/reply\nReply-Text: -ERR invalid\n\n".to_string()
                } else if cmd == "api status" {
                    let body = "UP 0 years, 0 days\n";
                    format!(
                        "Content-Type: api/response\nContent-Length: {}\n\n{body}",
                        body.len()
                    )
                } else if cmd.starts_with("event plain") {
                    let ev = "Event-Name: HEARTBEAT\nUp-Time: 0%20years\n\n";
                    format!(
                        "Content-Type: command/reply\nReply-Text: +OK event listener enabled plain\n\n\
                         Content-Type: text/event-plain\nContent-Length: {}\n\n{ev}",
                        ev.len()
                    )
                } else {
                    let body = "-ERR unknown command\n";
                    format!(
                        "Content-Type: api/response\nContent-Length: {}\n\n{body}",
                        body.len()
                    )
                };
                w.write_all(out.as_bytes()).await.unwrap();
            }
        });
        addr
    }

    #[tokio::test]
    async fn auth_api_and_events() {
        let addr = fake_server().await;
        let client = EslClient::connect(&addr, "secret", Duration::from_secs(2))
            .await
            .unwrap();
        assert!(client.is_connected());
        assert_eq!(client.api("status").await.unwrap(), "UP 0 years, 0 days\n");
        assert!(matches!(
            client.api("bogus").await,
            Err(EslError::CommandFailed(_))
        ));

        let mut events = client.events();
        client.subscribe(&["HEARTBEAT"]).await.unwrap();
        let event = events.recv().await.unwrap();
        assert_eq!(event.name(), Some("HEARTBEAT"));
        assert_eq!(event.headers.get("Up-Time"), Some("0 years"));
    }

    #[tokio::test]
    async fn wrong_password_is_rejected() {
        let addr = fake_server().await;
        let err = EslClient::connect(&addr, "wrong", Duration::from_secs(2))
            .await
            .err()
            .unwrap();
        assert!(matches!(err, EslError::AuthFailed(_)), "{err:?}");
    }
}
