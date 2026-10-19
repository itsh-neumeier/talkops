//! Parses CDRs posted by `mod_xml_cdr`.

use std::borrow::Cow;

use chrono::{DateTime, TimeZone, Utc};
use talkops_core::cdr::{Cdr, Direction};
use talkops_core::tenant::TenantId;
use uuid::Uuid;

/// Extracts a CDR from the XML document. Returns `None` for calls TalkOps did
/// not route (no `talkops_direction` variable), e.g. rejected scanner INVITEs.
/// Parses a CDR; also returns the call's recording file (`talkops_recording`).
pub fn parse(xml: &str) -> Result<Option<(TenantId, Cdr, Option<String>)>, String> {
    let xml = without_name_prefixes(xml);
    let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
    let vars = doc
        .descendants()
        .find(|n| n.has_tag_name("variables"))
        .ok_or("no <variables> element")?;
    let var = |name: &str| -> Option<String> {
        vars.children()
            .find(|n| n.has_tag_name(name))
            .and_then(|n| n.text())
            .map(|t| percent_decode(t.trim()))
    };
    let Some(direction) = var("talkops_direction") else {
        return Ok(None);
    };
    let direction = match direction.as_str() {
        "inbound" => Direction::Inbound,
        "outbound" => Direction::Outbound,
        "internal" => Direction::Internal,
        other => return Err(format!("unknown direction {other}")),
    };
    let tenant = var("talkops_tenant_id")
        .and_then(|v| v.parse().ok())
        .map(TenantId)
        .ok_or("missing tenant")?;
    let epoch = |name: &str| -> Option<DateTime<Utc>> {
        var(name)
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .and_then(|v| Utc.timestamp_opt(v, 0).single())
    };
    let uuid_var = |name: &str| var(name).and_then(|v| v.parse::<Uuid>().ok());
    let started_at = epoch("start_epoch").ok_or("missing start_epoch")?;
    let ended_at = epoch("end_epoch").unwrap_or(started_at);
    let num = |name: &str| var(name).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    let cdr = Cdr {
        id: Uuid::nil(),
        call_uuid: var("uuid").ok_or("missing uuid")?,
        direction,
        caller_number: var("talkops_caller_number").unwrap_or_default(),
        caller_name: var("talkops_caller_name").unwrap_or_default(),
        destination: var("talkops_destination").unwrap_or_default(),
        extension_id: uuid_var("talkops_extension_id"),
        dest_extension_id: uuid_var("talkops_dest_extension_id"),
        trunk_id: uuid_var("talkops_trunk_id"),
        number_id: uuid_var("talkops_number_id"),
        started_at,
        answered_at: epoch("answer_epoch"),
        ended_at,
        duration_secs: num("duration"),
        billsec: num("billsec"),
        hangup_cause: var("hangup_cause").unwrap_or_default(),
        blocked_reason: var("talkops_blocked").filter(|reason| {
            direction == Direction::Inbound && talkops_core::blocking::valid_block_reason(reason)
        }),
        recording_id: None,
    };
    Ok(Some((tenant, cdr, var("talkops_recording"))))
}

/// mod_xml_cdr writes each channel variable as an element with the
/// variable's name. A variable from `export nolocal:name=value` has the name
/// `nolocal:name`, which XML reads as an undeclared namespace prefix: the
/// whole record would be rejected. This replaces `:` in element names
/// with `_`. Values are URL-encoded, so a `<` always starts markup.
fn without_name_prefixes(xml: &str) -> Cow<'_, str> {
    if !xml.contains(':') {
        return Cow::Borrowed(xml);
    }
    let mut out = String::with_capacity(xml.len());
    let mut in_name = false;
    for c in xml.chars() {
        match c {
            '<' => in_name = true,
            // Processing instructions and comments keep their text.
            '?' | '!' if out.ends_with('<') => in_name = false,
            ':' if in_name => {
                out.push('_');
                continue;
            }
            c if c.is_whitespace() || c == '>' => in_name = false,
            _ => {}
        }
        out.push(c);
    }
    Cow::Owned(out)
}

/// mod_xml_cdr URL-encodes variable values.
fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<cdr core-uuid="x">
  <channel_data><direction>inbound</direction></channel_data>
  <variables>
    <uuid>1f0c0a2e-1111-2222-3333-444455556666</uuid>
    <talkops_direction>outbound</talkops_direction>
    <talkops_tenant_id>00000000-0000-4000-8000-000000000001</talkops_tenant_id>
    <talkops_caller_number>20</talkops_caller_number>
    <talkops_caller_name>M%C3%BCller</talkops_caller_name>
    <talkops_destination>%2B49301234567</talkops_destination>
    <talkops_extension_id>not-a-uuid</talkops_extension_id>
    <start_epoch>1791150000</start_epoch>
    <answer_epoch>1791150005</answer_epoch>
    <end_epoch>1791150065</end_epoch>
    <duration>65</duration>
    <billsec>60</billsec>
    <hangup_cause>NORMAL_CLEARING</hangup_cause>
  </variables>
</cdr>"#;

    #[test]
    fn parses_routed_call() {
        let (tenant, cdr, recording) = parse(SAMPLE).unwrap().unwrap();
        assert!(recording.is_none());
        assert_eq!(tenant, TenantId::DEFAULT);
        assert_eq!(cdr.direction, Direction::Outbound);
        assert_eq!(cdr.caller_name, "Müller");
        assert_eq!(cdr.destination, "+49301234567");
        assert_eq!(cdr.extension_id, None);
        assert_eq!(cdr.billsec, 60);
        assert_eq!((cdr.ended_at - cdr.started_at).num_seconds(), 65);
        assert!(cdr.answered_at.is_some());
    }

    #[test]
    fn accepts_variables_with_a_prefix() {
        // `export nolocal:absolute_codec_string=…` (calls from trunks, ADR
        // 0023 and 0025) gives an element name with a colon.
        let xml = SAMPLE.replace(
            "<uuid>",
            "<nolocal:absolute_codec_string>G722%2CPCMA</nolocal:absolute_codec_string>\n    <nolocal:talkops_door_dtmf>00000000-0000-4000-8000-000000000002</nolocal:talkops_door_dtmf>\n    <uuid>",
        );
        let (_, cdr, _) = parse(&xml).unwrap().unwrap();
        assert_eq!(cdr.destination, "+49301234567");
        // Attribute values and text keep their colons.
        assert_eq!(
            without_name_prefixes(r#"<?xml version="1.0"?><a b="c:d"><e:f>g:h</e:f></a>"#),
            r#"<?xml version="1.0"?><a b="c:d"><e_f>g:h</e_f></a>"#
        );
    }

    #[test]
    fn ignores_unrouted_calls() {
        let xml = SAMPLE.replace("<talkops_direction>outbound</talkops_direction>", "");
        assert!(parse(&xml).unwrap().is_none());
        assert!(parse("<cdr/>").is_err());
    }

    #[test]
    fn block_marker_requires_an_inbound_call_and_a_known_reason() {
        for reason in ["anonymous", "list:%2B4930*", "phoneblock:12"] {
            let xml = SAMPLE
                .replace("<talkops_direction>outbound", "<talkops_direction>inbound")
                .replace(
                    "</variables>",
                    &format!("<talkops_blocked>{reason}</talkops_blocked></variables>"),
                );
            let (_, cdr, _) = parse(&xml).unwrap().unwrap();
            assert_eq!(
                cdr.blocked_reason.as_deref(),
                Some(percent_decode(reason).as_str())
            );
        }
        for (direction, reason) in [
            ("inbound", "CALL_REJECTED"),
            ("inbound", "list:101"),
            ("outbound", "phoneblock:12"),
            ("internal", "anonymous"),
        ] {
            let xml = SAMPLE
                .replace(
                    "<talkops_direction>outbound",
                    &format!("<talkops_direction>{direction}"),
                )
                .replace(
                    "</variables>",
                    &format!("<talkops_blocked>{reason}</talkops_blocked></variables>"),
                );
            assert!(parse(&xml).unwrap().unwrap().1.blocked_reason.is_none());
        }
        assert!(parse(SAMPLE).unwrap().unwrap().1.blocked_reason.is_none());
    }
}
