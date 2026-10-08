//! Ad-hoc conferences from the browser softphone: the current call (both
//! parties) moves into a conference room and further participants are
//! dialled into it through the normal dialplan, as if the extension had
//! called them.

use serde_json::Value;
use talkops_esl::EslClient;

use crate::fsxml::{CONTEXT_INTERNAL, SIP_DOMAIN};

/// Conference profile (bootstrap `conference.conf.xml`).
pub const PROFILE: &str = "talkops";

#[derive(Debug, thiserror::Error)]
pub enum ConferenceError {
    #[error("FreeSWITCH is not connected")]
    Unavailable,
    #[error("no active call found")]
    NoCall,
    #[error("FreeSWITCH: {0}")]
    Command(String),
}

/// Who adds a participant: the extension and the browser's SIP Call-ID of
/// the current call.
pub struct Initiator<'a> {
    pub tenant: uuid::Uuid,
    pub extension_id: uuid::Uuid,
    pub extension_number: &'a str,
    pub display_name: &'a str,
    pub call_id: &'a str,
}

/// A channel as listed by `show channels as json`.
#[derive(Debug, Clone, PartialEq)]
struct Channel {
    uuid: String,
    presence_id: String,
    application: String,
    application_data: String,
}

fn parse_channels(json: &str) -> Vec<Channel> {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let field = |r: &Value, k: &str| {
        r.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    value
        .get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|r| Channel {
                    uuid: field(r, "uuid"),
                    presence_id: field(r, "presence_id"),
                    application: field(r, "application"),
                    application_data: field(r, "application_data"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The other leg bridged to `uuid` (`show calls as json`).
fn bridged_partner(json: &str, uuid: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(json).ok()?;
    value.get("rows")?.as_array()?.iter().find_map(|r| {
        let a = r.get("uuid").and_then(Value::as_str).unwrap_or_default();
        let b = r.get("b_uuid").and_then(Value::as_str).unwrap_or_default();
        match (a == uuid, b == uuid) {
            (true, _) if !b.is_empty() => Some(b.to_owned()),
            (_, true) if !a.is_empty() => Some(a.to_owned()),
            _ => None,
        }
    })
}

/// Only digits, `*`, `#` and a leading `+` reach the dialplan.
pub fn clean_number(number: &str) -> Option<String> {
    let n: String = number
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '*' | '#' | '+'))
        .collect();
    let valid = !n.is_empty() && n.len() <= 32 && !n[1..].contains('+');
    valid.then_some(n)
}

fn room_name(uuid: &str) -> String {
    format!("talkops-{}", uuid.replace('-', ""))
}

/// Moves the initiator's call into a conference (if it is not in one yet)
/// and dials `number` into it. Returns the conference name; the new
/// participant is dialled in the background.
pub async fn add_participant(
    client: std::sync::Arc<EslClient>,
    who: &Initiator<'_>,
    number: &str,
) -> Result<String, ConferenceError> {
    let api = |cmd: String| {
        let client = client.clone();
        async move {
            client
                .api(&cmd)
                .await
                .map_err(|e| ConferenceError::Command(e.to_string()))
        }
    };
    // The browser's own channel: same extension (presence) and Call-ID.
    let presence = format!("{}@{SIP_DOMAIN}", who.extension_number);
    let channels = parse_channels(&api("show channels as json".into()).await?);
    let mut mine = None;
    for c in channels.iter().filter(|c| c.presence_id == presence) {
        let call_id = api(format!("uuid_getvar {} sip_call_id", c.uuid)).await?;
        if call_id.trim() == who.call_id {
            mine = Some(c.clone());
            break;
        }
    }
    let mine = mine.ok_or(ConferenceError::NoCall)?;

    let room = if mine.application == "conference" {
        // Already a conference: just dial the next participant.
        mine.application_data
            .split('@')
            .next()
            .unwrap_or_default()
            .to_owned()
    } else {
        let calls = api("show calls as json".into()).await?;
        bridged_partner(&calls, &mine.uuid).ok_or(ConferenceError::NoCall)?;
        let room = room_name(&mine.uuid);
        // The room closes when the initiator leaves.
        api(format!(
            "uuid_setvar {} conference_member_flags moderator|endconf",
            mine.uuid
        ))
        .await?;
        api(format!(
            "uuid_transfer {} -both conference:{room}@{PROFILE} inline",
            mine.uuid
        ))
        .await?;
        room
    };

    // Dial through the internal context as the initiator's extension, so
    // numbering, trunks and caller ID apply as for a normal call.
    let name =
        crate::fsxml::sanitize_value(who.display_name).replace([',', ':', '\'', '{', '}'], " ");
    // `talkops_conference` makes the dialplan offer the usual codecs: the
    // loopback's own codec (L16) would otherwise be all a browser or phone
    // is offered.
    let originate = format!(
        "originate {{origination_caller_id_number={ext},origination_caller_id_name='{name}',\
         talkops_tenant_id={tenant},talkops_extension_id={ext_id},talkops_conference={room},\
         loopback_export=talkops_tenant_id\\,talkops_extension_id\\,talkops_conference,\
         originate_timeout=60,ignore_early_media=true}}loopback/{number}/{CONTEXT_INTERNAL} \
         &conference({room}@{PROFILE})",
        ext = who.extension_number,
        tenant = who.tenant,
        ext_id = who.extension_id,
    );
    client
        .bgapi(&originate)
        .await
        .map_err(|e| ConferenceError::Command(e.to_string()))?;
    Ok(room)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_channels_and_partners() {
        let channels = r#"{"row_count":2,"rows":[
            {"uuid":"a-1","presence_id":"24@talkops.local","application":"bridge","application_data":"user/24-1"},
            {"uuid":"b-2","presence_id":"","application":"conference","application_data":"talkops-x@talkops"}]}"#;
        let c = parse_channels(channels);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].presence_id, "24@talkops.local");
        assert_eq!(c[1].application, "conference");
        assert!(parse_channels("0 total.").is_empty());

        let calls = r#"{"rows":[{"uuid":"a-1","b_uuid":"b-2"},{"uuid":"c-3","b_uuid":""}]}"#;
        assert_eq!(bridged_partner(calls, "a-1").as_deref(), Some("b-2"));
        assert_eq!(bridged_partner(calls, "b-2").as_deref(), Some("a-1"));
        assert_eq!(bridged_partner(calls, "c-3"), None);
    }

    #[test]
    fn cleans_numbers() {
        assert_eq!(clean_number(" 030 / 123-45 ").as_deref(), Some("03012345"));
        assert_eq!(clean_number("+4930123").as_deref(), Some("+4930123"));
        assert_eq!(clean_number("*51").as_deref(), Some("*51"));
        assert_eq!(clean_number("${x}"), None);
        assert_eq!(clean_number("12+3"), None);
        assert_eq!(room_name("ab-cd"), "talkops-abcd");
    }
}
