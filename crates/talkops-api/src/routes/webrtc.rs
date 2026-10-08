//! Browser softphone: SIP credentials of the user's own browser device and
//! a WebSocket relay to FreeSWITCH's SIP-over-WebSocket listener, which is
//! bound to localhost and only reachable through TalkOps (logged-in users).

use axum::Json;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::extensions::{self, DeviceInput, DeviceKind};
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
    OpenApiRouter::new().routes(routes!(webrtc_account))
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
    }))
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
    _auth: AuthUser,
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
    ws.protocols(["sip"]).on_upgrade(move |socket| async move {
        if let Err(err) = relay(socket, &url).await {
            tracing::warn!(url, error = %err, "softphone WebSocket: FreeSWITCH not reachable");
        }
    })
}

async fn relay(client: WebSocket, url: &str) -> Result<(), tungstenite::Error> {
    let mut request = url.into_client_request()?;
    request.headers_mut().insert(
        "sec-websocket-protocol",
        tungstenite::http::HeaderValue::from_static("sip"),
    );
    let (upstream, _) = tokio_tungstenite::connect_async(request).await?;
    let (mut up_tx, mut up_rx) = upstream.split();
    let (mut cl_tx, mut cl_rx) = client.split();
    let to_fs = async {
        while let Some(Ok(msg)) = cl_rx.next().await {
            let out = match msg {
                Message::Text(t) => tungstenite::Message::Text(t.as_str().into()),
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
            let out = match msg {
                tungstenite::Message::Text(t) => Message::Text(t.as_str().into()),
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
    tokio::select! {
        _ = to_fs => {}
        _ = to_browser => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
