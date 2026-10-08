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

/// A call in progress (`show calls`).
#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
pub struct ActiveCall {
    pub uuid: String,
    pub caller_number: String,
    pub caller_name: String,
    /// Dialed number.
    pub destination: String,
    /// Who answered or is ringing, if connected to another phone.
    pub callee_number: String,
    pub callee_name: String,
    /// `ringing`, `talking`, `held`, `parked` or `system` (menu, voicemail,
    /// queue).
    pub state: String,
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Default)]
pub struct Telephony {
    pub esl: EslHandle,
    status: Arc<RwLock<LiveStatus>>,
    /// Park slots just handed out, until the call shows up in the lot.
    park_reservations: Arc<std::sync::Mutex<HashMap<String, std::time::Instant>>>,
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

    /// Calls in progress; `None` if FreeSWITCH is unreachable.
    pub async fn active_calls(&self) -> Option<Vec<ActiveCall>> {
        let client = self.esl.get().await?;
        let out = client.api("show calls as json").await.ok()?;
        Some(parse_calls(&out))
    }

    /// Callers waiting in a mod_callcenter queue; `None` if unknown.
    pub async fn queue_waiting(&self, queue: &str) -> Option<usize> {
        let client = self.esl.get().await?;
        let out = client
            .api(&format!("callcenter_config queue list members {queue}"))
            .await
            .ok()?;
        Some(crate::callcenter::count_waiting(&out))
    }

    /// Reserves a free park slot (`*51` … `*59`) for a call about to be
    /// parked; `None` if all are taken or FreeSWITCH is unreachable.
    pub async fn reserve_park_slot(&self) -> Option<String> {
        let client = self.esl.get().await?;
        let lot = crate::fsxml::dialplan::PARK_LOT;
        let info = client.api(&format!("valet_info {lot}")).await.ok()?;
        let taken = parse_valet_slots(&info);
        let mut reserved = self
            .park_reservations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        reserved.retain(|_, at| at.elapsed() < Duration::from_secs(10));
        let slot = (1..=9)
            .map(|n| format!("*5{n}"))
            .find(|s| !taken.contains(s) && !reserved.contains_key(s))?;
        reserved.insert(slot.clone(), std::time::Instant::now());
        Some(slot)
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

fn parse_calls(json: &str) -> Vec<ActiveCall> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(rows) = value.get("rows").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    rows.iter()
        .map(|r| {
            let f = |k: &str| {
                r.get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned()
            };
            let bridged = !f("b_uuid").is_empty();
            let parked = f("dest").starts_with("park+");
            let state = match (f("callstate").as_str(), bridged) {
                _ if parked => "parked",
                ("RINGING" | "EARLY" | "RING_WAIT", _) => "ringing",
                ("HELD", _) => "held",
                (_, true) => "talking",
                (_, false) => "system",
            };
            let (callee_number, callee_name) = if bridged {
                let number = f("b_cid_num");
                let number = if number.is_empty() || number == f("cid_num") {
                    f("callee_num")
                } else {
                    number
                };
                (number, f("callee_name"))
            } else {
                (String::new(), String::new())
            };
            let started_at = f("created_epoch")
                .parse::<i64>()
                .ok()
                .and_then(|s| DateTime::from_timestamp(s, 0));
            // Phones call with their device login (`20-1`): show the extension.
            let device = f("cid_num");
            let caller_number = match device.split_once('-') {
                Some((ext, n))
                    if !ext.is_empty()
                        && ext.bytes().all(|b| b.is_ascii_digit())
                        && n.bytes().all(|b| b.is_ascii_digit()) =>
                {
                    ext.to_owned()
                }
                _ => device.clone(),
            };
            let caller_name = Some(f("cid_name"))
                .filter(|n| *n != device)
                .unwrap_or_default();
            ActiveCall {
                uuid: f("uuid"),
                caller_number,
                caller_name,
                destination: f("dest").trim_start_matches("park+").to_owned(),
                callee_number,
                callee_name,
                state: state.to_owned(),
                started_at,
            }
        })
        .collect()
}

/// Slot names in `valet_info` output (`<extension uuid="…">*51</extension>`).
fn parse_valet_slots(xml: &str) -> HashSet<String> {
    xml.split("<extension")
        .skip(1)
        .filter_map(|part| {
            let inner = &part[part.find('>')? + 1..];
            Some(inner[..inner.find("</extension>")?].trim().to_owned())
        })
        .collect()
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
    fn parses_show_calls() {
        let json = r#"{"row_count":2,"rows":[
            {"uuid":"a","created_epoch":"1791450000","cid_name":"Anna","cid_num":"21",
             "dest":"20","callstate":"ACTIVE","callee_name":"Timo","callee_num":"20",
             "b_uuid":"b","b_cid_num":"21","b_callstate":"ACTIVE"},
            {"uuid":"c","created_epoch":"1791450100","cid_name":"","cid_num":"+4930123",
             "dest":"70","callstate":"ACTIVE","b_uuid":""}]}"#;
        let calls = parse_calls(json);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].state, "talking");
        assert_eq!(calls[0].callee_number, "20");
        assert_eq!(calls[0].callee_name, "Timo");
        assert_eq!(calls[1].state, "system");
        assert_eq!(calls[1].started_at.unwrap().timestamp(), 1791450100);
        assert!(parse_calls(r#"{"row_count":0}"#).is_empty());
        let parked = parse_calls(
            r#"{"rows":[{"uuid":"p","cid_num":"23-2","cid_name":"23-2","dest":"park+*51",
                "callstate":"ACTIVE"}]}"#,
        );
        assert_eq!(parked[0].state, "parked");
        assert_eq!(parked[0].caller_number, "23");
        assert_eq!(parked[0].caller_name, "");
        assert_eq!(parked[0].destination, "*51");
    }

    #[test]
    fn parses_valet_info() {
        let xml = "<lots>\n<lot name=\"talkops\">\n<extension uuid=\"a-b\">*51</extension>\n\
                   <extension uuid=\"c-d\">*53</extension>\n</lot>\n</lots>\n";
        let slots = parse_valet_slots(xml);
        assert_eq!(slots.len(), 2);
        assert!(slots.contains("*51") && slots.contains("*53"));
        assert!(parse_valet_slots("<lots></lots>").is_empty());
    }

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
