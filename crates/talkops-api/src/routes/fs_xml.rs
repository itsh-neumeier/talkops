//! `mod_xml_curl` endpoint. FreeSWITCH POSTs every directory, dialplan and
//! configuration lookup here and TalkOps answers from the database.
//!
//! Phase 0 answers "not found" for everything, which makes FreeSWITCH fall
//! back to its minimal static bootstrap configuration.

use std::collections::HashMap;

use axum::Form;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::AppState;

/// Basic-auth user name FreeSWITCH sends (`gateway-credentials` in xml_curl.conf).
pub const XMLCURL_USER: &str = "talkops";

const NOT_FOUND: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<document type="freeswitch/xml">
  <section name="result">
    <result status="not found"/>
  </section>
</document>
"#;

pub async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(params): Form<HashMap<String, String>>,
) -> Response {
    if !authorized(&headers, &state.xmlcurl_password) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"talkops-fs\"")],
        )
            .into_response();
    }

    let section = params
        .get("section")
        .map(String::as_str)
        .unwrap_or_default();
    tracing::debug!(
        section,
        key_name = params.get("key_name").map(String::as_str),
        key_value = params.get("key_value").map(String::as_str),
        "xml_curl lookup"
    );

    xml(NOT_FOUND)
}

fn xml(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "text/xml; charset=utf-8")], body).into_response()
}

fn authorized(headers: &HeaderMap, password: &str) -> bool {
    let Some(encoded) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
    else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded.trim()) else {
        return false;
    };
    let expected = format!("{XMLCURL_USER}:{password}");
    constant_time_eq(&decoded, expected.as_bytes())
}

/// Compares without short-circuiting on the first differing byte.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
