//! Endpoints called by FreeSWITCH: `mod_xml_curl` lookups and `mod_xml_cdr`
//! call records. Both use HTTP basic auth with the shared xml_curl password.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use axum::Form;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::AppState;
use crate::fsxml::{self, NOT_FOUND, dialplan, directory, sofia};
use crate::util::constant_time_eq;

/// Basic-auth user name FreeSWITCH sends (`gateway-credentials` in xml_curl.conf).
pub const XMLCURL_USER: &str = "talkops";

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
    constant_time_eq(&decoded, format!("{XMLCURL_USER}:{password}").as_bytes())
}

/// FreeSWITCH runs on the same host: the endpoints only answer loopback
/// peers (or the configured `TALKOPS_FS_PEERS`), and never requests relayed
/// by a reverse proxy.
pub async fn only_local(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let proxied = req.headers().contains_key("x-forwarded-for")
        || req.headers().contains_key(header::FORWARDED);
    let allowed = |ip: IpAddr| {
        ip.is_loopback()
            || state
                .fs_peers
                .iter()
                .any(|net| talkops_core::sip_guard::network_contains(*net, ip))
    };
    // No peer address: called in-process (tests).
    if proxied || peer.is_some_and(|ip| !allowed(ip)) {
        tracing::warn!(peer = ?peer, proxied, "FreeSWITCH endpoint refused for this client");
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Basic realm=\"talkops-fs\"")],
    )
        .into_response()
}

fn xml(body: String) -> Response {
    ([(header::CONTENT_TYPE, "text/xml; charset=utf-8")], body).into_response()
}

pub async fn xml_curl(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(params): Form<HashMap<String, String>>,
) -> Response {
    if !authorized(&headers, &state.xmlcurl_password) {
        return unauthorized();
    }
    let get = |k: &str| params.get(k).map(String::as_str).unwrap_or_default();
    let section = get("section");
    tracing::debug!(
        section,
        key_value = get("key_value"),
        action = get("action"),
        "xml_curl lookup"
    );

    let body = match section {
        "configuration" if get("key_value") == "sofia.conf" => sofia_conf(&state).await,
        "configuration" if get("key_value") == "callcenter.conf" => callcenter_conf(&state).await,
        "directory" => directory_user(&state, &params).await,
        "dialplan" => {
            let req = dialplan::CallRequest::from_params(&params);
            let routing = dialplan::Routing {
                pool: &state.db,
                catalog: &state.catalog,
                socket: &state.outbound_socket,
                recordings: &state.media.recordings,
                sounds: &state.media.sounds,
                telephony: &state.telephony,
                spam: &state.spam,
            };
            let actions = dialplan::plan(&routing, &req).await;
            if let (Some(station), Some(tenant)) = (
                dialplan::planned_var(&actions, "talkops_door_id").and_then(|v| v.parse().ok()),
                dialplan::planned_var(&actions, "talkops_tenant_id")
                    .and_then(|v| v.parse().ok())
                    .map(talkops_core::tenant::TenantId),
            ) {
                // Log the ring and take a snapshot without delaying the call.
                let ctx = crate::doors::DoorCtx::from(&state);
                let dialed = dialplan::planned_var(&actions, "talkops_door_dialed")
                    .unwrap_or_default()
                    .to_owned();
                tokio::spawn(async move { ctx.ring(tenant, station, &dialed).await });
            }
            Some(dialplan::render(&req.context, &actions))
        }
        _ => None,
    };
    xml(body.unwrap_or_else(|| NOT_FOUND.to_owned()))
}

async fn callcenter_conf(state: &AppState) -> Option<String> {
    let loaded = async {
        let queues = talkops_core::queues::list_all(&state.db).await?;
        let (agents, tiers) =
            talkops_core::queues::desired_agents(&state.db, fsxml::SIP_DOMAIN).await?;
        let mut hold_music = std::collections::HashMap::new();
        for q in &queues {
            if let std::collections::hash_map::Entry::Vacant(slot) = hold_music.entry(q.tenant_id) {
                let s = talkops_core::settings::get(&state.db, q.tenant_id).await?;
                let moh = fsxml::dialplan::hold_music(&state.media.sounds, q.tenant_id, &s);
                slot.insert(moh);
            }
        }
        talkops_core::error::CoreResult::Ok(fsxml::callcenter::render(
            &queues,
            &agents,
            &tiers,
            &state.media.sounds,
            &hold_music,
        ))
    }
    .await;
    match loaded {
        Ok(xml) => Some(xml),
        Err(err) => {
            tracing::error!(error = %err, "cannot load queues");
            None
        }
    }
}

/// Registers the recording of a finished call (and queues its transcript).
async fn store_recording(
    state: &AppState,
    tenant: talkops_core::tenant::TenantId,
    call_uuid: &str,
    file: &str,
) {
    if file.split('/').any(|p| p == ".." || p.is_empty()) || !file.ends_with(".wav") {
        tracing::warn!(file, "ignoring invalid recording path");
        return;
    }
    let path = state.media.recordings.join(file);
    let size = match tokio::fs::metadata(&path).await {
        Ok(m) => m.len() as i64,
        Err(_) => {
            // Unanswered calls leave no file.
            return;
        }
    };
    let secs = crate::voicemail::ivr::wav_seconds(&path).unwrap_or(0) as i32;
    if secs < 1 {
        let _ = tokio::fs::remove_file(&path).await;
        return;
    }
    let transcribe = talkops_core::settings::get(&state.db, tenant)
        .await
        .map(|s| s.transcription_enabled)
        .unwrap_or(false);
    if let Err(err) =
        talkops_core::recordings::create(&state.db, tenant, call_uuid, file, secs, size, transcribe)
            .await
    {
        tracing::error!(error = %err, "cannot store recording");
    }
}

async fn sofia_conf(state: &AppState) -> Option<String> {
    let rows = match talkops_core::trunks::active_gateways(&state.db).await {
        Ok(rows) => rows,
        Err(err) => {
            // Without the DB FreeSWITCH falls back to its bootstrap config.
            tracing::error!(error = %err, "cannot load gateways");
            return None;
        }
    };
    let gateways = sofia::gateway_specs(&rows, &state.catalog, &state.secrets);
    let mut profile = (*state.profile).clone();
    if let Ok(s) =
        talkops_core::settings::get(&state.db, talkops_core::tenant::TenantId::DEFAULT).await
    {
        profile.external_ip = s.external_ip;
    }
    Some(sofia::render(&profile, &gateways))
}

async fn directory_user(state: &AppState, params: &HashMap<String, String>) -> Option<String> {
    // Gateway/network-list lookups at profile start are not served from the directory.
    if params.contains_key("purpose") {
        return None;
    }
    // Banned addresses fail every authentication, even with the right password.
    if let Some(ip) = params.get("ip").and_then(|v| v.parse::<IpAddr>().ok()) {
        match talkops_core::sip_guard::is_banned(
            &state.db,
            talkops_core::tenant::TenantId::DEFAULT,
            ip,
        )
        .await
        {
            Ok(false) => {}
            Ok(true) => {
                tracing::debug!(%ip, "directory lookup from banned address refused");
                return None;
            }
            Err(err) => tracing::warn!(error = %err, "cannot check SIP bans"),
        }
    }
    let user = params.get("user").filter(|u| !u.is_empty())?;
    let device = match talkops_core::extensions::device_auth(&state.db, user).await {
        Ok(Some(device)) => device,
        Ok(None) => return None,
        Err(err) => {
            tracing::error!(error = %err, "directory lookup failed");
            return None;
        }
    };
    match state.secrets.decrypt(&device.sip_password_enc) {
        Ok(password) => Some(directory::render_user(&device, &password)),
        Err(err) => {
            tracing::error!(device = %device.device_id, error = %err, "cannot decrypt SIP password");
            None
        }
    }
}

pub async fn xml_cdr(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(params): Form<HashMap<String, String>>,
) -> Response {
    if !authorized(&headers, &state.xmlcurl_password) {
        return unauthorized();
    }
    let Some(doc) = params.get("cdr") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match fsxml::cdr::parse(doc) {
        Ok(Some((tenant, cdr, recording))) => {
            match talkops_core::cdr::insert(&state.db, tenant, &cdr).await {
                Ok(_) => {
                    if let Some(file) = recording {
                        store_recording(&state, tenant, &cdr.call_uuid, &file).await;
                    }
                    StatusCode::OK.into_response()
                }
                Err(err) => {
                    // A non-2xx answer makes mod_xml_cdr retry later.
                    tracing::error!(error = %err, "cannot store CDR");
                    StatusCode::SERVICE_UNAVAILABLE.into_response()
                }
            }
        }
        Ok(None) => StatusCode::OK.into_response(),
        Err(err) => {
            tracing::warn!(error = %err, "malformed CDR");
            StatusCode::OK.into_response()
        }
    }
}
