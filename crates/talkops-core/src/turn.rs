//! TURN relay for the browser softphone (coturn with `use-auth-secret`).
//!
//! Browsers get short-lived credentials following the "TURN REST API"
//! scheme: the username is `<expiry unix time>:<user id>`, the password the
//! Base64 HMAC-SHA1 of that username with the secret shared with coturn.
//! Nothing is stored; coturn checks the signature and the expiry.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use sha1::Sha1;

/// How long issued credentials stay valid (a browser tab may stay open).
pub const CREDENTIAL_TTL_SECS: u64 = 24 * 60 * 60;

/// An ICE server entry as `RTCPeerConnection` expects it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

/// Username and password for `user`, valid until `now + ttl`.
pub fn credentials(secret: &str, user: &str, now_unix: u64, ttl_secs: u64) -> (String, String) {
    let username = format!("{}:{user}", now_unix + ttl_secs);
    let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(username.as_bytes());
    let credential = STANDARD.encode(mac.finalize().into_bytes());
    (username, credential)
}

/// The ICE server entry for `user` (`None` without URLs or secret).
pub fn ice_server(urls: &[String], secret: &str, user: &str, now_unix: u64) -> Option<IceServer> {
    if urls.is_empty() || secret.is_empty() {
        return None;
    }
    let (username, credential) = credentials(secret, user, now_unix, CREDENTIAL_TTL_SECS);
    Some(IceServer {
        urls: urls.to_vec(),
        username,
        credential,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_api_credentials() {
        // Reference: printf '1700086400:alice' | openssl dgst -sha1 -hmac s3cret -binary | base64
        let (user, pass) = credentials("s3cret", "alice", 1_700_000_000, 86_400);
        assert_eq!(user, "1700086400:alice");
        assert_eq!(pass, "HJX0XIkrCiQCuU88Z0oeXj+QW60=");
        assert!(ice_server(&[], "s3cret", "alice", 0).is_none());
        assert!(ice_server(&["turn:x:3478".into()], "", "alice", 0).is_none());
        let s = ice_server(&["turn:x:3478".into()], "s3cret", "alice", 1_700_000_000).unwrap();
        assert_eq!(s.username, "1700086400:alice");
    }
}
