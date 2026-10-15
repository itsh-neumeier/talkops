//! Resolve live call identities from routing variables and the tenant directory.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use talkops_core::cdr::Direction;
use talkops_core::dialing::DialPlanSettings;
use talkops_core::extensions::{self, Device, Extension};
use talkops_core::phones::{self, Contact};
use talkops_core::settings;
use talkops_core::tenant::TenantId;
use talkops_core::trunks::{self, PhoneNumber};
use talkops_esl::Frame;
use uuid::Uuid;

use crate::AppState;
use crate::error::ApiResult;
use crate::telephony::ActiveCall;

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct Party {
    pub name: String,
    pub number: String,
    pub extension_id: Option<Uuid>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct OverviewCall {
    #[serde(flatten)]
    pub call: ActiveCall,
    pub direction: Option<Direction>,
    pub caller: Party,
    pub called: Party,
    pub answered_at: Option<DateTime<Utc>>,
    pub transcript_available: bool,
    #[serde(skip)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub tenant: Option<TenantId>,
    pub direction: Option<Direction>,
    pub extension: Option<Uuid>,
    pub dest_extension: Option<Uuid>,
    pub number: Option<Uuid>,
    pub caller_number: String,
    pub destination: String,
    pub channel_destination: String,
    pub peer_number: String,
    pub recording: String,
    pub answered_at: Option<DateTime<Utc>>,
}

impl Metadata {
    pub fn parse(dump: &str) -> Self {
        let frame = Frame {
            headers: Default::default(),
            body: dump.as_bytes().to_vec(),
        };
        let Ok(event) = frame.to_event() else {
            return Self::default();
        };
        let var = |key: &str| {
            event
                .headers
                .get(&format!("variable_{key}"))
                .unwrap_or_default()
        };
        let id = |key| Uuid::parse_str(var(key)).ok();
        Self {
            tenant: id("talkops_tenant_id").map(TenantId),
            direction: match var("talkops_direction") {
                "inbound" => Some(Direction::Inbound),
                "outbound" => Some(Direction::Outbound),
                "internal" => Some(Direction::Internal),
                _ => None,
            },
            extension: id("talkops_extension_id"),
            dest_extension: id("talkops_dest_extension_id"),
            number: id("talkops_number_id"),
            caller_number: var("talkops_caller_number").to_owned(),
            destination: var("talkops_destination").to_owned(),
            channel_destination: if var("dialed_user").is_empty() {
                event
                    .headers
                    .get("Caller-Destination-Number")
                    .unwrap_or_else(|| var("destination_number"))
                    .to_owned()
            } else {
                var("dialed_user").to_owned()
            },
            peer_number: String::new(),
            recording: var("talkops_recording").to_owned(),
            answered_at: var("answer_epoch")
                .parse::<i64>()
                .ok()
                .filter(|t| *t > 0)
                .and_then(|t| DateTime::from_timestamp(t, 0))
                .or_else(|| {
                    event
                        .headers
                        .get("Caller-Channel-Answered-Time")
                        .and_then(|s| s.parse::<i64>().ok())
                        .filter(|t| *t > 0)
                        .and_then(DateTime::from_timestamp_micros)
                }),
        }
    }
}

pub struct Directory {
    pub dial: DialPlanSettings,
    pub extensions: Vec<Extension>,
    pub devices: Vec<Device>,
    pub numbers: Vec<PhoneNumber>,
    pub contacts: Vec<Contact>,
}

impl Directory {
    fn extension(&self, raw: &str, id: Option<Uuid>) -> Option<&Extension> {
        let number = raw
            .strip_prefix("**")
            .or_else(|| raw.strip_prefix('*'))
            .unwrap_or(raw);
        if let Some(id) = id {
            return self.extensions.iter().find(|e| e.id == id);
        }
        // Caller IDs and dialed numbers are extension numbers. A SIP username
        // is free (`6578` on extension 8000) and can look like another
        // extension's number, so it is only the fallback (bridged legs).
        if let Some(ext) = self.extensions.iter().find(|e| e.number == number) {
            return Some(ext);
        }
        let device = self
            .devices
            .iter()
            .find(|d| d.sip_username.eq_ignore_ascii_case(raw))?;
        self.extensions.iter().find(|e| e.id == device.extension_id)
    }

    fn external(&self, raw: &str) -> String {
        // Short numbers and feature codes are not external directory identities.
        if raw.starts_with('*')
            || raw.starts_with('#')
            || self.dial.emergency_numbers.iter().any(|n| n == raw)
            || (raw.len() <= 6 && raw.starts_with("11"))
        {
            return raw.to_owned();
        }
        let without_trunk_zero = if raw.starts_with('+') {
            raw.replacen("(0)", "", 1)
        } else {
            raw.to_owned()
        };
        let compact: String = without_trunk_zero
            .chars()
            .filter(|c| !matches!(c, ' ' | '\u{a0}' | '-' | '/' | '.' | '(' | ')'))
            .collect();
        self.dial
            .normalize_incoming(&compact)
            .unwrap_or_else(|| raw.to_owned())
    }

    pub fn party(&self, raw: &str, fallback: &str, id: Option<Uuid>) -> Party {
        if let Some(ext) = self.extension(raw, id) {
            return Party {
                name: ext.display_name.clone(),
                number: ext.number.clone(),
                extension_id: Some(ext.id),
                ambiguous: false,
            };
        }
        self.external_party(raw, fallback)
    }

    fn external_party(&self, raw: &str, fallback: &str) -> Party {
        let number = self.external(raw);
        let mut matches = self.contacts.iter().filter(|c| {
            [&c.phone_work, &c.phone_mobile, &c.phone_other]
                .iter()
                .any(|n| !n.is_empty() && !n.starts_with('*') && self.external(n) == number)
        });
        let first = matches.next();
        let ambiguous = matches.next().is_some();
        // An ambiguous number must not be attributed to one arbitrary person.
        let name = if ambiguous {
            String::new()
        } else {
            first.map(Contact::display_name).unwrap_or_else(|| {
                if fallback.is_empty()
                    || fallback == raw
                    || matches!(fallback, "Outbound Call" | "Inbound Call")
                {
                    String::new()
                } else {
                    fallback.to_owned()
                }
            })
        };
        Party {
            name,
            number,
            extension_id: None,
            ambiguous,
        }
    }

    pub fn resolve(&self, call: ActiveCall, meta: Metadata) -> OverviewCall {
        let raw_caller = if meta.caller_number.is_empty() {
            &call.caller_number
        } else {
            &meta.caller_number
        };
        let raw_called = if !meta.destination.is_empty() {
            &meta.destination
        } else if !call.callee_number.is_empty() {
            &call.callee_number
        } else {
            &call.destination
        };
        let (caller_ext, called_ext) = match meta.direction {
            Some(Direction::Inbound) => (None, meta.dest_extension.or(meta.extension)),
            _ => (meta.extension, meta.dest_extension),
        };
        let caller = if meta.direction == Some(Direction::Inbound) {
            self.external_party(raw_caller, &call.caller_name)
        } else {
            self.party(raw_caller, &call.caller_name, caller_ext)
        };
        // A bridge's caller ID can still identify the original caller. Use the
        // peer's dialed destination, then the routing target, instead.
        let peer_extension = self.extension(&meta.peer_number, None).map(|e| e.id);
        let called = self.party(raw_called, &call.callee_name, peer_extension.or(called_ext));
        let mut resolved = OverviewCall {
            call,
            direction: meta.direction,
            caller,
            called,
            answered_at: meta.answered_at,
            transcript_available: false,
            metadata: meta,
        };
        if let Some(number) = self
            .numbers
            .iter()
            .find(|n| Some(n.id) == resolved.metadata.number)
        {
            match resolved.direction {
                Some(Direction::Outbound) => resolved.caller.number = number.e164.clone(),
                Some(Direction::Inbound) => resolved.called.number = number.e164.clone(),
                _ => (),
            }
        }
        // Outbound bridge CID can describe the local device, rather than the target.
        if resolved.direction == Some(Direction::Outbound)
            && !resolved.metadata.destination.is_empty()
            && peer_extension.is_none()
        {
            resolved.called = self.external_party(&resolved.metadata.destination, "");
        }
        resolved
    }
}

pub async fn list(state: &AppState, tenant: TenantId) -> ApiResult<Vec<OverviewCall>> {
    load(state, tenant, None).await
}

pub async fn load(
    state: &AppState,
    tenant: TenantId,
    only: Option<Uuid>,
) -> ApiResult<Vec<OverviewCall>> {
    let Some(client) = state.telephony.esl.get().await else {
        return Ok(Vec::new());
    };
    let raw = state.telephony.active_calls().await.unwrap_or_default();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let (settings, extensions, devices, numbers, contacts) = tokio::try_join!(
        settings::get(&state.db, tenant),
        extensions::list(&state.db, tenant),
        extensions::list_all_devices(&state.db, tenant),
        trunks::list_numbers(&state.db, tenant),
        phones::list_contacts(&state.db, tenant)
    )?;
    let directory = Directory {
        dial: settings.dial_plan(),
        extensions,
        devices,
        numbers,
        contacts,
    };
    let mut dumps: HashMap<String, Metadata> = HashMap::new();
    let mut result = Vec::new();
    for mut call in raw {
        if only.is_some_and(|id| call.uuid != id.to_string() && call.bridge_uuid != id.to_string())
        {
            continue;
        }
        if Uuid::parse_str(&call.uuid).is_err() {
            continue;
        }
        let mut meta = Metadata::default();
        let channels = [call.uuid.clone(), call.bridge_uuid.clone()];
        for uuid in &channels {
            if Uuid::parse_str(uuid).is_err() {
                continue;
            }
            let dump = if let Some(dump) = dumps.get(uuid) {
                dump.clone()
            } else {
                let dump = Metadata::parse(
                    &client
                        .api(&format!("uuid_dump {uuid}"))
                        .await
                        .unwrap_or_default(),
                );
                dumps.insert(uuid.to_owned(), dump.clone());
                dump
            };
            if dump.tenant == Some(tenant) && dump.direction.is_some() {
                meta = dump;
                // The routing leg is also the recording identity.
                call.uuid = uuid.to_owned();
                break;
            }
        }
        if meta.tenant != Some(tenant) {
            continue;
        }
        if let Some(peer) = channels
            .iter()
            .find(|id| **id != call.uuid && Uuid::parse_str(id).is_ok())
        {
            let dump = if let Some(dump) = dumps.get(peer) {
                dump.clone()
            } else {
                let dump = Metadata::parse(
                    &client
                        .api(&format!("uuid_dump {peer}"))
                        .await
                        .unwrap_or_default(),
                );
                dumps.insert(peer.clone(), dump.clone());
                dump
            };
            if dump.tenant == Some(tenant) {
                meta.peer_number = dump.channel_destination;
            }
        }
        let mut resolved = directory.resolve(call, meta);
        resolved.transcript_available = settings.transcription_enabled
            && !resolved.metadata.recording.is_empty()
            && resolved.answered_at.is_some()
            && matches!(resolved.call.state.as_str(), "talking" | "held");
        if !result
            .iter()
            .any(|c: &OverviewCall| c.call.uuid == resolved.call.uuid)
        {
            result.push(resolved)
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> Directory {
        let ext = Extension {
            id: Uuid::new_v4(),
            number: "101".into(),
            display_name: "Family Zweig".into(),
            user_id: None,
            outbound_number_id: None,
            hide_caller_id: false,
            ring_timeout_secs: 30,
            enabled: true,
            dnd: false,
            forward_all: None,
            record_calls: "inherit".into(),
            video_enabled: false,
        };
        Directory {
            dial: DialPlanSettings {
                area_code: "9545".into(),
                ..Default::default()
            },
            extensions: vec![ext],
            devices: vec![],
            numbers: vec![],
            contacts: vec![Contact {
                id: Uuid::new_v4(),
                name: "Family Vollmeier (Maria)".into(),
                company: "".into(),
                phone_work: "095458589".into(),
                phone_mobile: "".into(),
                phone_other: "".into(),
                section_id: Some(Uuid::new_v4()),
            }],
        }
    }

    fn call() -> ActiveCall {
        ActiveCall {
            uuid: Uuid::new_v4().to_string(),
            bridge_uuid: Uuid::new_v4().to_string(),
            caller_number: "095458589".into(),
            caller_name: "".into(),
            destination: "101-1".into(),
            callee_number: "101".into(),
            callee_name: "Outbound Call".into(),
            state: "talking".into(),
            started_at: None,
        }
    }

    #[test]
    fn matches_all_fields_formats_and_keeps_internal_numbers_separate() {
        let mut dir = directory();
        dir.contacts[0].name = "Family Vollmeier".into();
        dir.contacts[0].company = "Maria".into();
        for raw in [
            "095458589",
            "+4995458589",
            "004995458589",
            "4995458589",
            "+49 (9545) 8589",
            "8589",
        ] {
            assert_eq!(
                dir.party(raw, "Outbound Call", None).name,
                "Family Vollmeier (Maria)",
                "{raw}"
            );
        }
        dir.contacts[0].phone_mobile = std::mem::take(&mut dir.contacts[0].phone_work);
        assert!(!dir.party("+4995458589", "", None).name.is_empty());
        dir.contacts[0].phone_other = std::mem::take(&mut dir.contacts[0].phone_mobile);
        assert!(!dir.party("095458589", "", None).name.is_empty());
        for raw in ["101", "*101", "**101"] {
            assert_eq!(dir.party(raw, "", None).number, "101");
        }
        assert_ne!(
            dir.party("+499545101", "", None).extension_id,
            Some(dir.extensions[0].id)
        );
        assert_eq!(dir.party("112", "", None).number, "112");
        assert_eq!(dir.party("*97", "", None).number, "*97");
        assert_eq!(dir.party("anonymous", "", None).number, "anonymous");
        let duplicate = dir.contacts[0].clone();
        dir.contacts.push(duplicate);
        let ambiguous = dir.party("+4995458589", "Caller supplied name", None);
        assert!(ambiguous.ambiguous);
        assert!(ambiguous.name.is_empty());
    }

    #[test]
    fn extension_numbers_come_before_device_usernames() {
        let mut dir = directory();
        let door = Extension {
            id: Uuid::new_v4(),
            number: "8000".into(),
            display_name: "Front door".into(),
            ..dir.extensions[0].clone()
        };
        dir.devices.push(Device {
            id: Uuid::new_v4(),
            extension_id: door.id,
            browser_user_id: None,
            name: "VTO".into(),
            kind: talkops_core::extensions::DeviceKind::Door,
            sip_username: "101".into(),
            sip_password_enc: String::new(),
            phone_id: None,
            account_index: None,
            phone_label: String::new(),
            phone_display_name: String::new(),
            codec_profile: None,
            video: None,
            sip_password_format: Default::default(),
            enabled: true,
        });
        // The door extension is first, so a device match cannot hide behind
        // the list order.
        dir.extensions.insert(0, door.clone());
        let family = dir.extensions[1].id;
        // `101` is extension 101, although the VTO's SIP username is also `101`.
        assert_eq!(dir.party("101", "", None).extension_id, Some(family));
        assert_eq!(dir.party("8000", "", None).extension_id, Some(door.id));
        dir.devices[0].sip_username = "6578".into();
        assert_eq!(dir.party("6578", "", None).extension_id, Some(door.id));
        assert_eq!(
            dir.party("6578", "", Some(family)).extension_id,
            Some(family)
        );
    }

    #[test]
    fn resolves_routing_direction_despite_reversed_bridge_caller_id() {
        let mut dir = directory();
        let number_id = Uuid::new_v4();
        dir.numbers.push(PhoneNumber {
            id: number_id,
            trunk_id: Uuid::new_v4(),
            account_id: None,
            e164: "+4995431241".into(),
            label: "".into(),
            destination_type: trunks::NumberDestination::Extension,
            destination_id: Some(dir.extensions[0].id),
            extra_extensions: vec![],
            enabled: true,
        });
        let base = Metadata {
            number: Some(number_id),
            extension: Some(dir.extensions[0].id),
            ..Default::default()
        };
        let outgoing = dir.resolve(
            call(),
            Metadata {
                direction: Some(Direction::Outbound),
                caller_number: "101".into(),
                destination: "095458589".into(),
                ..base.clone()
            },
        );
        assert_eq!(outgoing.caller.name, "Family Zweig");
        assert_eq!(outgoing.caller.number, "+4995431241");
        assert_eq!(outgoing.called.name, "Family Vollmeier (Maria)");
        assert_eq!(outgoing.called.number, "+4995458589");
        let incoming = dir.resolve(
            call(),
            Metadata {
                direction: Some(Direction::Inbound),
                caller_number: "095458589".into(),
                ..base
            },
        );
        assert_eq!(incoming.called.name, "Family Zweig");
        assert_eq!(incoming.called.number, "+4995431241");
        assert_eq!(incoming.caller.name, "Family Vollmeier (Maria)");
    }

    #[test]
    fn parses_encoded_routing_variables_and_live_answer_time() {
        let id = Uuid::new_v4();
        let meta = Metadata::parse(&format!(
            "variable_talkops_tenant_id: {id}\nvariable_talkops_direction: outbound\nvariable_talkops_destination: %2B4995458589\nCaller-Channel-Answered-Time: 1791450000123456\n\n"
        ));
        assert_eq!(meta.tenant, Some(TenantId(id)));
        assert_eq!(meta.direction, Some(Direction::Outbound));
        assert_eq!(meta.destination, "+4995458589");
        assert_eq!(
            meta.answered_at.unwrap().timestamp_micros(),
            1791450000123456
        );
        assert!(Metadata::parse("-ERR absent").tenant.is_none());
    }

    #[test]
    fn bridge_caller_id_does_not_replace_the_target_extension() {
        let mut dir = directory();
        let mut target = dir.extensions[0].clone();
        target.id = Uuid::new_v4();
        target.number = "202".into();
        target.display_name = "Target".into();
        let target_id = target.id;
        dir.extensions.push(target);
        let meta = Metadata {
            direction: Some(Direction::Internal),
            extension: Some(dir.extensions[0].id),
            dest_extension: Some(target_id),
            caller_number: "101".into(),
            destination: "202".into(),
            ..Default::default()
        };
        let resolved = dir.resolve(call(), meta.clone());
        assert_eq!(resolved.called.extension_id, Some(target_id));
        assert_eq!(resolved.called.name, "Target");
        let mut transferred = dir.extensions[0].clone();
        transferred.id = Uuid::new_v4();
        transferred.number = "303".into();
        transferred.display_name = "Transferred target".into();
        let transferred_id = transferred.id;
        dir.extensions.push(transferred);
        let resolved = dir.resolve(
            call(),
            Metadata {
                peer_number: "303".into(),
                ..meta
            },
        );
        assert_eq!(resolved.called.extension_id, Some(transferred_id));
        assert_eq!(
            Metadata::parse("Caller-Destination-Number: 303\n\n").channel_destination,
            "303"
        );
    }
}
