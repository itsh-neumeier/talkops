//! Dahua VTO door stations over their HTTP API: open the door, take a
//! snapshot and follow the event stream (button pressed, door opened,
//! tamper alarm).
//!
//! Dahua's HTTP API specification is only available to partners, so the
//! requests follow what the device firmware answers and what established
//! open-source integrations use (see `docs/adr/0013-*.md`). Every request
//! authenticates with HTTP Digest (Basic as fallback); the challenge is
//! reused until the device rejects it.

mod events;

use std::time::Duration;

use bytes::Bytes;
use reqwest::{StatusCode, header};
use tokio::sync::Mutex;

pub use events::{Event, EventParser};

const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connection: {0}")]
    Http(#[from] reqwest::Error),
    #[error("wrong user name or password")]
    Unauthorized,
    #[error("authentication: {0}")]
    Auth(String),
    #[error("device answered {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("unexpected answer: {0}")]
    Unexpected(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Connection data of a door station.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

enum Challenge {
    Digest(digest_auth::WwwAuthenticateHeader),
    Basic,
}

/// HTTP client for one door station.
pub struct Vto {
    http: reqwest::Client,
    base: String,
    cfg: Config,
    challenge: Mutex<Option<Challenge>>,
}

/// reqwest is built without a bundled TLS provider (no aws-lc); this
/// registers ring as the process default once, before any client is built.
pub fn ensure_tls_provider() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

impl Vto {
    pub fn new(cfg: Config) -> Result<Self> {
        ensure_tls_provider();
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()?;
        let host = if cfg.host.contains(':') && !cfg.host.starts_with('[') {
            format!("[{}]", cfg.host)
        } else {
            cfg.host.clone()
        };
        Ok(Self {
            http,
            base: format!("http://{host}:{}", cfg.port),
            cfg,
            challenge: Mutex::new(None),
        })
    }

    fn authorization(&self, challenge: &mut Challenge, uri: &str) -> Result<String> {
        match challenge {
            Challenge::Digest(prompt) => {
                let ctx =
                    digest_auth::AuthContext::new(&self.cfg.username, &self.cfg.password, uri);
                Ok(prompt
                    .respond(&ctx)
                    .map_err(|e| Error::Auth(e.to_string()))?
                    .to_header_string())
            }
            Challenge::Basic => {
                use std::fmt::Write;
                let raw = format!("{}:{}", self.cfg.username, self.cfg.password);
                let mut out = String::from("Basic ");
                // Small base64 encoder; avoids a dependency for a fallback.
                const T: &[u8; 64] =
                    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
                for chunk in raw.as_bytes().chunks(3) {
                    let b = [
                        chunk[0],
                        *chunk.get(1).unwrap_or(&0),
                        *chunk.get(2).unwrap_or(&0),
                    ];
                    let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
                    for i in 0..4 {
                        if i <= chunk.len() {
                            let _ = out.write_char(T[(n >> (18 - 6 * i) & 63) as usize] as char);
                        } else {
                            out.push('=');
                        }
                    }
                }
                Ok(out)
            }
        }
    }

    /// GET with authentication; one retry with a fresh challenge.
    async fn get(&self, uri: &str, timeout: Option<Duration>) -> Result<reqwest::Response> {
        let url = format!("{}{uri}", self.base);
        let mut fresh = false;
        loop {
            let mut req = self.http.get(&url);
            if let Some(t) = timeout {
                req = req.timeout(t);
            }
            {
                let mut guard = self.challenge.lock().await;
                if let Some(ch) = guard.as_mut() {
                    req = req.header(header::AUTHORIZATION, self.authorization(ch, uri)?);
                }
            }
            let res = req.send().await?;
            if res.status() != StatusCode::UNAUTHORIZED {
                return Ok(res);
            }
            if fresh {
                return Err(Error::Unauthorized);
            }
            let header = res
                .headers()
                .get_all(header::WWW_AUTHENTICATE)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .find(|v| v.starts_with("Digest"))
                .map(str::to_owned);
            let basic = res
                .headers()
                .get_all(header::WWW_AUTHENTICATE)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .any(|v| v.starts_with("Basic"));
            let challenge = match header {
                Some(h) => Challenge::Digest(
                    digest_auth::parse(&h).map_err(|e| Error::Auth(e.to_string()))?,
                ),
                None if basic => Challenge::Basic,
                None => return Err(Error::Auth("no supported authentication scheme".into())),
            };
            *self.challenge.lock().await = Some(challenge);
            fresh = true;
        }
    }

    async fn text(&self, uri: &str) -> Result<String> {
        let res = self.get(uri, Some(TIMEOUT)).await?;
        let status = res.status();
        let body = res.text().await?;
        if !status.is_success() {
            return Err(Error::Status {
                status,
                body: body.trim().chars().take(200).collect(),
            });
        }
        Ok(body)
    }

    /// Device type, e.g. `VTO2202F-P` (connection test).
    pub async fn device_type(&self) -> Result<String> {
        let body = self
            .text("/cgi-bin/magicBox.cgi?action=getDeviceType")
            .await?;
        body.trim()
            .strip_prefix("type=")
            .map(|t| t.trim().to_owned())
            .ok_or_else(|| Error::Unexpected(body.trim().chars().take(100).collect()))
    }

    /// Opens a door (1 = the station's own relay, 2 = second lock).
    pub async fn open_door(&self, door: u8) -> Result<()> {
        let body = self
            .text(&format!(
                "/cgi-bin/accessControl.cgi?action=openDoor&channel={door}&UserID=101&Type=Remote"
            ))
            .await?;
        if body.trim() == "OK" {
            Ok(())
        } else {
            Err(Error::Unexpected(body.trim().chars().take(100).collect()))
        }
    }

    /// Current camera picture as JPEG.
    pub async fn snapshot(&self) -> Result<Bytes> {
        let res = self
            .get("/cgi-bin/snapshot.cgi?channel=1", Some(TIMEOUT))
            .await?;
        let status = res.status();
        if !status.is_success() {
            return Err(Error::Status {
                status,
                body: String::new(),
            });
        }
        let bytes = res.bytes().await?;
        strip_jpeg_trailer(bytes)
    }

    /// Opens the event stream. `heartbeat` (seconds, 1–60) makes the device
    /// send a keep-alive, so a dead connection is noticed.
    pub async fn events(&self, heartbeat: u8) -> Result<EventStream> {
        let res = self
            .get(
                &format!(
                    "/cgi-bin/eventManager.cgi?action=attach&codes=[All]&heartbeat={}",
                    heartbeat.clamp(1, 60)
                ),
                None,
            )
            .await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(Error::Status {
                status,
                body: body.trim().chars().take(200).collect(),
            });
        }
        let boundary = res
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(events::boundary_of)
            .unwrap_or_else(|| "myboundary".to_owned());
        Ok(EventStream {
            res,
            parser: EventParser::new(&boundary),
            idle: Duration::from_secs(u64::from(heartbeat.clamp(1, 60)) * 3),
        })
    }
}

/// Dahua appends a few bytes (`dhav…`) after the JPEG end marker, which
/// strict decoders reject.
fn strip_jpeg_trailer(bytes: Bytes) -> Result<Bytes> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return Err(Error::Unexpected("snapshot is not a JPEG".into()));
    }
    let end = bytes
        .windows(2)
        .rposition(|w| w == [0xFF, 0xD9])
        .map(|p| p + 2)
        .unwrap_or(bytes.len());
    Ok(bytes.slice(..end))
}

/// The open event stream of a door station.
pub struct EventStream {
    res: reqwest::Response,
    parser: EventParser,
    idle: Duration,
}

impl EventStream {
    /// Next batch of events (heartbeats are dropped). `Ok(None)` when the
    /// device closed the stream; an error if nothing (not even a heartbeat)
    /// arrived for three heartbeat intervals.
    pub async fn next(&mut self) -> Result<Option<Vec<Event>>> {
        loop {
            let chunk = tokio::time::timeout(self.idle, self.res.chunk())
                .await
                .map_err(|_| Error::Unexpected("event stream timed out".into()))??;
            let Some(chunk) = chunk else {
                return Ok(None);
            };
            let events = self.parser.push(&chunk);
            if !events.is_empty() {
                return Ok(Some(events));
            }
        }
    }
}

#[cfg(test)]
mod tests;
