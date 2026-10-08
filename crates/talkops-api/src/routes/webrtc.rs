//! Browser softphone: SIP credentials of the user's own browser device and
//! a WebSocket relay to FreeSWITCH's SIP-over-WebSocket listener, which is
//! bound to localhost and only reachable through TalkOps (logged-in users).

use axum::Json;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use std::net::IpAddr;

use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::extensions::{self, DeviceInput, DeviceKind};
use talkops_core::turn::{self, IceServer};
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::fsxml::SIP_DOMAIN;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(webrtc_account))
        .routes(routes!(conference_add))
}

#[derive(Deserialize, ToSchema)]
pub struct AccountRequest {
    /// Extension to use; default: the user's first extension.
    #[serde(default)]
    pub extension_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct WebrtcAccount {
    pub extension_id: Uuid,
    pub extension_number: String,
    pub display_name: String,
    pub sip_username: String,
    pub sip_password: String,
    pub sip_domain: &'static str,
    /// WebSocket path on this server (wss:// when served over HTTPS).
    pub ws_path: &'static str,
    /// TURN relays with short-lived credentials (empty: direct media only).
    pub ice_servers: Vec<IceServer>,
    /// Use only the TURN relays for media.
    pub relay_only: bool,
}

/// SIP account of the own browser softphone; created on first use.
#[utoipa::path(post, path = "/api/v1/me/webrtc", tag = "extensions", request_body = AccountRequest, responses((status = 200, body = WebrtcAccount), (status = 404, description = "the user has no extension")))]
pub async fn webrtc_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<AccountRequest>,
) -> ApiResult<Json<WebrtcAccount>> {
    let own = extensions::list_for_user(&state.db, auth.tenant, auth.id).await?;
    let ext = match req.extension_id {
        Some(id) => own.into_iter().find(|e| e.id == id),
        None => own.into_iter().find(|e| e.enabled),
    }
    .ok_or(ApiError::NotFound)?;
    let devices = extensions::list_devices(&state.db, auth.tenant, ext.id).await?;
    let (device, password) = match devices.into_iter().find(|d| d.kind == DeviceKind::Browser) {
        Some(d) => {
            let pw = state
                .secrets
                .decrypt(&d.sip_password_enc)
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            (d, pw)
        }
        None => {
            let input = DeviceInput {
                name: "Browser".into(),
                kind: DeviceKind::Browser,
                sip_username: None,
                phone_id: None,
                account_index: None,
                enabled: true,
            };
            let (d, pw) =
                extensions::create_device(&state.db, auth.tenant, &state.secrets, &ext, &input)
                    .await?;
            audit::record(
                &state.db,
                &auth.actor(),
                "create",
                "device",
                Some(d.id.to_string()),
                json!({"extension": ext.number, "sip_username": d.sip_username, "kind": "browser"}),
            )
            .await?;
            (d, pw)
        }
    };
    if !device.enabled {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(WebrtcAccount {
        extension_id: ext.id,
        extension_number: ext.number,
        display_name: ext.display_name,
        sip_username: device.sip_username,
        sip_password: password,
        sip_domain: SIP_DOMAIN,
        ws_path: "/api/v1/webrtc/ws",
        ice_servers: state
            .turn
            .as_ref()
            .and_then(|t| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or_default();
                turn::ice_server(&t.urls, &t.secret, &auth.id.to_string(), now)
            })
            .into_iter()
            .collect(),
        relay_only: state.turn.as_ref().is_some_and(|t| t.relay_only),
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct ConferenceRequest {
    /// Extension of the softphone (as in [WebrtcAccount]).
    pub extension_id: Uuid,
    /// SIP Call-ID of the softphone's current call.
    pub call_id: String,
    /// Who to add (internal or external number).
    pub number: String,
}

#[derive(Serialize, ToSchema)]
pub struct ConferenceResponse {
    pub conference: String,
}

/// Turns the softphone's current call into a conference and dials another
/// participant into it (again for further participants).
#[utoipa::path(post, path = "/api/v1/me/webrtc/conference", tag = "extensions", request_body = ConferenceRequest, responses((status = 202, body = ConferenceResponse), (status = 409, description = "no active call")))]
pub async fn conference_add(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ConferenceRequest>,
) -> ApiResult<(axum::http::StatusCode, Json<ConferenceResponse>)> {
    use crate::conference::{self, ConferenceError, Initiator};
    let ext = extensions::list_for_user(&state.db, auth.tenant, auth.id)
        .await?
        .into_iter()
        .find(|e| e.id == req.extension_id && e.enabled)
        .ok_or(ApiError::NotFound)?;
    let number = conference::clean_number(&req.number)
        .ok_or_else(|| ApiError::BadRequest("invalid number".into()))?;
    if req.call_id.is_empty() || req.call_id.len() > 256 {
        return Err(ApiError::BadRequest("invalid call id".into()));
    }
    let client = state
        .telephony
        .esl
        .get()
        .await
        .ok_or_else(|| ApiError::Internal(ConferenceError::Unavailable.to_string()))?;
    let who = Initiator {
        tenant: auth.tenant.0,
        extension_id: ext.id,
        extension_number: &ext.number,
        display_name: &ext.display_name,
        call_id: &req.call_id,
    };
    let room = conference::add_participant(client, &who, &number)
        .await
        .map_err(|e| match e {
            ConferenceError::NoCall => ApiError::Conflict(e.to_string()),
            other => ApiError::Internal(other.to_string()),
        })?;
    audit::record(
        &state.db,
        &auth.actor(),
        "conference",
        "call",
        Some(room.clone()),
        json!({"extension": ext.number, "number": number}),
    )
    .await?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(ConferenceResponse { conference: room }),
    ))
}

/// Same-origin check against cross-site WebSocket hijacking (the session
/// cookie is SameSite=Strict already; this is defence in depth). Behind a
/// reverse proxy that rewrites `Host`, the original host comes from
/// `X-Forwarded-Host` or `Forwarded: host=…`.
fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        // Non-browser clients send no Origin.
        return true;
    };
    let Some((_, origin_host)) = origin.split_once("://") else {
        return false;
    };
    let origin_host = origin_host.trim_end_matches('/');
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let forwarded = header("forwarded").into_iter().flat_map(|v| {
        v.split([';', ','])
            .filter_map(|p| p.trim().strip_prefix("host="))
            .map(|h| h.trim_matches('"'))
    });
    let forwarded_host = header("x-forwarded-host")
        .into_iter()
        .flat_map(|v| v.split(',').map(str::trim));
    header("host")
        .into_iter()
        .chain(forwarded_host)
        .chain(forwarded)
        .any(|h| h.eq_ignore_ascii_case(origin_host))
}

/// `GET /api/v1/webrtc/ws`: SIP over WebSocket (subprotocol `sip`) for
/// logged-in users, relayed to FreeSWITCH.
pub async fn sip_ws(
    State(state): State<AppState>,
    auth: AuthUser,
    connect: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !same_origin(&headers) {
        let value = |name: header::HeaderName| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-")
                .to_owned()
        };
        tracing::warn!(
            origin = value(header::ORIGIN),
            host = value(header::HOST),
            x_forwarded_host = value(header::HeaderName::from_static("x-forwarded-host")),
            "softphone WebSocket refused: Origin does not match the host \
             (reverse proxy: pass the original Host or set X-Forwarded-Host)"
        );
        return ApiError::Forbidden.into_response();
    }
    let url = state.sip_ws_url.to_string();
    let user = auth.id;
    let browser_ip = browser_ip(&headers, connect.map(|c| c.0.0.ip()));
    ws.protocols(["sip"]).on_upgrade(move |socket| async move {
        match relay(socket, &url, browser_ip).await {
            Ok(end) => tracing::info!(
                %user,
                to_freeswitch = end.to_fs,
                to_browser = end.to_browser,
                closed_by = end.closed_by,
                "softphone WebSocket closed"
            ),
            Err(err) => {
                tracing::warn!(url, error = %err, "softphone WebSocket: FreeSWITCH not reachable")
            }
        }
    })
}

/// Is `line` a Via header (long or compact form)?
fn is_via(line: &str) -> bool {
    let name = line.split(':').next().unwrap_or_default().trim();
    name.eq_ignore_ascii_case("via") || name.eq_ignore_ascii_case("v")
}

/// Rewrites the transport in the Via headers of a SIP message; the body
/// (after the first empty line) is left alone, so Content-Length stays valid.
fn rewrite_via(msg: &str, from: &str, to: &str) -> Option<String> {
    let (head, body) = match msg.find("\r\n\r\n") {
        Some(i) => msg.split_at(i),
        None => (msg, ""),
    };
    if !head.split("\r\n").any(|l| is_via(l) && l.contains(from)) {
        return None;
    }
    let head = head
        .split("\r\n")
        .map(|l| {
            if is_via(l) {
                l.replace(from, to)
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    Some(head + body)
}

/// Browsers on an `https://` page announce the transport `WSS` in Via, but
/// FreeSWITCH only has a plain WebSocket listener (TLS ends at TalkOps or the
/// reverse proxy) and would send its answers to a WSS transport it does not
/// have. The relay presents `WS` to FreeSWITCH and `WSS` to the browser.
fn to_freeswitch(msg: &str, secure: &std::sync::atomic::AtomicBool) -> String {
    match rewrite_via(msg, "SIP/2.0/WSS ", "SIP/2.0/WS ") {
        Some(out) => {
            secure.store(true, std::sync::atomic::Ordering::Relaxed);
            out
        }
        None => msg.to_owned(),
    }
}

fn to_browser(msg: &str, secure: &std::sync::atomic::AtomicBool) -> String {
    if secure.load(std::sync::atomic::Ordering::Relaxed) {
        rewrite_via(msg, "SIP/2.0/WS ", "SIP/2.0/WSS ").unwrap_or_else(|| msg.to_owned())
    } else {
        msg.to_owned()
    }
}

/// The browser's address as seen in the network: from the reverse proxy
/// (`X-Forwarded-For`, `X-Real-IP`) when the direct peer is on the local
/// network, else the peer itself.
fn browser_ip(headers: &HeaderMap, peer: Option<IpAddr>) -> Option<IpAddr> {
    let local = |ip: &IpAddr| match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
    };
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|v| v.trim().parse::<IpAddr>().ok())
    };
    match peer {
        Some(p) if local(&p) => header("x-forwarded-for")
            .or_else(|| header("x-real-ip"))
            .or(Some(p)),
        other => other,
    }
}

/// Browsers hide their address in ICE candidates behind random mDNS names
/// (`<uuid>.local`), which FreeSWITCH cannot resolve: the call then fails
/// with INCOMPATIBLE_DESTINATION. The relay replaces such host candidates
/// with the browser's address and fixes Content-Length. `None`: unchanged.
fn resolve_mdns_candidates(msg: &str, ip: IpAddr) -> Option<String> {
    let split = msg.find("\r\n\r\n")?;
    let (head, body) = (&msg[..split], &msg[split + 4..]);
    if !body.contains(".local ") {
        return None;
    }
    let addr = ip.to_string();
    let mut changed = false;
    let body: String = body
        .split_inclusive('\n')
        .map(|line| {
            // a=candidate:<foundation> <component> <proto> <prio> <addr> <port> typ …
            if !line.starts_with("a=candidate:") {
                return line.to_owned();
            }
            let fields: Vec<&str> = line.split(' ').collect();
            match fields.get(4) {
                Some(host) if host.ends_with(".local") => {
                    changed = true;
                    let mut fields = fields.clone();
                    fields[4] = &addr;
                    fields.join(" ")
                }
                _ => line.to_owned(),
            }
        })
        .collect();
    if !changed {
        return None;
    }
    let head = head
        .split("\r\n")
        .map(|l| {
            let name = l.split(':').next().unwrap_or_default().trim();
            if name.eq_ignore_ascii_case("content-length") || name.eq_ignore_ascii_case("l") {
                format!("{name}: {}", body.len())
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    Some(format!("{head}\r\n\r\n{body}"))
}

/// How a relayed connection ended (for the log).
struct RelayEnd {
    /// Messages relayed in each direction.
    to_fs: usize,
    to_browser: usize,
    closed_by: &'static str,
}

async fn relay(
    client: WebSocket,
    url: &str,
    browser_ip: Option<IpAddr>,
) -> Result<RelayEnd, tungstenite::Error> {
    let mut request = url.into_client_request()?;
    request.headers_mut().insert(
        "sec-websocket-protocol",
        tungstenite::http::HeaderValue::from_static("sip"),
    );
    let (upstream, _) = tokio_tungstenite::connect_async(request).await?;
    tracing::info!("softphone WebSocket connected to FreeSWITCH");
    let (mut up_tx, mut up_rx) = upstream.split();
    let (mut cl_tx, mut cl_rx) = client.split();
    let secure = std::sync::atomic::AtomicBool::new(false);
    let mut to_fs_count = 0;
    let mut to_browser_count = 0;
    let to_fs = async {
        while let Some(Ok(msg)) = cl_rx.next().await {
            to_fs_count += 1;
            let out = match msg {
                Message::Text(t) => {
                    let msg = to_freeswitch(t.as_str(), &secure);
                    let msg = match browser_ip {
                        Some(ip) => resolve_mdns_candidates(&msg, ip).unwrap_or(msg),
                        None => msg,
                    };
                    tungstenite::Message::Text(msg.into())
                }
                Message::Binary(b) => tungstenite::Message::Binary(b),
                Message::Ping(p) => tungstenite::Message::Ping(p),
                Message::Pong(p) => tungstenite::Message::Pong(p),
                Message::Close(_) => break,
            };
            if up_tx.send(out).await.is_err() {
                break;
            }
        }
        let _ = up_tx.close().await;
    };
    let to_browser = async {
        while let Some(Ok(msg)) = up_rx.next().await {
            to_browser_count += 1;
            let out = match msg {
                tungstenite::Message::Text(t) => {
                    Message::Text(to_browser(t.as_str(), &secure).into())
                }
                tungstenite::Message::Binary(b) => Message::Binary(b),
                tungstenite::Message::Ping(p) => Message::Ping(p),
                tungstenite::Message::Pong(p) => Message::Pong(p),
                tungstenite::Message::Close(_) | tungstenite::Message::Frame(_) => break,
            };
            if cl_tx.send(out).await.is_err() {
                break;
            }
        }
        let _ = cl_tx.close().await;
    };
    // Either side closing ends the relay.
    let closed_by = tokio::select! {
        _ = to_fs => "browser",
        _ = to_browser => "freeswitch",
    };
    Ok(RelayEnd {
        to_fs: to_fs_count,
        to_browser: to_browser_count,
        closed_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mdns_candidates() {
        let body = "v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\nc=IN IP4 0.0.0.0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
            a=candidate:1 1 udp 2113937151 dd83a502-f4be-4e9e-9833-bfd73f019f95.local 59361 typ host generation 0\r\n\
            a=candidate:2 1 udp 1677729535 91.1.2.3 40000 typ srflx raddr 0.0.0.0 rport 0\r\n";
        let msg = format!(
            "INVITE sip:0152@talkops.local SIP/2.0\r\nVia: SIP/2.0/WS x.invalid\r\nContent-Type: application/sdp\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let ip: IpAddr = "192.168.140.55".parse().unwrap();
        let out = resolve_mdns_candidates(&msg, ip).unwrap();
        let (head, new_body) = out.split_once("\r\n\r\n").unwrap();
        assert!(new_body.contains(
            "a=candidate:1 1 udp 2113937151 192.168.140.55 59361 typ host generation 0\r\n"
        ));
        assert!(new_body.contains("91.1.2.3 40000 typ srflx"));
        assert!(head.ends_with(&format!("Content-Length: {}", new_body.len())));
        assert!(resolve_mdns_candidates("REGISTER sip:x SIP/2.0\r\n\r\n", ip).is_none());

        // Address: proxy header from a local peer, otherwise the peer.
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            "192.168.140.55, 10.0.0.1".parse().unwrap(),
        );
        let lan: IpAddr = "192.168.140.30".parse().unwrap();
        assert_eq!(browser_ip(&h, Some(lan)), Some(ip));
        let public: IpAddr = "8.8.8.8".parse().unwrap();
        assert_eq!(browser_ip(&h, Some(public)), Some(public));
        assert_eq!(browser_ip(&HeaderMap::new(), Some(lan)), Some(lan));
    }

    #[test]
    fn via_transport_rewrite() {
        use std::sync::atomic::AtomicBool;
        let register = "REGISTER sip:talkops.local SIP/2.0\r\n\
            Via: SIP/2.0/WSS abc.invalid;branch=z9hG4bK1\r\n\
            To: <sip:610-1@talkops.local>\r\n\
            Contact: <sip:x@abc.invalid;transport=ws>\r\n\
            Content-Length: 0\r\n\r\n";
        let secure = AtomicBool::new(false);
        let out = to_freeswitch(register, &secure);
        assert!(out.contains("Via: SIP/2.0/WS abc.invalid;branch=z9hG4bK1\r\n"));
        assert_eq!(out.len(), register.len() - 1);
        assert!(secure.load(std::sync::atomic::Ordering::Relaxed));
        let answer = "SIP/2.0 401 Unauthorized\r\n\
            Via: SIP/2.0/WS abc.invalid;branch=z9hG4bK1;received=127.0.0.1\r\n\
            Content-Length: 0\r\n\r\n";
        assert!(to_browser(answer, &secure).contains("Via: SIP/2.0/WSS abc.invalid"));
        // Plain ws:// pages are left alone, and so are bodies.
        let plain = AtomicBool::new(false);
        assert_eq!(to_browser(answer, &plain), answer);
        let with_body = "MESSAGE sip:a SIP/2.0\r\nv: SIP/2.0/WSS h\r\n\r\nSIP/2.0/WSS in body";
        let out = to_freeswitch(with_body, &plain);
        assert!(out.starts_with("MESSAGE sip:a SIP/2.0\r\nv: SIP/2.0/WS h\r\n"));
        assert!(out.ends_with("SIP/2.0/WSS in body"));
    }

    #[test]
    fn origin_check() {
        let h = |origin: Option<&str>, host: &str| {
            let mut m = HeaderMap::new();
            m.insert(header::HOST, host.parse().unwrap());
            if let Some(o) = origin {
                m.insert(header::ORIGIN, o.parse().unwrap());
            }
            m
        };
        assert!(same_origin(&h(Some("https://pbx.local"), "pbx.local")));
        assert!(same_origin(&h(
            Some("http://pbx.local:8080"),
            "pbx.local:8080"
        )));
        assert!(!same_origin(&h(Some("https://evil.example"), "pbx.local")));
        assert!(same_origin(&h(None, "pbx.local")));
        // Behind a proxy that rewrites Host.
        let mut m = h(Some("https://talk.example.de"), "10.0.0.5:8095");
        assert!(!same_origin(&m));
        m.insert("x-forwarded-host", "talk.example.de".parse().unwrap());
        assert!(same_origin(&m));
        let mut m = h(Some("https://talk.example.de"), "10.0.0.5:8095");
        m.insert(
            "forwarded",
            "for=1.2.3.4;proto=https;host=\"talk.example.de\""
                .parse()
                .unwrap(),
        );
        assert!(same_origin(&m));
    }
}
