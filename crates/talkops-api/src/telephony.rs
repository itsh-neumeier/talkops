//! Live telephony state from FreeSWITCH (registrations, gateway states) and
//! reload commands after configuration changes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use talkops_core::error::CoreResult;
use talkops_core::{extensions, voicemail};
use tokio::sync::RwLock;

use crate::esl::EslHandle;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Registration {
    /// SIP username of the device.
    pub user: String,
    pub network_ip: String,
    pub network_port: String,
    pub transport: String,
    pub user_agent: String,
    pub expires: i64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct GatewayState {
    pub name: String,
    /// sofia registration state, e.g. `REGED`, `FAIL_WAIT`, `NOREG`, `TRYING`.
    pub state: String,
    /// `UP` or `DOWN` (OPTIONS ping status).
    pub status: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct LiveStatus {
    pub connected: bool,
    pub registrations: Vec<Registration>,
    pub gateways: HashMap<String, GatewayState>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Default)]
pub struct Telephony {
    pub esl: EslHandle,
    status: Arc<RwLock<LiveStatus>>,
}

impl Telephony {
    pub async fn snapshot(&self) -> LiveStatus {
        self.status.read().await.clone()
    }

    /// Polls FreeSWITCH every `interval`. Devices that newly registered get
    /// their voicemail indicator (MWI) right away.
    pub fn spawn_poller(&self, interval: Duration, db: PgPool) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut known: HashSet<String> = HashSet::new();
            loop {
                let status = this.poll().await;
                let current: HashSet<String> = status
                    .registrations
                    .iter()
                    .map(|r| r.user.to_lowercase())
                    .collect();
                let fresh: Vec<String> = current.difference(&known).cloned().collect();
                known = current;
                *this.status.write().await = status;
                for user in fresh {
                    this.initial_mwi(&db, &user).await;
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    async fn initial_mwi(&self, db: &PgPool, user: &str) {
        let result = async {
            let Some(device) = extensions::device_auth(db, user).await? else {
                return CoreResult::Ok(());
            };
            if voicemail::get_box(db, device.tenant_id, device.extension_id)
                .await?
                .enabled
            {
                let (new, saved) = voicemail::counts(db, device.extension_id).await?;
                self.send_mwi(&device.sip_username, new, saved).await;
            }
            Ok(())
        }
        .await;
        if let Err(err) = result {
            tracing::debug!(user, error = %err, "initial MWI failed");
        }
    }

    async fn poll(&self) -> LiveStatus {
        let Some(client) = self.esl.get().await else {
            return LiveStatus::default();
        };
        let registrations = match client.api("show registrations as json").await {
            Ok(json) => parse_registrations(&json),
            Err(err) => {
                tracing::debug!(error = %err, "show registrations failed");
                Vec::new()
            }
        };
        let gateways = match client.api("sofia xmlstatus gateway").await {
            Ok(xml) => parse_gateways(&xml),
            Err(err) => {
                tracing::debug!(error = %err, "sofia xmlstatus gateway failed");
                HashMap::new()
            }
        };
        LiveStatus {
            connected: true,
            registrations,
            gateways,
            updated_at: Some(Utc::now()),
        }
    }

    /// Re-reads gateways after trunk changes. Changed or removed gateways are
    /// killed first; `rescan` then adds the current set from TalkOps.
    pub async fn reload_gateways(&self, changed: &[String]) {
        let Some(client) = self.esl.get().await else {
            tracing::warn!("FreeSWITCH not connected; gateway changes apply on its next start");
            return;
        };
        for name in changed {
            if let Err(err) = client
                .api(&format!("sofia profile external killgw {name}"))
                .await
            {
                tracing::debug!(gateway = %name, error = %err, "killgw");
            }
        }
        if let Err(err) = client.api("sofia profile external rescan").await {
            tracing::warn!(error = %err, "sofia rescan failed");
        }
    }

    /// Asks a device to re-provision (SIP NOTIFY `check-sync`). Yealink phones
    /// reboot and fetch their configuration (`sip.notify_reboot_enable = 1`).
    pub async fn check_sync(&self, sip_user: &str) -> bool {
        let Some(client) = self.esl.get().await else {
            return false;
        };
        let cmd = format!(
            "sofia profile internal check_sync {sip_user}@{}",
            crate::fsxml::SIP_DOMAIN
        );
        match client.api(&cmd).await {
            Ok(_) => true,
            Err(err) => {
                tracing::warn!(user = sip_user, error = %err, "check_sync failed");
                false
            }
        }
    }

    /// Updates the message-waiting indicator (MWI LED) of a device.
    pub async fn send_mwi(&self, sip_user: &str, new: u32, saved: u32) -> bool {
        let Some(client) = self.esl.get().await else {
            return false;
        };
        let account = format!("{sip_user}@{}", crate::fsxml::SIP_DOMAIN);
        let waiting = if new > 0 { "yes" } else { "no" };
        let counts = format!("{new}/{saved} (0/0)");
        let headers = [
            ("MWI-Messages-Waiting", waiting),
            ("MWI-Message-Account", account.as_str()),
            ("MWI-Voice-Message", counts.as_str()),
        ];
        match client.sendevent("MESSAGE_WAITING", &headers).await {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(user = sip_user, error = %err, "MWI update failed");
                false
            }
        }
    }

    /// Restarts the external profile (needed when its own parameters change,
    /// e.g. the public IP). Interrupts active trunk calls.
    pub async fn restart_external_profile(&self) {
        if let Some(client) = self.esl.get().await {
            if let Err(err) = client.api("sofia profile external restart").await {
                tracing::warn!(error = %err, "sofia profile restart failed");
            }
        }
    }
}

fn parse_registrations(json: &str) -> Vec<Registration> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let s = |row: &serde_json::Value, key: &str| {
        row.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    value
        .get("rows")
        .and_then(|r| r.as_array())
        .map(|rows| {
            rows.iter()
                .map(|row| Registration {
                    user: s(row, "reg_user"),
                    network_ip: s(row, "network_ip"),
                    network_port: s(row, "network_port"),
                    transport: s(row, "network_proto"),
                    user_agent: s(row, "metadata"),
                    expires: s(row, "expires").parse().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_gateways(xml: &str) -> HashMap<String, GatewayState> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return HashMap::new();
    };
    doc.descendants()
        .filter(|n| n.has_tag_name("gateway"))
        .filter_map(|gw| {
            let text = |tag: &str| {
                gw.children()
                    .find(|c| c.has_tag_name(tag))
                    .and_then(|c| c.text())
                    .unwrap_or_default()
                    .trim()
                    .to_owned()
            };
            let name = text("name");
            if name.is_empty() {
                return None;
            }
            let last_error = Some(text("last_error")).filter(|e| !e.is_empty());
            Some((
                name.clone(),
                GatewayState {
                    name,
                    state: text("state"),
                    status: text("status"),
                    last_error,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registrations_json() {
        let json = r#"{"row_count":1,"rows":[{"reg_user":"20-1","realm":"talkops.local","expires":"1791150000","network_ip":"192.168.1.20","network_port":"5060","network_proto":"udp","metadata":"Yealink"}]}"#;
        let regs = parse_registrations(json);
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].user, "20-1");
        assert_eq!(regs[0].expires, 1791150000);
        assert!(parse_registrations("{\"row_count\":0}").is_empty());
        assert!(parse_registrations("-ERR").is_empty());
    }

    #[test]
    fn parses_gateway_xml() {
        let xml = r#"<?xml version="1.0"?><gateways>
          <gateway><name>gw-1</name><profile>external</profile><state>REGED</state><status>UP</status></gateway>
          <gateway><name>gw-2</name><state>FAIL_WAIT</state><status>DOWN</status><last_error>403</last_error></gateway>
        </gateways>"#;
        let gws = parse_gateways(xml);
        assert_eq!(gws["gw-1"].state, "REGED");
        assert_eq!(gws["gw-2"].last_error.as_deref(), Some("403"));
    }
}
