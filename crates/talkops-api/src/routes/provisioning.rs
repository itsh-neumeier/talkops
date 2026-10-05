//! Phone auto-provisioning (`/provisioning/...`), fetched by the phones
//! themselves: Yealink configuration files, XML phonebooks, firmware images
//! and action-URL events.
//!
//! Phones authenticate with HTTP basic auth (credentials from DHCP option 66
//! or the phone's auto-provisioning settings). Action URLs cannot carry basic
//! auth on all firmware versions, so `events` also accepts `key=<password>`.

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use talkops_core::phones::{self, Phone};
use talkops_core::tenant::TenantId;
use talkops_core::{extensions, settings};
use talkops_provisioning::phonebook::{self, Entry};
use talkops_provisioning::yealink::{self, Account, LineKey, Locale, PhoneSetup, Provisioning};
use talkops_provisioning::{catalog::firmware_from_user_agent, normalize_mac};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{client_ip, is_https};
use crate::error::{ApiError, ApiResult};
use crate::util::constant_time_eq;

/// Feature code phones dial for voicemail (Phase 3).
pub const VOICEMAIL_CODE: &str = "*97";

/// Provisioning base URL as seen by the client (`http://host:port/provisioning`).
pub fn base_url(headers: &HeaderMap) -> String {
    let scheme = if is_https(headers) { "https" } else { "http" };
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .filter(|h| {
            h.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
        })
        .unwrap_or("localhost");
    format!("{scheme}://{host}/provisioning")
}

/// Host part of the `Host` header (without port), used as SIP server address.
fn sip_host(headers: &HeaderMap) -> String {
    let url = base_url(headers);
    let host = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .split('/')
        .next()
        .unwrap_or_default();
    if let Some(v6) = host.strip_prefix('[') {
        return format!("[{}]", v6.split(']').next().unwrap_or_default());
    }
    host.split(':').next().unwrap_or_default().to_owned()
}

async fn provisioning(state: &AppState, headers: &HeaderMap) -> ApiResult<(Provisioning, String)> {
    let secrets =
        phones::provisioning_secrets(&state.db, TenantId::DEFAULT, &state.secrets, false).await?;
    Ok((
        Provisioning {
            base_url: base_url(headers),
            username: secrets.username,
            password: secrets.password,
        },
        secrets.phone_admin_password,
    ))
}

fn basic_auth(headers: &HeaderMap) -> Option<Vec<u8>> {
    let encoded = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Basic ")?;
    STANDARD.decode(encoded.trim()).ok()
}

fn text(content_type: &'static str, body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

/// Renders the per-phone configuration file.
pub async fn render_phone_config(
    state: &AppState,
    tenant: TenantId,
    phone: &Phone,
    headers: &HeaderMap,
) -> ApiResult<String> {
    let model = state
        .phone_catalog
        .get(&phone.model)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown phone model {}", phone.model)))?;
    let (prov, _) = provisioning(state, headers).await?;
    let mut accounts = Vec::new();
    for a in phones::accounts(&state.db, phone.id).await? {
        let password = state
            .secrets
            .decrypt(&a.sip_password_enc)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        accounts.push(Account {
            index: a.account_index.max(1) as u16,
            label: a.extension_number.clone(),
            display_name: a.display_name,
            username: a.sip_username,
            password,
        });
    }
    let keys: Vec<LineKey> = serde_json::from_value(phone.line_keys.clone()).unwrap_or_default();
    let firmware_url = phones::active_firmware(&state.db, tenant, &phone.model)
        .await?
        .map(|fw| {
            format!(
                "{}/firmware/{}/{}",
                prov.authenticated_base(),
                fw.id,
                fw.filename
            )
        });
    let setup = PhoneSetup {
        name: phone.name.clone(),
        mac: phone.mac.clone(),
        model,
        accounts,
        keys,
        sip_host: sip_host(headers),
        sip_port: state.profile.internal_port,
        firmware_url,
        voicemail_code: VOICEMAIL_CODE.to_owned(),
    };
    yealink::render_phone(&setup).map_err(|e| ApiError::Internal(e.to_string()))
}

/// `GET /provisioning/{*path}`
pub async fn serve(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    req: Request,
) -> Response {
    let (parts, _) = req.into_parts();
    let headers = &parts.headers;
    let ip = client_ip(&parts);
    let limiter_key = format!("prov:{}", ip.as_deref().unwrap_or("unknown"));
    if state.limiter.blocked(&limiter_key) {
        return ApiError::TooManyRequests.into_response();
    }

    let (prov, admin_password) = match provisioning(&state, headers).await {
        Ok(p) => p,
        Err(err) => return err.into_response(),
    };
    let expected = format!("{}:{}", prov.username, prov.password);
    let authorized = match basic_auth(headers) {
        Some(given) => constant_time_eq(&given, expected.as_bytes()),
        None => false,
    } || (path == "events"
        && query
            .get("key")
            .is_some_and(|k| constant_time_eq(k.as_bytes(), prov.password.as_bytes())));
    if !authorized {
        // Phones first ask without credentials; only wrong ones count.
        let wrong = headers.contains_key(header::AUTHORIZATION) || query.contains_key("key");
        if wrong && !state.limiter.allow(&limiter_key) {
            return ApiError::TooManyRequests.into_response();
        }
        tracing::debug!(path, ip = ?ip, "provisioning request without valid credentials");
        return (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                "Basic realm=\"talkops-provisioning\"",
            )],
        )
            .into_response();
    }

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let result = match path.as_str() {
        p if is_common_cfg(p) => common_cfg(&state, &prov, &admin_password).await,
        "phonebook/internal.xml" => internal_phonebook(&state, query.get("search")).await,
        "phonebook/contacts.xml" => contacts_phonebook(&state, query.get("search")).await,
        "events" => events(&state, &query).await,
        p if p.starts_with("firmware/") => return firmware(&state, p).await,
        p => match p.strip_suffix(".cfg").and_then(normalize_mac) {
            Some(mac) if p.len() == 16 => {
                phone_cfg(&state, &mac, headers, ip.as_deref(), user_agent).await
            }
            _ => return not_found(),
        },
    };
    result.unwrap_or_else(|err| match err {
        ApiError::NotFound => not_found(),
        other => other.into_response(),
    })
}

/// `y0000000000XX.cfg` (some models use three digits): the model-specific
/// common file. All models get the same content.
fn is_common_cfg(path: &str) -> bool {
    path.strip_prefix("y0000000000")
        .and_then(|rest| rest.strip_suffix(".cfg"))
        .is_some_and(|code| {
            (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_digit())
        })
}

async fn common_cfg(state: &AppState, prov: &Provisioning, admin: &str) -> ApiResult<Response> {
    let s = settings::get(&state.db, TenantId::DEFAULT).await?;
    let locale = Locale::new(&s.timezone, &s.default_language);
    let body = yealink::render_common(prov, admin, &locale, &s.default_language)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(text("text/plain; charset=utf-8", body))
}

async fn phone_cfg(
    state: &AppState,
    mac: &str,
    headers: &HeaderMap,
    ip: Option<&str>,
    user_agent: &str,
) -> ApiResult<Response> {
    let Some((tenant, phone)) = phones::find_by_mac(&state.db, mac).await? else {
        tracing::info!(mac, "provisioning request from unknown phone");
        return Err(ApiError::NotFound);
    };
    let firmware = firmware_from_user_agent(user_agent);
    phones::record_seen(&state.db, phone.id, ip, firmware.as_deref()).await?;
    let body = render_phone_config(state, tenant, &phone, headers).await?;
    Ok(text("text/plain; charset=utf-8", body))
}

async fn internal_phonebook(state: &AppState, search: Option<&String>) -> ApiResult<Response> {
    let entries = extensions::list(&state.db, TenantId::DEFAULT)
        .await?
        .into_iter()
        .filter(|e| e.enabled)
        .map(|e| Entry {
            name: if e.display_name.is_empty() {
                e.number.clone()
            } else {
                e.display_name.clone()
            },
            numbers: vec![("Extension".to_owned(), e.number)],
        })
        .collect();
    let entries = phonebook::search(entries, search.map(String::as_str));
    Ok(text(
        "text/xml; charset=utf-8",
        phonebook::render("TalkOps", &entries),
    ))
}

async fn contacts_phonebook(state: &AppState, search: Option<&String>) -> ApiResult<Response> {
    let s = settings::get(&state.db, TenantId::DEFAULT).await?;
    let plan = s.dial_plan();
    let (work, mobile, other) = if s.default_language == "de" {
        ("Geschäftlich", "Mobil", "Sonstige")
    } else {
        ("Work", "Mobile", "Other")
    };
    let display = |n: &str| {
        if n.starts_with('+') {
            plan.for_display(n)
        } else {
            n.to_owned()
        }
    };
    let entries = phones::list_contacts(&state.db, TenantId::DEFAULT)
        .await?
        .into_iter()
        .map(|c| {
            let numbers = [
                (work, &c.phone_work),
                (mobile, &c.phone_mobile),
                (other, &c.phone_other),
            ]
            .into_iter()
            .filter(|(_, n)| !n.is_empty())
            .map(|(label, n)| (label.to_owned(), display(n)))
            .collect();
            let name = if c.company.is_empty() || c.name.is_empty() {
                format!("{}{}", c.name, c.company)
            } else {
                format!("{} ({})", c.name, c.company)
            };
            Entry { name, numbers }
        })
        .collect();
    let entries = phonebook::search(entries, search.map(String::as_str));
    Ok(text(
        "text/xml; charset=utf-8",
        phonebook::render("TalkOps", &entries),
    ))
}

/// Action URL events: DND toggled on the phone, setup completed.
async fn events(state: &AppState, query: &HashMap<String, String>) -> ApiResult<Response> {
    let Some(mac) = query.get("mac").and_then(|m| normalize_mac(m)) else {
        return Err(ApiError::BadRequest("mac required".into()));
    };
    let Some((tenant, phone)) = phones::find_by_mac(&state.db, &mac).await? else {
        return Err(ApiError::NotFound);
    };
    match query.get("event").map(String::as_str) {
        Some(ev @ ("dnd_on" | "dnd_off")) => {
            // The DND key acts on the phone's first account.
            if let Some(account) = phones::accounts(&state.db, phone.id).await?.first() {
                extensions::set_dnd(&state.db, tenant, account.extension_id, ev == "dnd_on")
                    .await?;
            }
        }
        Some("setup_completed") => {
            let ip = query
                .get("ip")
                .map(String::as_str)
                .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok());
            let fw = query
                .get("firmware")
                .map(String::as_str)
                .filter(|f| f.len() <= 32 && f.chars().all(|c| c.is_ascii_digit() || c == '.'));
            phones::record_seen(&state.db, phone.id, ip, fw).await?;
        }
        _ => return Err(ApiError::BadRequest("unknown event".into())),
    }
    Ok(StatusCode::OK.into_response())
}

/// `firmware/<id>/<filename>`: an uploaded firmware image.
async fn firmware(state: &AppState, path: &str) -> Response {
    let mut parts = path.splitn(3, '/').skip(1);
    let (Some(id), Some(name)) = (
        parts.next().and_then(|id| id.parse::<Uuid>().ok()),
        parts.next(),
    ) else {
        return not_found();
    };
    let Ok(fw) = phones::get_firmware(&state.db, TenantId::DEFAULT, id).await else {
        return not_found();
    };
    if fw.filename != name {
        return not_found();
    }
    let file_path = firmware_path(state, fw.id, &fw.filename);
    let Ok(file) = tokio::fs::File::open(&file_path).await else {
        tracing::warn!(path = %file_path.display(), "firmware file missing");
        return not_found();
    };
    let stream = tokio_util::io::ReaderStream::new(file);
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (header::CONTENT_LENGTH, fw.size_bytes.to_string()),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

/// Location of an uploaded firmware image on disk.
pub fn firmware_path(state: &AppState, id: Uuid, filename: &str) -> std::path::PathBuf {
    state
        .provisioning_dir
        .join("firmware")
        .join(id.to_string())
        .join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_file_names() {
        assert!(is_common_cfg("y000000000028.cfg"));
        assert!(!is_common_cfg("y00000000002x.cfg"));
        assert!(!is_common_cfg("y000000000000.boot"));
    }

    #[test]
    fn host_parsing() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "pbx.lan:8080".parse().unwrap());
        assert_eq!(base_url(&h), "http://pbx.lan:8080/provisioning");
        assert_eq!(sip_host(&h), "pbx.lan");
        h.insert(header::HOST, "[fd00::1]:8080".parse().unwrap());
        assert_eq!(sip_host(&h), "[fd00::1]");
        h.insert(header::HOST, "evil\"host".parse().unwrap());
        assert_eq!(sip_host(&h), "localhost");
    }
}
