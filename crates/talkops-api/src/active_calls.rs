//! Resolve live call identities from routing variables and the tenant directory.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use talkops_core::cdr::{Cdr, Direction};
use talkops_core::dialing::DialPlanSettings;
use talkops_core::extensions::{self, Device, Extension};
use talkops_core::geo::{GeoArea, GeoIndex};
use talkops_core::phones::{self, Contact, PhonebookSection};
use talkops_core::settings;
use talkops_core::tenant::TenantId;
use talkops_core::transcription_jobs::JobView;
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
    /// Phone book entry that supplies `name`, if any.
    pub contact_id: Option<Uuid>,
    /// More than one phone book entry has this number; see `matches`.
    pub ambiguous: bool,
    /// All phone book entries with this number when `ambiguous`, ordered by
    /// phone book (global first) and name; empty otherwise.
    pub matches: Vec<ContactMatch>,
}

/// Resolved parties of a transcription job (see `Directory::job_parties`).
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct JobParties {
    /// The direction of the call; for a voicemail derived from the caller.
    #[serde(skip)]
    pub direction: Option<Direction>,
    pub caller: Option<Party>,
    pub called: Option<Party>,
    /// Inbound call: the own number that the caller dialed.
    pub via_number: Option<String>,
}

/// One phone book entry that has the number of a call party.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct ContactMatch {
    pub contact_id: Uuid,
    pub name: String,
    /// Phone book section; none = global phone book.
    pub section_id: Option<Uuid>,
    /// Name of the section; empty for the global phone book.
    pub section_name: String,
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
    pub sections: Vec<PhonebookSection>,
    /// Phone book sections that each provisioned phone shows, by phone ID.
    pub phone_sections: HashMap<Uuid, Vec<Uuid>>,
}

impl Directory {
    /// Loads the extensions, numbers and phone books of a tenant.
    pub async fn load(state: &AppState, tenant: TenantId) -> ApiResult<Self> {
        let (settings, extensions, devices, numbers, contacts, sections, phone_list) = tokio::try_join!(
            settings::get(&state.db, tenant),
            extensions::list(&state.db, tenant),
            extensions::list_all_devices(&state.db, tenant),
            trunks::list_numbers(&state.db, tenant),
            phones::list_contacts(&state.db, tenant),
            phones::list_sections(&state.db, tenant),
            phones::list(&state.db, tenant)
        )?;
        Ok(Self {
            dial: settings.dial_plan(),
            extensions,
            devices,
            numbers,
            contacts,
            sections,
            phone_sections: phone_list
                .into_iter()
                .map(|p| (p.id, p.phonebook_sections))
                .collect(),
        })
    }

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

    pub fn external(&self, raw: &str) -> String {
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

    /// The area of an external number for the call map. The tenant dial
    /// plan normalizes the number first, so national formats resolve.
    pub fn locate<'g>(&self, geo: &'g GeoIndex, raw: &str) -> Option<&'g GeoArea> {
        geo.lookup(&self.external(raw))
    }

    pub fn party(&self, raw: &str, fallback: &str, id: Option<Uuid>) -> Party {
        if let Some(ext) = self.extension(raw, id) {
            return Party {
                name: ext.display_name.clone(),
                number: ext.number.clone(),
                extension_id: Some(ext.id),
                ..Party::default()
            };
        }
        self.external_party(raw, fallback, None)
    }

    /// Phone book sections that belong to an extension: the sections shown
    /// on its provisioned phones, and a section with its display name.
    fn own_sections(&self, extension: Uuid) -> Vec<Uuid> {
        let mut own: Vec<Uuid> = self
            .devices
            .iter()
            .filter(|d| d.extension_id == extension)
            .filter_map(|d| d.phone_id.and_then(|p| self.phone_sections.get(&p)))
            .flatten()
            .copied()
            .collect();
        if let Some(ext) = self.extensions.iter().find(|e| e.id == extension) {
            own.extend(
                self.sections
                    .iter()
                    .filter(|s| s.name.trim().eq_ignore_ascii_case(ext.display_name.trim()))
                    .map(|s| s.id),
            );
        }
        own
    }

    /// Resolves an external number against all phone books. `local` is the
    /// own extension in the call: when several entries have the number, an
    /// entry from a phone book of that extension supplies the name.
    pub fn external_party(&self, raw: &str, fallback: &str, local: Option<Uuid>) -> Party {
        let number = self.external(raw);
        let found: Vec<&Contact> = self
            .contacts
            .iter()
            .filter(|c| {
                [&c.phone_work, &c.phone_mobile, &c.phone_other]
                    .iter()
                    .any(|n| !n.is_empty() && !n.starts_with('*') && self.external(n) == number)
            })
            .collect();
        let ambiguous = found.len() > 1;
        let own = local.map(|e| self.own_sections(e)).unwrap_or_default();
        let by_name = |a: &&&Contact, b: &&&Contact| {
            a.display_name()
                .to_lowercase()
                .cmp(&b.display_name().to_lowercase())
        };
        // Several entries: prefer the own phone book, else the first name.
        let chosen = found
            .iter()
            .filter(|c| c.section_id.is_some_and(|s| own.contains(&s)))
            .min_by(by_name)
            .or_else(|| found.iter().min_by(by_name))
            .copied();
        let name = chosen.map(Contact::display_name).unwrap_or_else(|| {
            if fallback.is_empty()
                || fallback == raw
                || matches!(fallback, "Outbound Call" | "Inbound Call")
            {
                String::new()
            } else {
                fallback.to_owned()
            }
        });
        let mut matches: Vec<ContactMatch> = if ambiguous {
            found
                .iter()
                .map(|c| ContactMatch {
                    contact_id: c.id,
                    name: c.display_name(),
                    section_id: c.section_id,
                    section_name: c
                        .section_id
                        .and_then(|id| self.sections.iter().find(|s| s.id == id))
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                })
                .collect()
        } else {
            Vec::new()
        };
        matches.sort_by_cached_key(|m| {
            (
                m.section_id.is_some(),
                m.section_name.to_lowercase(),
                m.name.to_lowercase(),
            )
        });
        Party {
            name,
            number,
            extension_id: None,
            contact_id: chosen.map(|c| c.id),
            ambiguous,
            matches,
        }
    }

    /// Numbers of the phone book entries whose name contains `text`, in
    /// the form of call records (external numbers normalized).
    pub fn contact_numbers(&self, text: &str) -> Vec<String> {
        let text = text.trim().to_lowercase();
        if text.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<String> = self
            .contacts
            .iter()
            .filter(|c| c.display_name().to_lowercase().contains(&text))
            .flat_map(|c| [&c.phone_work, &c.phone_mobile, &c.phone_other])
            .filter(|n| !n.is_empty() && !n.starts_with('*'))
            .flat_map(|n| [n.clone(), self.external(n)])
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The parties of a call record: caller, called party and the own number
    /// of the call. For inbound calls, this is the dialed number. For outbound
    /// calls, this is the number that supplied the caller ID. Internal calls
    /// and records without a known number have no own number.
    pub fn record_parties(&self, c: &Cdr) -> (Party, Party, Option<String>) {
        let number = || {
            self.numbers
                .iter()
                .find(|n| Some(n.id) == c.number_id)
                .map(|n| n.e164.clone())
        };
        match c.direction {
            Direction::Inbound => {
                let answered = c.dest_extension_id.or(c.extension_id);
                let caller = self.external_party(&c.caller_number, &c.caller_name, answered);
                let called =
                    match answered.and_then(|id| self.extensions.iter().find(|e| e.id == id)) {
                        Some(ext) => Party {
                            name: ext.display_name.clone(),
                            number: ext.number.clone(),
                            extension_id: Some(ext.id),
                            ..Party::default()
                        },
                        None => self.party(&c.destination, "", None),
                    };
                (caller, called, number())
            }
            Direction::Outbound => {
                let caller = self.party(&c.caller_number, &c.caller_name, c.extension_id);
                let called = self.external_party(&c.destination, "", c.extension_id);
                (caller, called, number())
            }
            Direction::Internal => (
                self.party(&c.caller_number, &c.caller_name, c.extension_id),
                self.party(&c.destination, "", c.dest_extension_id),
                None,
            ),
        }
    }

    /// The parties of a transcription job: the parties of the recorded call
    /// (as in the call log), or the caller and the box of a voicemail. A
    /// voicemail from an extension is internal, else inbound.
    pub fn job_parties(&self, j: &JobView) -> JobParties {
        let caller_number = j.caller_number.as_deref().unwrap_or_default();
        let caller_name = j.caller_name.as_deref().unwrap_or_default();
        let destination = j.destination.as_deref().unwrap_or_default();
        if let Some(direction) = j.direction {
            let at = j.call_started_at.unwrap_or(j.created_at);
            let record = Cdr {
                id: Uuid::nil(),
                call_uuid: String::new(),
                direction,
                caller_number: caller_number.to_owned(),
                caller_name: caller_name.to_owned(),
                destination: destination.to_owned(),
                extension_id: j.extension_id,
                dest_extension_id: j.dest_extension_id,
                trunk_id: None,
                number_id: j.number_id,
                started_at: at,
                answered_at: None,
                ended_at: at,
                duration_secs: 0,
                billsec: 0,
                hangup_cause: String::new(),
                blocked_reason: None,
                recording_id: j.recording_id,
            };
            let (caller, called, via_number) = self.record_parties(&record);
            return JobParties {
                direction: Some(direction),
                caller: Some(caller),
                called: Some(called),
                via_number,
            };
        }
        if j.voicemail_id.is_none() {
            return JobParties::default();
        }
        let box_owner = j.extension_id;
        let internal = self.party(caller_number, caller_name, None);
        let (direction, caller) = if internal.extension_id.is_some() {
            (Direction::Internal, internal)
        } else {
            (
                Direction::Inbound,
                self.external_party(caller_number, caller_name, box_owner),
            )
        };
        JobParties {
            direction: Some(direction),
            caller: Some(caller),
            called: Some(self.party(destination, "", box_owner)),
            via_number: None,
        }
    }

    pub fn resolve(&self, call: ActiveCall, meta: Metadata) -> OverviewCall {
        let raw_caller = if meta.caller_number.is_empty() {
            call.caller_number.clone()
        } else {
            meta.caller_number.clone()
        };
        let raw_called = if !meta.destination.is_empty() {
            meta.destination.clone()
        } else if !call.callee_number.is_empty() {
            call.callee_number.clone()
        } else {
            call.destination.clone()
        };
        let (caller_ext, called_ext) = match meta.direction {
            Some(Direction::Inbound) => (None, meta.dest_extension.or(meta.extension)),
            _ => (meta.extension, meta.dest_extension),
        };
        let caller = if meta.direction == Some(Direction::Inbound) {
            self.external_party(&raw_caller, &call.caller_name, None)
        } else {
            self.party(&raw_caller, &call.caller_name, caller_ext)
        };
        // A bridge's caller ID can still identify the original caller. Use the
        // peer's dialed destination, then the routing target, instead.
        let peer_extension = self.extension(&meta.peer_number, None).map(|e| e.id);
        let called = self.party(
            &raw_called,
            &call.callee_name,
            peer_extension.or(called_ext),
        );
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
            resolved.called = self.external_party(&resolved.metadata.destination, "", None);
        }
        // Several phone book entries share the number: resolve the external
        // party again, so that a phone book of the own extension supplies
        // the name. Only the contact choice changes, not the number.
        if resolved.caller.ambiguous
            && let Some(own) = resolved.called.extension_id
        {
            let again = self.external_party(&raw_caller, "", Some(own));
            resolved.caller.name = again.name;
            resolved.caller.contact_id = again.contact_id;
        }
        if resolved.called.ambiguous
            && let Some(own) = resolved.caller.extension_id
        {
            let raw = if resolved.direction == Some(Direction::Outbound)
                && !resolved.metadata.destination.is_empty()
            {
                &resolved.metadata.destination
            } else {
                &raw_called
            };
            let again = self.external_party(raw, "", Some(own));
            resolved.called.name = again.name;
            resolved.called.contact_id = again.contact_id;
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
    let (settings, directory) = tokio::try_join!(
        async { Ok(settings::get(&state.db, tenant).await?) },
        Directory::load(state, tenant)
    )?;
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
            dialing_profile_id: None,
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
            sections: vec![],
            phone_sections: HashMap::new(),
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
        assert_eq!(ambiguous.name, "Family Vollmeier (Maria)");
        assert_eq!(ambiguous.matches.len(), 2);
    }

    fn shared_number() -> (Directory, Uuid, Uuid) {
        let mut dir = directory();
        let neumeier = PhonebookSection {
            id: Uuid::new_v4(),
            name: "Familie Neumeier".into(),
        };
        let zweig = PhonebookSection {
            id: Uuid::new_v4(),
            name: "Familie Zweig".into(),
        };
        let entry = |name: &str, company: &str, section: Option<Uuid>| Contact {
            id: Uuid::new_v4(),
            name: name.into(),
            company: company.into(),
            phone_work: "".into(),
            phone_mobile: "0170 123".into(),
            phone_other: "".into(),
            section_id: section,
        };
        dir.contacts = vec![
            entry("Max Mustermann", "Mustermann GmbH", Some(neumeier.id)),
            entry("Maxi", "", Some(zweig.id)),
            entry("Anna", "", None),
        ];
        dir.sections = vec![neumeier.clone(), zweig.clone()];
        (dir, neumeier.id, zweig.id)
    }

    #[test]
    fn lists_all_entries_of_a_shared_number_by_phone_book() {
        let (dir, neumeier, zweig) = shared_number();
        let party = dir.party("+49170123", "", None);
        assert!(party.ambiguous);
        // Without an own extension, the alphabetically first name is shown.
        assert_eq!(party.name, "Anna");
        assert_eq!(party.contact_id, Some(dir.contacts[2].id));
        let rows: Vec<_> = party
            .matches
            .iter()
            .map(|m| (m.section_id, m.section_name.as_str(), m.name.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (None, "", "Anna"),
                (
                    Some(neumeier),
                    "Familie Neumeier",
                    "Max Mustermann (Mustermann GmbH)"
                ),
                (Some(zweig), "Familie Zweig", "Maxi"),
            ]
        );
        let single = dir.party("+4995458589", "", None);
        assert!(single.matches.is_empty());
        assert!(single.contact_id.is_none());
    }

    #[test]
    fn prefers_the_phone_book_of_the_own_extension() {
        let (mut dir, neumeier, zweig) = shared_number();
        dir.extensions[0].display_name = "Familie Zweig".into();
        let incoming = dir.resolve(
            call(),
            Metadata {
                direction: Some(Direction::Inbound),
                caller_number: "0170123".into(),
                extension: Some(dir.extensions[0].id),
                ..Default::default()
            },
        );
        // The phone book with the extension's name supplies the name.
        assert_eq!(incoming.caller.name, "Maxi");
        assert_eq!(incoming.caller.contact_id, Some(dir.contacts[1].id));
        assert_eq!(incoming.caller.matches.len(), 3);
        // A phone of the extension that shows another phone book has the
        // same effect for an outbound call.
        dir.extensions[0].display_name = "Office".into();
        let phone = Uuid::new_v4();
        dir.phone_sections.insert(phone, vec![neumeier]);
        dir.devices.push(Device {
            id: Uuid::new_v4(),
            extension_id: dir.extensions[0].id,
            browser_user_id: None,
            name: "Desk".into(),
            kind: talkops_core::extensions::DeviceKind::Desk,
            sip_username: "desk101".into(),
            sip_password_enc: String::new(),
            phone_id: Some(phone),
            account_index: Some(1),
            phone_label: String::new(),
            phone_display_name: String::new(),
            codec_profile: None,
            video: None,
            sip_password_format: Default::default(),
            enabled: true,
        });
        let outgoing = dir.resolve(
            call(),
            Metadata {
                direction: Some(Direction::Outbound),
                caller_number: "101".into(),
                destination: "0170123".into(),
                extension: Some(dir.extensions[0].id),
                ..Default::default()
            },
        );
        assert_eq!(outgoing.called.name, "Max Mustermann (Mustermann GmbH)");
        assert_eq!(outgoing.called.number, "+49170123");
        assert!(!dir.own_sections(dir.extensions[0].id).contains(&zweig));
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

    #[test]
    fn locates_external_numbers_in_all_formats() {
        let geo = GeoIndex::from_files([(
            "sample.tsv",
            "A\tDE-951\tDE\tarea\t49.89\t10.89\t49 951\tBamberg\n\
             P\t49951\tDE-951\n\
             A\tDE-9545\tDE\tarea\t49.8\t10.7\t49 9545\tBurgebrach\n\
             P\t499545\tDE-9545\n",
        )])
        .unwrap();
        let dir = directory();
        for raw in [
            "095112345",
            "004995112345",
            "+4995112345",
            "+49 (0) 951 12345",
            "0951/12345",
        ] {
            assert_eq!(
                dir.locate(&geo, raw).map(|a| a.id.as_str()),
                Some("DE-951"),
                "{raw}"
            );
        }
        // A local number takes the area code of the tenant.
        assert_eq!(dir.locate(&geo, "8589").unwrap().name, "Burgebrach");
        for raw in ["anonymous", "", "112", "*98", "+4930123456"] {
            assert!(dir.locate(&geo, raw).is_none(), "{raw}");
        }
    }

    #[test]
    fn call_records_show_the_answering_extension_the_number_and_phone_book_names() {
        let mut dir = directory();
        let number = PhoneNumber {
            id: Uuid::new_v4(),
            trunk_id: Uuid::new_v4(),
            account_id: None,
            e164: "+4995431241".into(),
            label: String::new(),
            destination_type: talkops_core::trunks::NumberDestination::Extension,
            destination_id: None,
            extra_extensions: vec![],
            enabled: true,
        };
        dir.numbers.push(number.clone());
        let ext = dir.extensions[0].id;
        let now = Utc::now();
        let record = Cdr {
            id: Uuid::new_v4(),
            call_uuid: "x".into(),
            direction: Direction::Inbound,
            caller_number: "+4995458589".into(),
            caller_name: "095458589".into(),
            destination: "101".into(),
            extension_id: Some(ext),
            dest_extension_id: None,
            trunk_id: None,
            number_id: Some(number.id),
            started_at: now,
            answered_at: Some(now),
            ended_at: now,
            duration_secs: 1,
            billsec: 1,
            hangup_cause: String::new(),
            blocked_reason: None,
            recording_id: None,
        };
        let (caller, called, via) = dir.record_parties(&record);
        assert_eq!(caller.name, "Family Vollmeier (Maria)");
        assert_eq!(caller.number, "+4995458589");
        assert_eq!(called.number, "101");
        assert_eq!(called.name, "Family Zweig");
        assert_eq!(called.extension_id, Some(ext));
        assert_eq!(via.as_deref(), Some("+4995431241"));

        let outbound = Cdr {
            direction: Direction::Outbound,
            caller_number: "101".into(),
            caller_name: "Family Zweig".into(),
            destination: "+4995458589".into(),
            ..record
        };
        let (caller, called, via) = dir.record_parties(&outbound);
        assert_eq!(caller.extension_id, Some(ext));
        assert_eq!(called.name, "Family Vollmeier (Maria)");
        assert_eq!(via.as_deref(), Some("+4995431241"));

        // Older or unrouted outbound records have no number.
        let unknown = Cdr {
            number_id: None,
            ..outbound.clone()
        };
        assert_eq!(dir.record_parties(&unknown).2, None);
        let deleted = Cdr {
            number_id: Some(Uuid::new_v4()),
            ..outbound.clone()
        };
        assert_eq!(dir.record_parties(&deleted).2, None);

        // Internal calls never show an own number.
        let internal = Cdr {
            direction: Direction::Internal,
            destination: "101".into(),
            dest_extension_id: Some(ext),
            ..outbound
        };
        assert_eq!(dir.record_parties(&internal).2, None);

        assert_eq!(
            dir.contact_numbers("vollmeier"),
            ["+4995458589", "095458589"]
        );
        assert!(dir.contact_numbers(" ").is_empty());
    }
}
