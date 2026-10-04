//! Endpoints called by FreeSWITCH: `mod_xml_curl` lookups and `mod_xml_cdr`
//! call records. Both use HTTP basic auth with the shared xml_curl password.

use std::collections::HashMap;

use axum::Form;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
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
        "directory" => directory_user(&state, &params).await,
        "dialplan" => {
            let req = dialplan::CallRequest::from_params(&params);
            let actions = dialplan::plan(&state.db, &state.catalog, &req).await;
            Some(dialplan::render(&req.context, &actions))
        }
        _ => None,
    };
    xml(body.unwrap_or_else(|| NOT_FOUND.to_owned()))
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
        Ok(Some((tenant, cdr))) => match talkops_core::cdr::insert(&state.db, tenant, &cdr).await {
            Ok(_) => StatusCode::OK.into_response(),
            Err(err) => {
                // A non-2xx answer makes mod_xml_cdr retry later.
                tracing::error!(error = %err, "cannot store CDR");
                StatusCode::SERVICE_UNAVAILABLE.into_response()
            }
        },
        Ok(None) => StatusCode::OK.into_response(),
        Err(err) => {
            tracing::warn!(error = %err, "malformed CDR");
            StatusCode::OK.into_response()
        }
    }
}
