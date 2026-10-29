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
    /// SIP Call-ID of the REGISTER dialog (Cisco service-control needs it).
    #[serde(skip)]
    pub call_id: String,
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
    /// FreeSWITCH uptime in seconds (for the restart notification).
    #[serde(skip)]
    pub uptime: Option<u64>,
    /// False if a query failed (registrations or gateways may be missing).
    #[serde(skip)]
    pub complete: bool,
}

/// A call in progress (`show calls`).
#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
pub struct ActiveCall {
    pub uuid: String,
    #[serde(skip)]
    pub bridge_uuid: String,
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
            let mut contacts: HashMap<String, std::time::Instant> = HashMap::new();
            let mut monitor = crate::notifier::Monitor::default();
            loop {
                let status = this.poll().await;
                if !status.connected || status.complete {
                    monitor.observe(&db, observed(&status)).await;
                }
                record_cisco_contacts(&db, &status.registrations, &mut contacts).await;
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
        let mut complete = true;
        let registrations = match load_registrations(&client).await {
            Ok(registrations) => registrations,
            Err(err) => {
                tracing::debug!(error = %err, "show registrations failed");
                complete = false;
                Vec::new()
            }
        };
        let gateways = match client.api("sofia xmlstatus gateway").await {
            Ok(xml) => parse_gateways(&xml),
            Err(err) => {
                tracing::debug!(error = %err, "sofia xmlstatus gateway failed");
                complete = false;
                HashMap::new()
            }
        };
        let uptime = client
            .api("uptime s")
            .await
            .ok()
            .and_then(|s| s.trim().parse().ok());
        LiveStatus {
            connected: true,
            registrations,
            gateways,
            updated_at: Some(Utc::now()),
            uptime,
            complete,
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

    /// Asks a device to re-provision (SIP NOTIFY `check-sync`). Yealink
    /// phones fetch their configuration; with `reboot` they restart, too
    /// (`check-sync;reboot=true`, honoured with `sip.notify_reboot_enable = 0`).
    /// Returns false if the device is not registered.
    pub async fn check_sync(&self, sip_user: &str, reboot: bool) -> bool {
        let Some(client) = self.esl.get().await else {
            return false;
        };
        let contact = format!("sofia_contact */{sip_user}@{}", crate::fsxml::SIP_DOMAIN);
        match client.api(&contact).await {
            Ok(out) if !out.trim_start().starts_with("error/") => {}
            Ok(_) => return false,
            Err(err) => {
                tracing::warn!(user = sip_user, error = %err, "sofia_contact failed");
                return false;
            }
        }
        let event = if reboot {
            "check-sync;reboot=true"
        } else {
            "check-sync;reboot=false"
        };
        let headers = [
            ("profile", "internal"),
            ("event-string", event),
            ("user", sip_user),
            ("host", crate::fsxml::SIP_DOMAIN),
            ("content-type", "application/simple-message-summary"),
        ];
        match client.sendevent("NOTIFY", &headers).await {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(user = sip_user, error = %err, "check-sync NOTIFY failed");
                false
            }
        }
    }

    /// Restarts (`reboot = false`) or resets (`reboot = true`) a Cisco
    /// Enterprise SIP phone with a SIP NOTIFY `Event: service-control`
    /// (ADR 0051). A restart reloads all configuration files. The phone
    /// accepts the NOTIFY only with the Call-ID of its current REGISTER.
    /// Returns false if no Cisco registration of the user is found.
    pub async fn cisco_service_control(&self, sip_user: &str, reboot: bool) -> bool {
        let Some(client) = self.esl.get().await else {
            return false;
        };
        let registrations = match load_registrations(&client).await {
            Ok(registrations) => registrations,
            Err(err) => {
                tracing::warn!(user = sip_user, error = %err, "show registrations failed");
                return false;
            }
        };
        let Some(call_id) = registrations
            .iter()
            .filter(|r| r.user.eq_ignore_ascii_case(sip_user))
            .filter(|r| r.user_agent.to_ascii_lowercase().contains("cisco"))
            .map(|r| r.call_id.as_str())
            .find(|id| valid_call_id(id))
        else {
            return false;
        };
        let result = if reboot {
            let body = service_control_body("reset", call_id);
            let headers = [
                ("profile", "internal"),
                ("event-string", "service-control"),
                ("user", sip_user),
                ("host", crate::fsxml::SIP_DOMAIN),
                ("call-id", call_id),
                ("content-type", "text/plain"),
            ];
            client
                .sendevent_with_body("NOTIFY", &headers, Some(&body))
                .await
        } else {
            // mod_sofia sends `action=restart` with this Call-ID to Cisco user agents.
            client
                .api(&format!("sofia profile internal check_sync {call_id}"))
                .await
                .map(|_| ())
        };
        match result {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(user = sip_user, error = %err, "Cisco service-control NOTIFY failed");
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
        match client.api("show calls as json").await {
            Ok(out) => Some(parse_calls(&out)),
            Err(err) => {
                tracing::warn!(error = %err, "show calls failed");
                None
            }
        }
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

/// The parts of the status that the notification monitor compares.
fn observed(status: &LiveStatus) -> crate::notifier::Observed {
    crate::notifier::Observed {
        connected: status.connected,
        uptime: status.uptime,
        gateways: status
            .gateways
            .iter()
            .map(|(k, g)| (k.clone(), (g.state.clone(), g.last_error.clone())))
            .collect(),
        registered: status
            .registrations
            .iter()
            .map(|r| r.user.to_lowercase())
            .collect(),
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
                bridge_uuid: f("b_uuid"),
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

/// Cisco Enterprise phones fetch their files over TFTP, so TalkOps sees them
/// only through SIP. Records each Cisco registration as phone contact (last
/// contact, IP, firmware), at most once a minute per user.
async fn record_cisco_contacts(
    db: &PgPool,
    registrations: &[Registration],
    recorded: &mut HashMap<String, std::time::Instant>,
) {
    let now = std::time::Instant::now();
    recorded.retain(|_, at| now.duration_since(*at) < Duration::from_secs(60));
    for r in registrations {
        if !r.user_agent.to_ascii_lowercase().starts_with("cisco") || r.network_ip.is_empty() {
            continue;
        }
        let user = r.user.to_lowercase();
        if recorded.contains_key(&user) {
            continue;
        }
        recorded.insert(user, now);
        let firmware = cisco_firmware(&r.user_agent);
        if let Err(err) = talkops_core::phones::record_registration(
            db,
            &r.user,
            talkops_provisioning::vendors::CISCO_ENTERPRISE_MODELS,
            &r.network_ip,
            firmware,
        )
        .await
        {
            tracing::debug!(user = r.user, error = %err, "cannot record Cisco registration");
        }
    }
}

/// Firmware version from a Cisco user agent (`Cisco-CP7861/14.2.1`).
fn cisco_firmware(user_agent: &str) -> Option<&str> {
    let version = user_agent.rsplit_once('/')?.1.trim();
    (!version.is_empty()
        && version.len() <= 32
        && version
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-_()".contains(&c)))
    .then_some(version)
}

/// Accepts a SIP Call-ID that is safe as one FreeSWITCH API argument.
fn valid_call_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.@:+".contains(&c))
}

/// Text body of a Cisco `service-control` NOTIFY. Zero version stamps make
/// the phone use the action as given.
fn service_control_body(action: &str, call_id: &str) -> String {
    format!(
        "action={action}\nRegisterCallId={{{call_id}}}\nConfigVersionStamp={{0000000000000000}}\n\
         DialplanVersionStamp={{0000000000000000}}\nSoftkeyVersionStamp={{0000000000000000}}\n"
    )
}

/// Current registrations with their user agents. The core table of `show
/// registrations` has no user agent: its `metadata` column is the directory
/// variable `registration_metadata`. mod_sofia keeps the agent per Call-ID.
async fn load_registrations(
    client: &talkops_esl::EslClient,
) -> Result<Vec<Registration>, talkops_esl::EslError> {
    let mut registrations = parse_registrations(&client.api("show registrations as json").await?);
    match client.api("sofia xmlstatus profile internal reg").await {
        Ok(xml) => {
            let agents = parse_agents(&xml);
            for r in &mut registrations {
                if let Some(agent) = agents.get(&r.call_id) {
                    r.user_agent.clone_from(agent);
                }
            }
        }
        Err(err) => tracing::debug!(error = %err, "sofia xmlstatus reg failed"),
    }
    Ok(registrations)
}

/// Call-ID → user agent from `sofia xmlstatus profile <name> reg`.
fn parse_agents(xml: &str) -> HashMap<String, String> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return HashMap::new();
    };
    doc.descendants()
        .filter(|n| n.has_tag_name("registration"))
        .filter_map(|reg| {
            let text = |name: &str| {
                reg.children()
                    .find(|c| c.has_tag_name(name))
                    .and_then(|c| c.text())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            };
            Some((text("call-id")?.to_owned(), text("agent")?.to_owned()))
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
                    call_id: s(row, "token"),
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
    fn service_control_body_and_call_id() {
        assert_eq!(
            service_control_body("reset", "00451d6b-e3270002@192.168.90.18"),
            "action=reset\nRegisterCallId={00451d6b-e3270002@192.168.90.18}\n\
             ConfigVersionStamp={0000000000000000}\nDialplanVersionStamp={0000000000000000}\n\
             SoftkeyVersionStamp={0000000000000000}\n"
        );
        assert!(valid_call_id(
            "00451d6b-e3270002-1a2b3c4d-5e6f7a8b@192.168.90.18"
        ));
        assert!(!valid_call_id(""));
        assert!(!valid_call_id("a b"));
        assert!(!valid_call_id("a\nb"));
        assert!(!valid_call_id("a'b"));
        assert_eq!(cisco_firmware("Cisco-CP7861/14.2.1"), Some("14.2.1"));
        assert_eq!(cisco_firmware("Cisco-CP7861"), None);
        assert_eq!(cisco_firmware("Cisco-CP7861/<script>"), None);
    }

    #[test]
    fn parses_user_agents_by_call_id() {
        let xml = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<profile>
  <registrations>
    <registration>
        <call-id>00451d6b-e3270002-5c2f1a7e-0d3b6f21@192.168.90.18</call-id>
        <user>999@talkops.local</user>
        <contact>&quot;999&quot; &lt;sip:999@192.168.90.18:5060;transport=tcp&gt;</contact>
        <agent>Cisco-CP7861/14.2.1</agent>
        <status>Registered(TCP)(unknown) exp(2026-10-29 21:30:00) expsecs(1800)</status>
        <network-ip>192.168.90.18</network-ip>
    </registration>
    <registration>
        <call-id>abc</call-id>
        <agent></agent>
    </registration>
  </registrations>
</profile>"#;
        let agents = parse_agents(xml);
        assert_eq!(agents.len(), 1);
        assert_eq!(
            agents["00451d6b-e3270002-5c2f1a7e-0d3b6f21@192.168.90.18"],
            "Cisco-CP7861/14.2.1"
        );
        assert!(parse_agents("Invalid Profile!").is_empty());
    }

    #[test]
    fn parses_registrations_json() {
        let json = r#"{"row_count":1,"rows":[{"reg_user":"20-1","realm":"talkops.local","expires":"1791150000","network_ip":"192.168.1.20","network_port":"5060","network_proto":"udp","metadata":"Yealink","token":"abc@192.168.1.20"}]}"#;
        let regs = parse_registrations(json);
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].user, "20-1");
        assert_eq!(regs[0].expires, 1791150000);
        assert_eq!(regs[0].call_id, "abc@192.168.1.20");
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
