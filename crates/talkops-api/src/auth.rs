//! Session authentication, CSRF protection, roles and login rate limiting.
//!
//! - The session token lives in an `HttpOnly; SameSite=Strict` cookie; the
//!   database stores only its SHA-256 digest.
//! - State-changing API requests must carry `X-Requested-With: TalkOps` (cannot
//!   be set cross-site without a CORS preflight) and, once logged in, the
//!   session's CSRF token in `X-CSRF-Token`.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, header};
use talkops_core::audit::Actor;
use talkops_core::tenant::TenantId;
use talkops_core::users::{self, Role, SessionUser};
use uuid::Uuid;

use crate::AppState;
use crate::error::ApiError;

pub const SESSION_COOKIE: &str = "talkops_session";
pub const CSRF_HEADER: &str = "x-csrf-token";
pub const REQUESTED_WITH: &str = "x-requested-with";

/// An authenticated user, extracted from the session cookie.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub tenant: TenantId,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub csrf_token: String,
    pub ip: Option<String>,
}

impl AuthUser {
    pub fn require(&self, role: Role) -> Result<(), ApiError> {
        if self.role >= role {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }

    pub fn actor(&self) -> Actor {
        Actor {
            tenant: self.tenant,
            user_id: Some(self.id),
            ip: self.ip.clone(),
        }
    }
}

pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// Client IP. `X-Forwarded-For` is only trusted from a loopback peer (the
/// optional Caddy reverse proxy runs on the same host).
pub fn client_ip(parts: &Parts) -> Option<String> {
    let peer = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    if peer.is_some_and(|ip| ip.is_loopback()) {
        if let Some(fwd) = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|v| v.trim().parse::<IpAddr>().ok())
        {
            return Some(fwd.to_string());
        }
    }
    peer.map(|ip| ip.to_string())
}

pub fn is_mutating(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Rejects cross-site state-changing requests (see module docs).
pub fn check_requested_with(parts: &Parts) -> Result<(), ApiError> {
    if is_mutating(&parts.method)
        && parts
            .headers
            .get(REQUESTED_WITH)
            .and_then(|v| v.to_str().ok())
            != Some("TalkOps")
    {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        check_requested_with(parts)?;
        let token = cookie_value(&parts.headers, SESSION_COOKIE).ok_or(ApiError::Unauthorized)?;
        let su: SessionUser = users::session_user(&state.db, token)
            .await?
            .ok_or(ApiError::Unauthorized)?;
        if is_mutating(&parts.method) {
            let sent = parts
                .headers
                .get(CSRF_HEADER)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            if !crate::util::constant_time_eq(sent.as_bytes(), su.csrf_token.as_bytes()) {
                return Err(ApiError::Forbidden);
            }
        }
        Ok(AuthUser {
            id: su.user_id,
            tenant: su.tenant_id,
            username: su.username,
            display_name: su.display_name,
            role: su.role,
            csrf_token: su.csrf_token,
            ip: client_ip(parts),
        })
    }
}

/// Builds the session cookie. `secure` adds the `Secure` attribute (HTTPS).
pub fn session_cookie(token: &str, secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        users::SESSION_TTL_HOURS * 3600,
        if secure { "; Secure" } else { "" }
    )
}

pub fn clear_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0")
}

/// Whether the client reached us via HTTPS (directly or through the proxy).
pub fn is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        == Some("https")
}

/// Fixed-window limiter for login attempts per client IP.
pub struct LoginLimiter {
    attempts: Mutex<HashMap<String, (u32, Instant)>>,
    max: u32,
    window: Duration,
}

impl Default for LoginLimiter {
    fn default() -> Self {
        Self {
            attempts: Mutex::default(),
            max: 10,
            window: Duration::from_secs(300),
        }
    }
}

impl LoginLimiter {
    /// Records an attempt; returns false if the client is over the limit.
    pub fn allow(&self, key: &str) -> bool {
        let mut map = self.attempts.lock().expect("limiter lock poisoned");
        let now = Instant::now();
        map.retain(|_, (_, start)| now.duration_since(*start) < self.window);
        let entry = map.entry(key.to_owned()).or_insert((0, now));
        entry.0 += 1;
        entry.0 <= self.max
    }

    pub fn reset(&self, key: &str) {
        self.attempts
            .lock()
            .expect("limiter lock poisoned")
            .remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cookies() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            "a=1; talkops_session=tok; b=2".parse().unwrap(),
        );
        assert_eq!(cookie_value(&h, SESSION_COOKIE), Some("tok"));
        assert_eq!(cookie_value(&h, "c"), None);
    }

    #[test]
    fn limiter_blocks_after_max() {
        let l = LoginLimiter {
            max: 2,
            ..Default::default()
        };
        assert!(l.allow("ip"));
        assert!(l.allow("ip"));
        assert!(!l.allow("ip"));
        assert!(l.allow("other"));
        l.reset("ip");
        assert!(l.allow("ip"));
    }

    #[test]
    fn cookie_attributes() {
        let c = session_cookie("t", true);
        assert!(c.contains("HttpOnly") && c.contains("SameSite=Strict") && c.ends_with("Secure"));
        assert!(!session_cookie("t", false).contains("Secure"));
    }
}
