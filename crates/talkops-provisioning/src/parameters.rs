//! Model-specific scalar parameters. XML changes replace value ranges only.
//! The schema comes from reproducible fixtures, never from a device response.
use crate::{
    PhoneModel,
    model::{Account, PhoneSetup, Provisioning},
    vendors,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Range,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub key: String,
    pub parameter: String,
    pub description: String,
    pub artifact: String,
    pub category: String,
    pub kind: String,
    pub managed: bool,
    pub copyable: bool,
    pub default: Option<String>,
}

pub fn category(name: &str) -> &'static str {
    let name = name.to_ascii_lowercase();
    let groups: &[(&str, &[&str])] = &[
        (
            "security",
            &[
                "password",
                "passwd",
                "secret",
                "certificate",
                "tls",
                "https",
                "auth",
                "802.1x",
                "8021x",
                "vpn",
                "srtp",
                "encrypt",
                "private",
            ],
        ),
        (
            "locale",
            &["language", "locale", "country", "time", "ntp", "daylight"],
        ),
        (
            "network",
            &[
                "network",
                "vlan",
                "dhcp",
                "dns",
                "gateway",
                "ip_address",
                "ipaddr",
                "netmask",
                "ethernet",
                "wifi",
                "wireless",
                "wlan",
                "ipv6",
                "stun",
                "qos",
                "dscp",
                "802.1",
            ],
        ),
        (
            "keys",
            &[
                "key",
                "button",
                "dss",
                "blf",
                "efk",
                "speed_dial",
                "speeddial",
            ],
        ),
        (
            "directory",
            &["phonebook", "directory", "contact", "ldap", "favorite"],
        ),
        (
            "audio",
            &[
                "audio",
                "codec",
                "volume",
                "ringer",
                "ringtone",
                "tone",
                "headset",
                "handset",
                "speaker",
                "microphone",
                "echo",
                "dtmf",
                "noise",
                "acoustic",
            ],
        ),
        (
            "display",
            &[
                "display",
                "screen",
                "backlight",
                "wallpaper",
                "logo",
                "lcd",
                "brightness",
                "contrast",
                "font",
                "color",
            ],
        ),
        (
            "provisioning",
            &[
                "provision",
                "firmware",
                "upgrade",
                "update",
                "download",
                "config_server",
                "cfg_server",
            ],
        ),
        (
            "calls",
            &[
                "forward",
                "dnd",
                "waiting",
                "pickup",
                "dial",
                "transfer",
                "conference",
                "call",
                "voicemail",
                "mwi",
                "intercom",
                "paging",
                "park",
            ],
        ),
        (
            "sip",
            &[
                "sip",
                "account",
                "reg.",
                "register",
                "proxy",
                "voip",
                "line.",
                "voiceprofile",
                "rtp",
                "transport",
                "expire",
            ],
        ),
        (
            "maintenance",
            &[
                "log", "debug", "syslog", "reboot", "reset", "trace", "snmp", "web", "admin",
            ],
        ),
    ];
    groups
        .iter()
        .find(|(_, words)| words.iter().any(|word| name.contains(word)))
        .map(|(group, _)| *group)
        .unwrap_or("other")
}

pub fn url_contains_secret(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| !url.username().is_empty() || url.password().is_some())
}

struct Scalar {
    parameter: String,
    description: String,
    value: String,
    range: Range<usize>,
}
/// Fanvil H5 uses a slash in one element name. Keep wire bytes unchanged.
/// The equal-length substitution is only used for structural inspection.
pub fn inspection_xml(body: &str) -> String {
    body.replace("<Time/Date_Display>", "<Time_Date_Display>")
        .replace("</Time/Date_Display>", "</Time_Date_Display>")
}
fn fragments(body: &str) -> String {
    let mut body = body.to_string();
    if let Some(start) = body.find("<?xml") {
        if let Some(end) = body[start..].find("?>") {
            body.replace_range(start..start + end + 2, &" ".repeat(end + 2));
        }
    }
    format!("<fragments>{body}</fragments>")
}
fn scalars(body: &str, xml: bool) -> Result<Vec<Scalar>, String> {
    let mut result = vec![];
    if xml {
        let inspected = inspection_xml(body);
        let body = inspected.as_str();
        let wrapped = fragments(body);
        let (text, shift) = if roxmltree::Document::parse(body).is_ok() {
            (body, 0)
        } else {
            (wrapped.as_str(), 11)
        };
        let doc = roxmltree::Document::parse(text).map_err(|_| "invalid XML parameter document")?;
        let mut paths = HashMap::<roxmltree::NodeId, String>::new();
        let mut indexes = HashMap::new();
        for node in doc.descendants().filter(|n| n.is_element()) {
            let parent = node.parent().map(|p| p.id());
            let index = indexes
                .entry((parent, node.tag_name().name()))
                .or_insert(0usize);
            let semantic = node
                .attribute("N")
                .or_else(|| node.attribute("name"))
                .or_else(|| node.attribute("id"))
                .or_else(|| {
                    node.children()
                        .find(|n| n.has_tag_name("Name"))
                        .and_then(|n| n.text())
                });
            let segment = format!(
                "{}[{}]{}",
                node.tag_name().name(),
                index,
                semantic.map(|s| format!("({s})")).unwrap_or_default()
            );
            *index += 1;
            let node_path = parent
                .and_then(|id| paths.get(&id))
                .map(|p| format!("{p}/{segment}"))
                .unwrap_or(segment);
            paths.insert(node.id(), node_path.clone());
            let description = node
                .prev_siblings()
                .skip(1)
                .take_while(|n| !n.is_element())
                .filter(|n| n.is_comment())
                .filter_map(|n| n.text())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(str::trim)
                .filter(|s| !s.trim_matches(['#', ' ', '-']).is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            for attr in node.attributes() {
                // Namespace, access permissions and object names are syntax, not values.
                if [
                    "perm",
                    "idx",
                    "index",
                    "Name",
                    "name",
                    "type",
                    "ua",
                    "group",
                    "X_UseDefault",
                    "N",
                    "T",
                    "X_UD",
                    "id",
                ]
                .contains(&attr.name())
                {
                    continue;
                }
                let range = attr.range_value();
                if range.start < shift {
                    continue;
                }
                result.push(Scalar {
                    parameter: format!("{node_path}/@{}", attr.name()),
                    description: description.clone(),
                    value: attr.value().into(),
                    range: range.start - shift..range.end - shift,
                });
            }
            if node.children().any(|n| n.is_element())
                || ["Name", "name"].contains(&node.tag_name().name())
            {
                continue;
            }
            let range = node.range();
            let node_text = &text[range.clone()];
            let Some(open) = node_text.find('>') else {
                continue;
            };
            let Some(close) = node_text.rfind("</") else {
                continue;
            };
            if open >= close || node_text[open + 1..close].contains("<!") {
                continue;
            }
            // Object/Name + ParameterValueStruct/Name identifies OBi parameters.
            let semantic = if node.has_tag_name("Value") {
                node.parent()
                    .and_then(|p| p.children().find(|c| c.has_tag_name("Name")))
                    .and_then(|n| n.text())
            } else {
                node.attribute("name")
            };
            result.push(Scalar {
                parameter: format!(
                    "{node_path}{}",
                    semantic.map(|n| format!("/{n}")).unwrap_or_default()
                ),
                description,
                value: node.text().unwrap_or("").into(),
                range: range.start + open + 1 - shift..range.start + close - shift,
            });
        }
    } else {
        let mut offset = 0;
        let mut occurrences = BTreeMap::<String, usize>::new();
        let mut section = String::new();
        let mut comments = vec![];
        for raw in body.split_inclusive('\n') {
            let line = raw.trim_end_matches(['\r', '\n']);
            let trimmed = line.trim();
            if let Some(percent) = trimmed.strip_prefix('%') {
                if let Some((name, value)) = percent.split_once("%:") {
                    let start = line.find("%:").expect("percent separator") + 2;
                    let value = value.trim_start();
                    let left = line[start..].len() - value.len();
                    let (left, right) = if let Some(quoted) = value.strip_prefix('"') {
                        let end = quoted.find('"').ok_or("unterminated percent parameter")?;
                        (left + 1, left + 1 + end)
                    } else {
                        (
                            left,
                            left + value.split("//").next().unwrap_or("").trim_end().len(),
                        )
                    };
                    result.push(Scalar {
                        parameter: format!("%{name}%"),
                        description: line[start + right..]
                            .trim_start_matches(['"', ' ', '/'])
                            .into(),
                        value: line[start + left..start + right].into(),
                        range: offset + start + left..offset + start + right,
                    });
                    offset += raw.len();
                    continue;
                }
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                section = trimmed.into();
            }
            if trimmed.is_empty() || trimmed.starts_with(['#', ';', '<']) {
                if trimmed.starts_with(['#', ';']) {
                    comments.push(trimmed.trim_start_matches(['#', ';']).trim().to_string());
                    if comments.len() > 8 {
                        comments.remove(0);
                    }
                }
                offset += raw.len();
                continue;
            }
            let pair = if let Some(set) = trimmed.strip_prefix("SET ") {
                set.find(char::is_whitespace)
                    .map(|i| (set[..i].to_string(), line.find(set).unwrap() + i + 1))
            } else {
                line.find('=')
                    .or_else(|| line.find(':'))
                    .map(|i| (line[..i].trim().to_owned(), i + 1))
            };
            if let Some((name, start)) = pair {
                if !name.is_empty()
                    && name.len() < 200
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || " _.-[]".contains(c))
                {
                    let value = &line[start..];
                    let left = value.len() - value.trim_start().len();
                    let right = value.trim_end().len().max(left);
                    let (left, right) = if value[left..right].starts_with('"')
                        && value[left..right].ends_with('"')
                        && right - left >= 2
                    {
                        (left + 1, right - 1)
                    } else {
                        (left, right)
                    };
                    let key = format!("{section}{name}");
                    let index = occurrences.entry(key.clone()).or_default();
                    result.push(Scalar {
                        parameter: format!("{key}[{index}]"),
                        description: comments.join(" "),
                        value: value[left..right].into(),
                        range: offset + start + left..offset + start + right,
                    });
                    *index += 1;
                }
            }
            offset += raw.len();
            comments.clear();
        }
    }
    Ok(result)
}

pub fn source_profile(model: &PhoneModel) -> Option<&'static vendors::Profile> {
    vendors::profile(&model.id).or_else(|| {
        vendors::profiles()
            .iter()
            .find(|p| p.vendor == model.vendor && p.source_model.eq_ignore_ascii_case(&model.id))
    })
}

/// Fixture values are deliberately recognizable, so managed values cannot enter defaults.
pub fn schema(model: &PhoneModel) -> Result<Vec<Field>, String> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Vec<Field>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let cache_key = format!("{}@{}", model.id, model.accounts);
    if let Some(fields) = cache
        .lock()
        .map_err(|_| "parameter cache unavailable")?
        .get(&cache_key)
        .cloned()
    {
        return Ok(fields);
    }
    let Some(profile) = source_profile(model) else {
        return Ok(vec![]);
    };
    let mut native = model.clone();
    native.id = profile.id.clone();
    native.accounts = model.accounts;
    let mut setup = PhoneSetup {
        name: "TALKOPS_MANAGED_NAME".into(),
        mac: "805ec0010203".into(),
        model: &native,
        accounts: (1..=native.accounts)
            .map(|index| Account {
                index,
                label: format!("TALKOPS_MANAGED_LABEL{index}"),
                display_name: "TALKOPS_MANAGED_DISPLAY".into(),
                username: format!("TALKOPS_MANAGED_USER{index}"),
                password: "TALKOPS_MANAGED_SECRET".into(),
                codecs: Default::default(),
            })
            .collect(),
        keys: vec![],
        sip_host: "talkops-managed.invalid".into(),
        sip_port: 5060,
        firmware_url: None,
        voicemail_code: "*97".into(),
        ringtone: None,
        wallpaper: None,
        phonebooks: vec![],
        dial_now: vec![],
        settings: vec![],
        provisioning: Default::default(),
        asset_base: "https://talkops-managed.invalid/provisioning".into(),
    };
    if native.vendor == "acrobits" {
        setup
            .provisioning
            .vendor_settings
            .insert("code".into(), "TALKOPS_MANAGED_CODE".into());
    }
    let provisioning = Provisioning {
        base_url: setup.asset_base.clone(),
        username: "TALKOPS_MANAGED_AUTH".into(),
        password: "TALKOPS_MANAGED_SECRET".into(),
    };
    let common = vendors::Common {
        provisioning: &provisioning,
        admin_password: "TALKOPS_MANAGED_SECRET",
        timezone: "Europe/Berlin",
        language: "en",
        ntp_server: "pool.ntp.org",
        contacts: &[],
    };
    let files = vendors::builtin(&native)?;
    let mut fields = vec![];
    for artifact in &profile.artifacts {
        // Contact exports, scripts and app QR payloads are not setting documents.
        if !artifact.name.contains("{$mac}")
            && profile.primary.as_deref() != Some(&artifact.name)
            && !artifact.name.ends_with(".cfg")
        {
            continue;
        }
        let (body, _) = vendors::render(&setup, &common, Some(&artifact.name), &files)?;
        for scalar in scalars(&body, artifact.format.starts_with("xml"))? {
            let lower = format!("{} {}", scalar.parameter, scalar.description).to_ascii_lowercase();
            let secret = [
                "password", "passwd", "passcode", "pincode", "secret", "private", "psk", "wep_key",
                "wpa_key", "token",
            ]
            .iter()
            .any(|word| lower.contains(word))
                || lower
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|word| word == "pin")
                // Cisco dictionary scripts can embed URL credentials after `serv=`.
                || profile.revision == "talkops-cisco7800-v1"
                    && scalar.parameter.contains("/Dictionary_Server_Script[");
            // The authored MPP profile only exposes locale fields. Account,
            // registration and button bindings stay under TalkOps control.
            let mpp_managed = profile.revision == "talkops-cisco7800-v1"
                && ![
                    "Primary_NTP_Server",
                    "Time_Zone",
                    "Daylight_Saving_Time_Enable",
                    "Daylight_Saving_Time_Rule",
                    "Dictionary_Server_Script",
                    "Language_Selection",
                    "Locale",
                    "Ignore_DHCP_Time_Offset",
                ]
                .iter()
                .any(|name| scalar.parameter.contains(&format!("/{name}[")));
            let managed = mpp_managed
                || scalar.value.contains("TALKOPS_MANAGED")
                || scalar.value.contains("talkops-managed.invalid")
                || scalar.value.contains("805ec0010203")
                || [
                    "mac_address",
                    "macaddress",
                    "serial_number",
                    "serialnumber",
                    "auth.username",
                    "auth.password",
                    "userid",
                    "user_id",
                    "auth_id",
                    "authpassword",
                    "authusername",
                ]
                .iter()
                .any(|word| lower.contains(word))
                || scalar.value.contains("/app/provision")
                || scalar.value.contains("/app/contacts");
            let unique = [
                "hostname",
                "host_name",
                "static_ip",
                "ipaddress",
                "ip_address",
                "staticip",
                "ip address",
                "ipei",
                "serial",
                "uuid",
            ]
            .iter()
            .any(|word| lower.contains(word));
            fields.push(Field {
                key: format!("{}::{}", artifact.name, scalar.parameter),
                parameter: scalar.parameter.clone(),
                description: scalar.description,
                artifact: artifact.name.clone(),
                category: category(&lower).into(),
                kind: if secret { "secret" } else { "text" }.into(),
                managed,
                copyable: !managed && !secret && !unique,
                default: (!managed && !secret).then_some(scalar.value),
            });
        }
    }
    if model.id != native.id {
        fields.sort_by_key(|field| profile.primary.as_deref() != Some(&field.artifact));
        let mut seen = std::collections::BTreeSet::new();
        fields.retain(|field| seen.insert(field.parameter.clone()));
    }
    fn size(fields: &[Field]) -> usize {
        fields
            .iter()
            .map(|f| {
                f.key.len()
                    + f.parameter.len()
                    + f.description.len()
                    + f.artifact.len()
                    + f.default.as_ref().map_or(0, String::len)
                    + 128
            })
            .sum()
    }
    let bytes = size(&fields);
    if bytes <= 4_194_304 {
        let mut entries = cache.lock().map_err(|_| "parameter cache unavailable")?;
        while !entries.is_empty()
            && (entries.len() >= 16
                || entries.values().map(|f| size(f)).sum::<usize>() + bytes > 16_777_216)
        {
            entries.pop_first();
        }
        entries.insert(cache_key, fields.clone());
    }
    Ok(fields)
}

pub fn validate(model: &PhoneModel, values: &BTreeMap<String, String>) -> Result<(), String> {
    if values.is_empty() {
        return Ok(());
    }
    if values.len() > 4096 {
        return Err("too many device parameters".into());
    }
    let fields = schema(model)?;
    for (key, value) in values {
        let field = fields
            .iter()
            .find(|f| f.key == *key)
            .ok_or("unknown model parameter")?;
        if field.managed {
            return Err("parameter is managed by TalkOps".into());
        }
        let limit = if value.starts_with("v1:") { 8192 } else { 4096 };
        if value.len() > limit || value.chars().any(char::is_control) {
            return Err("invalid parameter value".into());
        }
    }
    Ok(())
}

/// Older Yealink IDs keep their independent native files. Add supported source
/// parameters when those smaller files do not already contain the parameter.
pub fn apply_yealink(
    body: &str,
    model: &PhoneModel,
    values: &BTreeMap<String, String>,
) -> Result<String, String> {
    if values.is_empty() {
        return Ok(body.into());
    }
    let schema = schema(model)?;
    let mut parameters = BTreeMap::new();
    for (key, value) in values {
        let field = schema
            .iter()
            .find(|f| f.key == *key && !f.managed)
            .ok_or("unknown Yealink parameter")?;
        let name = field
            .parameter
            .rsplit_once('[')
            .map(|(name, _)| name)
            .ok_or("invalid Yealink parameter")?;
        if !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
            || value.chars().any(char::is_control)
        {
            return Err("invalid Yealink parameter value".into());
        }
        parameters.insert(name.to_owned(), value.clone());
    }
    let mut replacements = vec![];
    let mut used = std::collections::BTreeSet::new();
    for scalar in scalars(body, false)? {
        let name = scalar
            .parameter
            .rsplit_once('[')
            .map(|(name, _)| name)
            .unwrap_or("");
        if let Some(value) = parameters.get(name) {
            replacements.push((scalar.range, value.clone()));
            used.insert(name.to_owned());
        }
    }
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut output = body.to_owned();
    for (range, value) in replacements {
        output.replace_range(range, &value);
    }
    for (name, value) in parameters {
        if !used.contains(&name) {
            output.push_str(&format!("\n{name} = {value}\n"));
        }
    }
    if output.len() > 1_048_576 {
        return Err("configuration output is too large".into());
    }
    Ok(output)
}

pub fn apply(
    body: &str,
    artifact: &str,
    xml: bool,
    values: &BTreeMap<String, String>,
) -> Result<String, String> {
    if values.is_empty() {
        return Ok(body.into());
    }
    let mut replacements = vec![];
    let mut applied = std::collections::BTreeSet::new();
    for scalar in scalars(body, xml)? {
        if let Some(value) = values.get(&format!("{artifact}::{}", scalar.parameter)) {
            applied.insert(format!("{artifact}::{}", scalar.parameter));
            // XML metacharacters and text format delimiters cannot create new settings.
            let value = if xml {
                value
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;")
                    .replace('\'', "&apos;")
            } else {
                if value.contains('"') || value.chars().any(char::is_control) {
                    return Err("text parameter contains an unsafe delimiter".into());
                }
                value.clone()
            };
            replacements.push((scalar.range, value));
        }
    }
    if values
        .keys()
        .any(|key| key.starts_with(&format!("{artifact}::")) && !applied.contains(key))
    {
        return Err("parameter is unavailable in the current template configuration".into());
    }
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut output = body.to_owned();
    for (range, value) in replacements {
        output.replace_range(range, &value);
    }
    if output.len() > 1_048_576 {
        return Err("configuration output is too large".into());
    }
    if xml && roxmltree::Document::parse(&inspection_xml(&output)).is_err() {
        roxmltree::Document::parse(&fragments(&inspection_xml(&output)))
            .map_err(|_| "invalid parameter XML")?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percent_parameters_keep_quotes_and_inline_comments() {
        let source =
            "%NETWORK_SNTP_SERVER%:\"old.example\" // server\n%INTERVAL%:3600 // seconds\n";
        let fields = scalars(source, false).unwrap();
        assert_eq!(fields[0].value, "old.example");
        assert_eq!(fields[1].value, "3600");
        let result = apply(
            source,
            "device.cfg",
            false,
            &BTreeMap::from([(
                "device.cfg::%NETWORK_SNTP_SERVER%".into(),
                "time.example".into(),
            )]),
        )
        .unwrap();
        assert!(result.contains("%NETWORK_SNTP_SERVER%:\"time.example\" // server"));
    }
    #[test]
    fn xml_and_cfg_replace_only_the_selected_value() {
        let xml = "<PHONE><a value=\"old\"/><b>old</b><b>keep</b></PHONE>";
        let fields = scalars(xml, true).unwrap();
        let key = format!("test::{}", fields[1].parameter);
        let result = apply(xml, "test", true, &BTreeMap::from([(key, "a<&\"".into())])).unwrap();
        assert!(result.contains("<b>a&lt;&amp;&quot;</b><b>keep</b>"));
        let cfg = "[sip]\r\nvalue = \"old\"\r\nvalue = keep\n";
        let key = format!("test::{}", scalars(cfg, false).unwrap()[0].parameter);
        assert_eq!(
            apply(
                cfg,
                "test",
                false,
                &BTreeMap::from([(key.clone(), "new".into())])
            )
            .unwrap(),
            "[sip]\r\nvalue = \"new\"\r\nvalue = keep\n"
        );
        assert!(
            apply(
                cfg,
                "test",
                false,
                &BTreeMap::from([(key, "x\"\npassword=y".into())])
            )
            .is_err()
        );
    }
}
