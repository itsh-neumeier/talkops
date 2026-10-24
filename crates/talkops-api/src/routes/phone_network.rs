//! Network data of provisioned phones from the Ubiquiti Networks API (the
//! UniFi connection, ADR 0041). Off until an administrator switches it on.
//!
//! One classic `stat/sta` request reads all connected clients; TalkOps
//! matches them to the phones by MAC. The answer stays in a short cache, so
//! several open phone pages cause one console request per interval. This
//! module reads only.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::phones;
use talkops_core::roles::Permission;
use talkops_core::tenant::TenantId;
use talkops_core::unifi::{self, UnifiConnection};
use talkops_unifi::{NetClient, colon_mac};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::wifi_survey::UnifiView;
use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiResult;

/// Age of a cached console answer that TalkOps uses again.
const CACHE_TTL: Duration = Duration::from_secs(15);

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(phone_network))
        .routes(routes!(set_phone_network))
}

/// Network state of one provisioned phone.
#[derive(Debug, Serialize, ToSchema)]
pub struct PhoneNetwork {
    pub phone_id: Uuid,
    #[schema(value_type = Object)]
    pub client: NetClient,
    /// Name of the access point (Wi-Fi) from the console.
    pub ap_name: Option<String>,
    /// Name of the switch (wired) from the console.
    pub switch_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PhoneNetworkView {
    /// `not_configured`, `disabled`, `ok` or `error`.
    pub state: &'static str,
    /// Error class for `error`: `unauthorized`, `not_found`, `certificate`,
    /// `unreachable` or `invalid`.
    pub error: Option<&'static str>,
    /// Time of the console answer.
    pub read_at: Option<DateTime<Utc>>,
    /// Connected phones only; a phone that is not in the list is offline or
    /// unknown to the console.
    pub phones: Vec<PhoneNetwork>,
}

impl PhoneNetworkView {
    fn empty(state: &'static str) -> Self {
        Self {
            state,
            error: None,
            read_at: None,
            phones: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct Snapshot {
    at: Instant,
    read_at: DateTime<Utc>,
    clients: Vec<NetClient>,
    /// Device names (access points and switches) by MAC.
    names: HashMap<String, String>,
}

/// Cache by tenant; the console address and site are part of the entry, so
/// a changed connection reads again.
type Cache = HashMap<TenantId, (String, Snapshot)>;
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Mutex::default);

async fn snapshot(
    state: &AppState,
    tenant: TenantId,
    conn: &UnifiConnection,
) -> Result<Snapshot, talkops_unifi::UnifiError> {
    let key = format!("{}|{}", conn.url, conn.site);
    if let Some((k, s)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).get(&tenant)
        && *k == key
        && s.at.elapsed() < CACHE_TTL
    {
        return Ok(s.clone());
    }
    let api_key = unifi::api_key(&state.db, tenant, &state.secrets)
        .await
        .map_err(|_| talkops_unifi::UnifiError::Invalid)?;
    let client = talkops_unifi::Client::new(talkops_unifi::Config {
        url: conn.url.clone(),
        site: conn.site.clone(),
        api_key,
        cert_sha256: conn.cert_sha256.clone(),
    })?;
    let clients = client.clients().await?;
    // Names are optional: without them the page shows the MAC.
    let names = client.ap_names(conn.site_id()).await.unwrap_or_default();
    let snap = Snapshot {
        at: Instant::now(),
        read_at: Utc::now(),
        clients,
        names,
    };
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(tenant, (key, snap.clone()));
    Ok(snap)
}

/// Matches clients to phones by MAC.
fn match_phones(
    phones: &[(Uuid, String)],
    clients: &[NetClient],
    names: &HashMap<String, String>,
) -> Vec<PhoneNetwork> {
    let by_mac: HashMap<&str, &NetClient> = clients.iter().map(|c| (c.mac.as_str(), c)).collect();
    phones
        .iter()
        .filter_map(|(id, mac)| {
            let client = *by_mac.get(colon_mac(mac)?.as_str())?;
            let name = |m: &Option<String>| m.as_ref().and_then(|m| names.get(m)).cloned();
            Some(PhoneNetwork {
                phone_id: *id,
                ap_name: name(&client.ap_mac),
                switch_name: name(&client.sw_mac),
                client: client.clone(),
            })
        })
        .collect()
}

/// Network data of the provisioned phones (Wi-Fi or wired).
#[utoipa::path(get, path = "/api/v1/phones/network", tag = "phones", responses((status = 200, body = PhoneNetworkView)))]
pub async fn phone_network(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<PhoneNetworkView>> {
    auth.require_perm(Permission::PhonesView)?;
    let Some(conn) = unifi::get(&state.db, auth.tenant).await? else {
        return Ok(Json(PhoneNetworkView::empty("not_configured")));
    };
    if !conn.phone_network {
        return Ok(Json(PhoneNetworkView::empty("disabled")));
    }
    let snap = match snapshot(&state, auth.tenant, &conn).await {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(error = %e, "phone network data not available");
            return Ok(Json(PhoneNetworkView {
                error: Some(e.class()),
                ..PhoneNetworkView::empty("error")
            }));
        }
    };
    let phones: Vec<(Uuid, String)> = phones::list(&state.db, auth.tenant)
        .await?
        .into_iter()
        .map(|p| (p.id, p.mac))
        .collect();
    Ok(Json(PhoneNetworkView {
        state: "ok",
        error: None,
        read_at: Some(snap.read_at),
        phones: match_phones(&phones, &snap.clients, &snap.names),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PhoneNetworkInput {
    pub enabled: bool,
}

/// Switches "Show network data on phones" on or off.
#[utoipa::path(put, path = "/api/v1/unifi/phone-network", tag = "wifi-survey", request_body = PhoneNetworkInput, responses((status = 200, body = UnifiView)))]
pub async fn set_phone_network(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<PhoneNetworkInput>,
) -> ApiResult<Json<UnifiView>> {
    auth.require_perm(Permission::UnifiManage)?;
    let conn = unifi::set_phone_network(&state.db, auth.tenant, input.enabled).await?;
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        "unifi.phone_network",
        "wifi_survey",
        None,
        json!({ "enabled": input.enabled }),
    )
    .await;
    Ok(Json(UnifiView {
        connection: Some(conn),
        has_api_key: true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_phones_by_mac() {
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let clients = vec![NetClient {
            mac: "80:5e:c0:00:00:01".into(),
            ap_mac: Some("f4:92:bf:00:00:02".into()),
            ..NetClient::default()
        }];
        let names = HashMap::from([("f4:92:bf:00:00:02".to_owned(), "Hall AP".to_owned())]);
        let phones = vec![
            (id, "805EC0000001".to_owned()),
            (other, "805ec0000009".to_owned()),
        ];
        let out = match_phones(&phones, &clients, &names);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].phone_id, id);
        assert_eq!(out[0].ap_name.as_deref(), Some("Hall AP"));
        assert_eq!(out[0].switch_name, None);
    }
}
