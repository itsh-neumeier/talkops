//! Native derivatives of the pinned FusionPBX source library.
use crate::{PhoneModel, yealink::PhoneSetup};
use md5::{Digest, Md5};
use minijinja::{AutoEscape, Environment, Error, ErrorKind, UndefinedBehavior, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, sync::OnceLock};

include!(concat!(env!("OUT_DIR"), "/native_files.rs"));
include!(concat!(env!("OUT_DIR"), "/authored_files.rs"));
include!(concat!(env!("OUT_DIR"), "/all_reference.rs"));

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Artifact {
    pub name: String,
    pub template: String,
    pub format: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Setting {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub copyable: bool,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyFunction {
    pub uuid: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub subtype: String,
    pub value: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub groups: Vec<String>,
}
pub fn function_matches_group(function: &KeyFunction, group: &str) -> bool {
    function.groups.is_empty() || function.groups.iter().any(|g| g == group)
}
pub fn key_function(vendor: &str, key: &crate::keys::Key) -> Option<&'static KeyFunction> {
    use crate::yealink::KeyType;
    let group = key.group.replace("expansion_", "expansion-");
    if let Some(id) = key.native_function.as_deref() {
        return functions(vendor)
            .iter()
            .find(|f| f.uuid == id && function_matches_group(f, &group));
    }
    let names: &[&str] = match key.kind {
        KeyType::None => &["none", "na", "disabled", "empty"],
        KeyType::Line => &["line"],
        KeyType::Blf => match vendor {
            "fanvil" | "telekonnectors" => &["blf_new"],
            "poly" | "polycom" => &["automata"],
            _ => &["blf"],
        },
        KeyType::SpeedDial => match vendor {
            "poly" | "polycom" => &["normal"],
            _ => &["speed_dial"],
        },
        _ => &[],
    };
    names.iter().find_map(|name| {
        functions(vendor)
            .iter()
            .find(|f| f.kind == *name && function_matches_group(f, &group))
    })
}
pub fn functions(vendor: &str) -> &'static [KeyFunction] {
    #[derive(Deserialize)]
    struct Catalog {
        vendors: BTreeMap<String, Vec<KeyFunction>>,
    }
    static FUNCTIONS: OnceLock<Catalog> = OnceLock::new();
    FUNCTIONS
        .get_or_init(|| {
            serde_json::from_str(include_str!("../profiles/functions.json"))
                .expect("checked function catalog")
        })
        .vendors
        .get(match vendor {
            "poly" => "polycom",
            "flyingvoice" => "yealink",
            "telekonnectors" => "fanvil",
            _ => vendor,
        })
        .map(Vec::as_slice)
        .unwrap_or_default()
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    pub vendor: String,
    pub source_model: String,
    pub revision: String,
    pub adaptation: String,
    pub device_validation: String,
    pub reason: Option<String>,
    pub accounts: u16,
    pub account_capacity_source: String,
    pub family: String,
    pub primary: Option<String>,
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub settings: Vec<Setting>,
    #[serde(default)]
    pub key_groups: Vec<String>,
    #[serde(default)]
    pub indexed_lines: bool,
    #[serde(default)]
    pub template_selector: Option<String>,
}
pub fn profiles() -> &'static [Profile] {
    static PROFILES: OnceLock<Vec<Profile>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        let mut profiles: Vec<Profile> =
            serde_json::from_str(include_str!("../profiles/fusionpbx.json"))
                .expect("checked native profiles");
        let authored: Vec<Profile> =
            serde_json::from_str(include_str!("../profiles/cisco7800.json"))
                .expect("checked authored profiles");
        for profile in authored {
            assert!(
                !profiles.iter().any(|p| p.id == profile.id),
                "duplicate profile ID"
            );
            profiles.push(profile);
        }
        profiles
    })
}
pub fn profile(id: &str) -> Option<&'static Profile> {
    profiles().iter().find(|p| p.id == id)
}
/// Model IDs of the Cisco Enterprise SIP profiles (ADR 0049).
pub const CISCO_ENTERPRISE_MODELS: &[&str] = &[
    "cisco-7811-enterprise",
    "cisco-7821-enterprise",
    "cisco-7841-enterprise",
    "cisco-7861-enterprise",
];
pub fn cisco_enterprise(id: &str) -> bool {
    CISCO_ENTERPRISE_MODELS.contains(&id)
}
/// Replaces the text of the first `<versionStamp>` element with a hash of the
/// file with an empty stamp. The phone compares stamps to detect a changed
/// configuration, so a fixed stamp can hide changes (ADR 0051).
pub fn cisco_version_stamp(body: &str) -> String {
    const OPEN: &str = "<versionStamp>";
    const CLOSE: &str = "</versionStamp>";
    let Some(start) = body.find(OPEN).map(|i| i + OPEN.len()) else {
        return body.to_owned();
    };
    let Some(end) = body[start..].find(CLOSE).map(|i| start + i) else {
        return body.to_owned();
    };
    let empty = format!("{}{}", &body[..start], &body[end..]);
    let stamp = format!("talkops-{:x}", Md5::digest(empty.as_bytes()));
    format!("{}{stamp}{}", &body[..start], &body[end..])
}
/// Largest number of entries in a Cisco ring list.
pub const CISCO_RING_LIMIT: usize = 50;
/// File name of a TalkOps ringtone on a Cisco Enterprise phone (at most 25
/// characters; the TFTP map accepts only this form).
pub fn cisco_ring_filename(media: &str) -> String {
    let prefix: String = media
        .chars()
        .filter(char::is_ascii_hexdigit)
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("talkops-{prefix:0>8}.rwb")
}
/// `Ringlist-wb.xml` for Cisco 7800 phones: (display name, file name) pairs.
/// Names are cut to 25 characters; at most [`CISCO_RING_LIMIT`] entries.
pub fn cisco_ringlist(rings: &[(String, String)]) -> String {
    let esc = |s: &str| {
        s.chars()
            .filter(|c| !c.is_control())
            .take(25)
            .collect::<String>()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut out = String::from("<CiscoIPPhoneRingList>\n");
    for (name, file) in rings.iter().take(CISCO_RING_LIMIT) {
        out.push_str(&format!(
            "  <Ring><DisplayName>{}</DisplayName><FileName>{}</FileName></Ring>\n",
            esc(name),
            esc(file)
        ));
    }
    out.push_str("</CiscoIPPhoneRingList>\n");
    out
}
pub fn cisco_tftp_ip(value: &str) -> Option<std::net::Ipv4Addr> {
    let ip = value.parse::<std::net::Ipv4Addr>().ok()?;
    (!ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast()).then_some(ip)
}
pub fn sources() -> &'static [(&'static str, &'static [u8])] {
    ALL_REFERENCE
}
pub fn source(path: &str) -> Option<&'static [u8]> {
    ALL_REFERENCE
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, bytes)| *bytes)
}
pub fn native_source(path: &str) -> Option<&'static str> {
    NATIVE_FILES
        .iter()
        .chain(AUTHORED_FILES.iter())
        .find(|(name, _)| *name == path)
        .map(|(_, text)| *text)
}
pub fn builtin(model: &PhoneModel) -> Result<BTreeMap<String, String>, String> {
    let p = profile(&model.id).ok_or("unsupported native model")?;
    if p.adaptation != "adapted" {
        return Err("source profile has no usable device configuration".into());
    }
    p.artifacts
        .iter()
        .map(|a| {
            Ok((
                a.name.clone(),
                native_source(&a.template)
                    .ok_or("missing native artifact")?
                    .to_owned(),
            ))
        })
        .collect()
}
pub fn filename(pattern: &str, mac: &str) -> String {
    pattern
        .replace("{$mac|upper}", &mac.to_ascii_uppercase())
        .replace("{$mac|lower}", &mac.to_ascii_lowercase())
        .replace("{$mac}", mac)
        .replace("{mac}", mac)
        .replace("{$address}", mac)
}
pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        })
}
pub fn artifact<'a>(profile: &'a Profile, requested: &str, mac: &str) -> Option<&'a Artifact> {
    if !valid_path(requested) {
        return None;
    }
    profile.artifacts.iter().find(|a| {
        filename(&a.name, mac) == requested
            || filename(&a.name, &mac.to_ascii_uppercase()) == requested
            || profile.vendor == "grandstream"
                && profile.primary.as_deref() == Some(&a.name)
                && [format!("cfg{mac}.xml"), "cfg.xml".into()]
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(requested))
    })
}
pub fn delivery_filename(profile: &Profile, artifact: &Artifact, mac: &str) -> String {
    if profile.vendor == "grandstream"
        && profile.primary.as_deref() == Some(&artifact.name)
        && artifact.format.starts_with("xml")
    {
        format!("cfg{mac}.xml")
    } else if profile.vendor == "cisco"
        && (artifact.name.starts_with("SIP") || artifact.name.starts_with("SEP"))
    {
        filename(&artifact.name, &mac.to_ascii_uppercase())
    } else {
        filename(&artifact.name, mac)
    }
}
fn values(value: Value) -> Result<Vec<Value>, Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(vec![]);
    }
    let map = value.kind() == minijinja::value::ValueKind::Map;
    value
        .try_iter()?
        .map(|item| if map { value.get_item(&item) } else { Ok(item) })
        .collect()
}
fn put(map: Value, key: Value, value: Value) -> Result<Value, Error> {
    let mut entries = BTreeMap::<String, Value>::new();
    if map.kind() == minijinja::value::ValueKind::Map {
        for k in map.try_iter()? {
            entries.insert(k.to_string(), map.get_item(&k)?);
        }
    }
    if entries.len() >= 512 {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "mapping exceeds 512 entries",
        ));
    }
    entries.insert(key.to_string(), value);
    Ok(Value::from_iter(entries))
}
pub fn environment(
    files: &BTreeMap<String, String>,
    xml: bool,
) -> Result<Environment<'static>, String> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_auto_escape_callback(move |_| {
        if xml {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    env.set_fuel(Some(500_000));
    env.set_recursion_limit(20);
    env.add_function("put", put);
    env.add_function("ksort", |value: Value| value);
    env.add_function("is_numeric", |value: Value| {
        value.to_string().parse::<f64>().is_ok_and(f64::is_finite)
    });
    env.add_function("ip_address", |value: Value| {
        value.to_string().parse::<std::net::IpAddr>().is_ok()
    });
    env.add_function("php_bool", |value: Value| {
        matches!(
            value.to_string().trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "on" | "yes"
        )
    });
    env.add_function(
        "lookup",
        |mut value: Value, path: Vec<Value>| -> Result<Value, Error> {
            for key in path {
                if value.is_none() || value.is_undefined() {
                    return Ok(Value::UNDEFINED);
                }
                let key = if value.kind() == minijinja::value::ValueKind::Seq {
                    key.to_string()
                        .parse::<u64>()
                        .map(Value::from)
                        .unwrap_or(key)
                } else {
                    Value::from(key.to_string())
                };
                value = value.get_item(&key)?;
            }
            Ok(value)
        },
    );
    env.add_function(
        "substring",
        |value: String, start: i64, len: Option<usize>| {
            let chars = value.chars().collect::<Vec<_>>();
            let from = if start < 0 {
                chars.len().saturating_sub(start.unsigned_abs() as usize)
            } else {
                start as usize
            };
            chars
                .into_iter()
                .skip(from)
                .take(len.unwrap_or(usize::MAX))
                .collect::<String>()
        },
    );
    env.add_function(
        "array_merge",
        |left: Value, right: Value| -> Result<Value, Error> {
            let merged = values(left)?
                .into_iter()
                .chain(values(right)?)
                .collect::<Vec<_>>();
            if merged.len() > 1024 {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "merged array exceeds 1024 entries",
                ));
            }
            Ok(Value::from_iter(merged))
        },
    );
    env.add_filter("truncate", |value: String, length: Option<usize>| {
        let length = length.unwrap_or(80).min(4096);
        if value.chars().count() <= length {
            return value;
        }
        let suffix = "...".chars().take(length).collect::<String>();
        let mut prefix = value
            .chars()
            .take(length.saturating_sub(suffix.len()))
            .collect::<String>();
        if let Some(boundary) = prefix.rfind(char::is_whitespace)
            && boundary > 0
        {
            prefix.truncate(boundary);
        }
        format!("{prefix}{suffix}")
    });
    env.add_filter("values", values);
    env.add_filter("int", |value: Value| {
        value
            .to_string()
            .parse::<i64>()
            .unwrap_or_else(|_| value.to_string().parse::<f64>().unwrap_or(0.0) as i64)
    });
    env.add_filter("items", |value: Value| -> Result<Value, Error> {
        if value.is_none() || value.is_undefined() {
            return Ok(Value::from_iter(Vec::<Value>::new()));
        }
        let entries: Vec<(Value, Value)> = if value.kind() == minijinja::value::ValueKind::Map {
            value
                .try_iter()?
                .map(|key| Ok((key.clone(), value.get_item(&key)?)))
                .collect::<Result<_, Error>>()?
        } else {
            values(value)?
                .into_iter()
                .enumerate()
                .map(|(i, row)| (Value::from(i), row))
                .collect()
        };
        Ok(Value::from_iter(
            entries
                .into_iter()
                .map(|(key, value)| Value::from_iter([key, value])),
        ))
    });
    env.add_filter("md5", |value: String| {
        format!("{:x}", Md5::digest(value.as_bytes()))
    });
    env.add_filter("json", |value: Value| {
        serde_json::to_string(&value)
            .map_err(|_| Error::new(ErrorKind::InvalidOperation, "invalid JSON value"))
    });
    env.add_filter("output", |value: Value| {
        if value.is_none() || value.is_undefined() {
            String::new()
        } else {
            value
                .to_string()
                .chars()
                .filter(|c| !c.is_control())
                .take(4096)
                .collect()
        }
    });
    for (name, text) in files {
        if text.len() > 1_048_576 {
            return Err("template source exceeds 1 MiB".into());
        }
        env.add_template_owned(name.clone(), text.clone())
            .map_err(|e| e.to_string())?;
    }
    Ok(env)
}

/// User locale from a Cisco locale package that the administrator uploaded.
/// TalkOps does not ship Cisco locale files.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CiscoLocale {
    /// Cisco name, such as `German_Germany`.
    pub name: String,
    /// Language code, such as `de_DE`.
    pub lang_code: String,
    /// Package version, such as `14.3(1.9900)`.
    pub version: String,
}

impl CiscoLocale {
    /// True if all values are safe for the SEP file and TFTP paths.
    pub fn is_valid(&self) -> bool {
        let name = !self.name.is_empty()
            && self.name.len() <= 64
            && self
                .name
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '_');
        let code = self.lang_code.len() == 5
            && self.lang_code.as_bytes()[2] == b'_'
            && self.lang_code[..2].chars().all(|c| c.is_ascii_lowercase())
            && self.lang_code[3..].chars().all(|c| c.is_ascii_uppercase());
        let version = !self.version.is_empty()
            && self.version.len() <= 32
            && self
                .version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "().-".contains(c));
        name && code && version
    }
}

/// Picks the installed user locale for a TalkOps language such as `de`.
/// `de_DE` wins over other `de_*` locales; ties sort by name.
pub fn select_cisco_locale<'a>(
    locales: &'a [CiscoLocale],
    language: &str,
) -> Option<&'a CiscoLocale> {
    let language = language.split(['-', '_']).next().unwrap_or_default();
    let preferred = format!("{language}_{}", language.to_ascii_uppercase());
    locales
        .iter()
        .filter(|l| l.lang_code.split('_').next() == Some(language))
        .min_by_key(|l| (l.lang_code != preferred, l.name.clone()))
}

pub struct Common<'a> {
    pub provisioning: &'a crate::yealink::Provisioning,
    pub admin_password: &'a str,
    pub timezone: &'a str,
    pub language: &'a str,
    pub ntp_server: &'a str,
    pub contacts: &'a [crate::phonebook::Entry],
    /// Cisco Enterprise `directoryURL`; empty omits the element.
    pub directory_url: &'a str,
    /// Cisco Enterprise `userLocale`; `None` keeps the installed language.
    pub cisco_locale: Option<&'a CiscoLocale>,
}
pub fn context(setup: &PhoneSetup<'_>, common: &Common<'_>) -> Value {
    let base = format!(
        "{}/devices/{}",
        common.provisioning.base_url.trim_end_matches('/'),
        setup.mac
    );
    let authenticated = format!(
        "{}/devices/{}",
        common
            .provisioning
            .authenticated_base()
            .trim_end_matches('/'),
        setup.mac
    );
    let mut accounts = serde_json::Map::new();
    let mut lines = Vec::new();
    // Always clear unused MPP slots, including slots above a reduced account count.
    let capacity = profile(&setup.model.id)
        .filter(|p| p.revision == "talkops-cisco7800-v1")
        .map(|p| p.accounts)
        .unwrap_or(setup.model.accounts);
    for index in 1..=capacity {
        let a = setup.accounts.iter().find(|a| a.index == index);
        let row = json!({"line_number":index,"enabled":a.is_some(),"user_id":a.map(|a|a.username.as_str()),
            "auth_id":a.map(|a|a.username.as_str()),"password":a.map(|a|a.password.as_str()),"user_password":a.map(|a|a.password.as_str()),
            "display_name":a.map(|a|a.display_name.as_str()).unwrap_or(""),"label":a.map(|a|a.label.as_str()).unwrap_or(""),
            "server_address":setup.sip_host,"server_address_primary":setup.sip_host,"sip_port":setup.sip_port,
            "sip_transport":"udp","register_expires":120,"shared_line":"private","voicemail_number":setup.voicemail_code,
            "outbound_proxy_primary":null,"outbound_proxy_secondary":null,"server_address_secondary":null});
        accounts.insert(index.to_string(), row.clone());
        lines.push(row);
    }
    let contacts=common.contacts.iter().enumerate().flat_map(|(entry_index,entry)|entry.numbers.iter().map(move |(label,number)| {
        let internal=label=="Extension";
        let kind=match label.to_ascii_lowercase().as_str(){"mobile"|"cell"=>"mobile","home"=>"home",_=>"work"};
        json!({
            "contact_name_given":entry.name,"contact_name_family":"","contact_organization":"",
            "effective_caller_id_name":entry.name,"contact_category":"TalkOps","contact_uuid":format!("entry-{entry_index}"),
            "phone_number_work":if kind=="mobile"{""}else{number},"phone_number_mobile":if kind=="mobile"{number}else{""},
            "contact_type":if setup.model.id=="grandstream-gxp2100"{"user"}else{"contact"},
            "contact_work":if kind=="work"{number}else{""},"contact_cell":if kind=="mobile"{number}else{""},"contact_home":if kind=="home"{number}else{""},"phone_number":number,
            "phone_extension":number,"phone_type":kind,"phone_label":kind,"phone_primary":1,
            "numbers":[{"phone_number":number,"phone_extension":number,"phone_label":kind,"phone_primary":1}],
            "category":if internal{"extensions"}else{"users"},"group_description":"TalkOps",
            "name":entry.name,"number":number,
        })
    })).collect::<Vec<_>>();
    let primary = profile(&setup.model.id)
        .and_then(|p| {
            p.artifacts
                .iter()
                .find(|a| Some(&a.name) == p.primary.as_ref())
                .map(|a| delivery_filename(p, a, &setup.mac))
        })
        .unwrap_or_default();
    let primary_url = format!("{authenticated}/{primary}");
    let endpoint = url::Url::parse(&base).ok();
    let mut ctx = json!({"mac":setup.mac,"address":setup.mac,"device_address":setup.mac,"device_label":setup.name,"label":setup.name,
        "domain_name":setup.sip_host,"server_address":setup.sip_host,"project_path":"",
        "http_auth_username":common.provisioning.username,"http_auth_password":common.provisioning.password,
        "admin_name":"admin","admin_password":common.admin_password,"user_password":common.admin_password,
        "voicemail_number":setup.voicemail_code,"ntp_server_primary":common.ntp_server,"ntp_server_secondary":common.ntp_server,
        "lines":lines,"account":accounts,"contacts":contacts,"contact_groups":{},
        "keys":{"line":[],"memory":[],"programmable":[],"expansion":[],"expansion-1":[],"expansion-2":[],"expansion-3":[]},
        "smarty":{"get":{"contacts":if setup.model.vendor=="zoiper"{"extensions"}else{"all"}}},"provision_url":base,"artifact_base":authenticated,
        "primary_url":primary_url,"provision_path":endpoint.as_ref().map(|u|u.path()),"provision_host":endpoint.as_ref().and_then(|u|u.host_str()),
        "groundwire_link":primary_url.replacen("https://","provlinkbs://",1).replacen("http://","provlink://",1),
        "cisco_mpp_zone":match common.timezone {"UTC"|"Etc/UTC"=>"UTC","Europe/London"=>"UK",_=>"CET"},
        "cisco_mpp_german":common.language=="de",
        "cisco_mpp_buttons":setup.model.id!="cisco-7811-mpp",
        // Escaped here and emitted with `safe`: HTML autoescape would also encode `/`.
        "cisco_directory_url":common.directory_url.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"),
        "cisco_user_locale":common.cisco_locale.filter(|l|l.is_valid()),
        "cisco_enterprise_timezone":match common.timezone {"UTC"|"Etc/UTC"=>"Greenwich Standard Time","Europe/London"=>"GMT Standard/Daylight Time",_=>"W. Europe Standard/Daylight Time"},
    });
    ctx["user"] = ctx["account"].clone();
    ctx["web_contacts"] = json!({"contacts":common.contacts.iter().enumerate().map(|(index,entry)|json!({
        "contactId":format!("contact-{index}"),"displayName":entry.name,
        "contactEntries":entry.numbers.iter().enumerate().map(|(number_index,(label,number))|json!({
            "entryId":format!("contact-{index}-{number_index}"),"type":"tel","uri":format!("tel:{number}"),"label":label,
        })).collect::<Vec<_>>()
    })).collect::<Vec<_>>()});
    ctx["template"] = json!(
        profile(&setup.model.id)
            .and_then(|p| p.template_selector.clone())
            .unwrap_or_else(|| format!(
                "{}/{}",
                setup.model.vendor,
                profile(&setup.model.id)
                    .map(|p| p.source_model.as_str())
                    .unwrap_or(&setup.model.id)
            ))
    );
    if profile(&setup.model.id).is_some_and(|p| p.indexed_lines) {
        ctx["lines"] = ctx["account"].clone();
    }
    for (index, row) in ctx["account"].as_object().expect("accounts").clone() {
        for field in [
            "user_id",
            "auth_id",
            "password",
            "user_password",
            "server_address",
            "sip_port",
            "display_name",
            "label",
            "register_expires",
            "sip_transport",
        ] {
            ctx[format!("{field}_{index}")] = row[field].clone();
        }
    }
    let defaults = json!({
        "firmware_version":"","dns_server_primary":"","dns_server_secondary":"","stun_server":"",
        "daylight_savings_start_month":3,"daylight_savings_stop_month":10,"daylight_savings_start_day":25,
        "daylight_savings_stop_day":25,"daylight_savings_start_weekday":0,"daylight_savings_stop_weekday":0,
        "daylight_savings_start_time":2,"daylight_savings_stop_time":3,
        "polycom_emergency_number":["112","110"],"snom_time_zone":common.timezone,"algo_timezone":common.timezone,
        "snom_ntp_server":common.ntp_server,"snom_language":if common.language=="de"{"Deutsch"}else{"English"},
        "snom_admin_mode_password":common.admin_password,"snom_provision_url":format!("{base}/{}",profile(&setup.model.id).and_then(|p|p.primary.as_deref()).map(|p|filename(p,&setup.mac)).unwrap_or_default()),
        "yeastar_provision_url":base,"polycom_provision_url":base,"grandstream_config_server_path":base,
        "grandstream_firmware_path":setup.firmware_url,"fanvil_time_zone":crate::locale::standard_offset(common.timezone)/900,
        "fanvil_enable_dst":u8::from(crate::locale::posix(common.timezone).is_some_and(|rule|rule.contains(','))),
        "sangoma_transport":0,"sangoma_dns_mode":0,"htek_transport":0,"htek_dns_mode":0,
        "polycom_feature_key_sync":1,"polycom_cert_validation":1,"polycom_provision_cert_validate":1,
    });
    ctx.as_object_mut()
        .expect("context object")
        .extend(defaults.as_object().expect("defaults object").clone());
    let locale = crate::yealink::Locale::new(common.timezone, common.language);
    let german = common.language == "de";
    let locale_defaults = json!({
        "aastra_gmt_offset":-crate::locale::standard_offset(common.timezone)/60,
        "yealink_language_gui":locale.language,"yealink_language_web":locale.language,
        "yealink_time_zone":locale.time_zone,"yealink_time_zone_name":locale.time_zone_name,
        "digium_time_zone":common.timezone,"digium_ntp_server":common.ntp_server,
        "polycom_olson_timezone_id":common.timezone,"polycom_display_language":if german{"German_Germany"}else{"English_United_States"},
        "grandstream_display_language":common.language,"grandstream_gxp_time_zone":crate::locale::posix(common.timezone),
        "grandstream_time_zone":common.timezone,"snom_time_zone":if setup.model.id=="snom-c520"{Some(common.timezone)}else{match common.timezone{"Europe/Berlin"=>Some("GER+1"),"Europe/Vienna"=>Some("AUT+1"),"Europe/Zurich"=>Some("CHE+1"),"Europe/London"=>Some("GBR-0"),"Europe/Paris"=>Some("FRA+1"),"Europe/Rome"=>Some("ITA+1"),"Europe/Madrid"=>Some("ESP+1"),"Europe/Amsterdam"=>Some("NLD+1"),"Asia/Kolkata"=>Some("IND+5.5"),_=>None}},
        "snom_language":if german{"Deutsch"}else{"English"},
    });
    ctx.as_object_mut().expect("context").extend(
        locale_defaults
            .as_object()
            .expect("locale defaults")
            .clone(),
    );
    for (name, value) in &setup.settings {
        ctx[name] = json!(value);
    }
    for (name, value) in &setup.provisioning.vendor_settings {
        ctx[name] = if profile(&setup.model.id).is_some_and(|p| {
            p.settings
                .iter()
                .any(|s| s.key == *name && s.kind == "list")
        }) {
            serde_json::from_str(value).unwrap_or(json!([]))
        } else {
            json!(value)
        };
    }
    for key in &setup.provisioning.keys {
        let group = key.group.replace("expansion_", "expansion-");
        let Some(function) = key_function(&setup.model.vendor, key) else {
            continue;
        };
        let kind = function.value.as_str();
        let value = if setup.model.vendor == "snom"
            && (key.kind == crate::yealink::KeyType::Blf
                || key.native_function.as_deref().is_some_and(|id| {
                    functions("snom")
                        .iter()
                        .any(|f| f.uuid == id && f.kind == "blf")
                })) {
            format!("sip:{}@{}", key.value, setup.sip_host)
        } else {
            key.value.clone()
        };
        if !ctx["keys"][&group].is_object() {
            ctx["keys"][&group] = json!({});
        }
        ctx["keys"][&group][key.key.to_string()] = json!({"device_key_id":key.key,"device_key_category":group,"device_key_type":kind,"device_key_value":value,"device_key_label":key.label,"device_key_line":key.account});
    }
    // App export templates use the first assigned account outside a loop.
    if let Some(account) = setup.accounts.first() {
        for (key, value) in [
            ("user_id", &account.username),
            ("auth_id", &account.username),
            ("password", &account.password),
            ("display_name", &account.display_name),
        ] {
            ctx[key] = json!(value);
        }
        ctx["sip_port"] = json!(setup.sip_port);
        ctx["sip_transport"] = json!(0);
        ctx["outbound_proxy"] = json!(setup.sip_host);
    }
    ctx["variables"] = ctx.clone();
    Value::from_serialize(ctx)
}

pub fn render(
    setup: &PhoneSetup<'_>,
    common: &Common<'_>,
    requested: Option<&str>,
    files: &BTreeMap<String, String>,
) -> Result<(String, String), String> {
    let profile = profile(&setup.model.id).ok_or("unsupported native profile")?;
    if cisco_enterprise(&setup.model.id)
        && (setup.model.accounts != 1
            || setup.accounts.len() > 1
            || setup.accounts.iter().any(|a| a.index != 1))
    {
        return Err("Cisco Enterprise supports one independent SIP account in slot 1".into());
    }
    if profile.revision == "talkops-cisco7800-v1" || cisco_enterprise(&setup.model.id) {
        if setup.model.accounts == 0
            || setup.model.accounts > profile.accounts
            || setup
                .accounts
                .iter()
                .any(|a| a.index == 0 || a.index > setup.model.accounts)
            || setup.accounts.iter().enumerate().any(|(i, a)| {
                setup.accounts[..i]
                    .iter()
                    .any(|other| other.index == a.index)
            })
        {
            return Err("invalid Cisco MPP account assignment".into());
        }
        if !matches!(
            common.timezone,
            "UTC"
                | "Etc/UTC"
                | "Europe/Berlin"
                | "Europe/Vienna"
                | "Europe/Zurich"
                | "Europe/Paris"
                | "Europe/Rome"
                | "Europe/Madrid"
                | "Europe/Amsterdam"
                | "Europe/London"
        ) {
            return Err(
                "Cisco MPP time zone mapping supports UTC and the documented European zones only"
                    .into(),
            );
        }
    }
    if profile.family == "app" && setup.accounts.is_empty() {
        return Err("assign a SIP account before exporting this app profile".into());
    }
    if profile.vendor == "acrobits"
        && setup
            .provisioning
            .vendor_settings
            .get("code")
            .is_none_or(|code| code.is_empty())
    {
        return Err("enter the Cloud Softphone code before exporting this app profile".into());
    }
    if profile.vendor == "sipnetic"
        && setup.accounts.iter().any(|a| {
            [
                a.username.as_str(),
                a.password.as_str(),
                a.display_name.as_str(),
                a.label.as_str(),
            ]
            .iter()
            .any(|s| s.contains([';', '\n', '\r']))
        })
    {
        return Err(
            "Sipnetic export cannot represent semicolons or line breaks in account values".into(),
        );
    }
    let name = requested
        .or(profile.primary.as_deref())
        .ok_or("profile has no primary artifact")?;
    let a = profile
        .artifacts
        .iter()
        .find(|a| a.name == name)
        .ok_or("unknown profile artifact")?;
    let owned = if files.is_empty() {
        builtin(setup.model)?
    } else {
        files.clone()
    };
    validate_manifest(setup.model, &owned)?;
    let env = environment(&owned, a.format.starts_with("xml"))?;
    let template = env.get_template(&a.name).map_err(|e| e.to_string())?;
    struct Limited(Vec<u8>);
    impl std::io::Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len() + bytes.len() > 1_048_576 {
                return Err(std::io::Error::other("artifact exceeds 1 MiB"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut out = Limited(Vec::new());
    template
        .render_captured_to(context(setup, common), &mut out)
        .map_err(|e| e.to_string())?;
    let body = String::from_utf8(out.0).map_err(|e| e.to_string())?;
    let body = if a.format.starts_with("xml") {
        body.trim_start().to_owned()
    } else {
        body
    };
    if a.format == "xml_fragment" {
        let fragment = body
            .lines()
            .filter(|line| !line.trim_start().starts_with("<?xml"))
            .collect::<Vec<_>>()
            .join("\n");
        roxmltree::Document::parse(&format!("<fragments>{fragment}</fragments>"))
            .map_err(|e| format!("invalid XML fragments {}: {e}", a.name))?;
    } else if a.format.starts_with("xml") {
        roxmltree::Document::parse(&crate::parameters::inspection_xml(&body))
            .map_err(|e| format!("invalid XML artifact {}: {e}", a.name))?;
        if cisco_enterprise(&setup.model.id) && profile.primary.as_deref() == Some(&a.name) {
            let doc = roxmltree::Document::parse(&body).map_err(|_| "invalid Enterprise XML")?;
            if doc
                .descendants()
                .filter(|n| n.has_tag_name("dialTemplate"))
                .count()
                != 1
                || !doc
                    .descendants()
                    .any(|n| n.has_tag_name("dialTemplate") && n.text() == Some("dialplan.xml"))
                || doc.descendants().any(|n| {
                    matches!(
                        n.tag_name().name(),
                        "loadInformation"
                            | "loadServer"
                            | "alternateLoadServer"
                            | "phonePersonalization"
                    )
                })
            {
                return Err("Enterprise profile must reference its declared dialplan and cannot select firmware".into());
            }
            return Ok((
                cisco_version_stamp(&body),
                "application/xml; charset=utf-8".into(),
            ));
        }
    } else if a.format == "json" {
        serde_json::from_str::<serde_json::Value>(&body).map_err(|_| "invalid JSON artifact")?;
    }
    Ok((
        body,
        if a.format.starts_with("xml") {
            "application/xml; charset=utf-8"
        } else if a.format == "csv" {
            "text/csv; charset=utf-8"
        } else if a.format == "json" {
            "application/json; charset=utf-8"
        } else {
            "text/plain; charset=utf-8"
        }
        .into(),
    ))
}
pub fn validate_manifest(
    model: &PhoneModel,
    files: &BTreeMap<String, String>,
) -> Result<(), String> {
    let p = profile(&model.id).ok_or("unknown native profile")?;
    if files.len() != p.artifacts.len() || p.artifacts.iter().any(|a| !files.contains_key(&a.name))
    {
        return Err("template files must match the model artifact manifest".into());
    }
    if files.values().map(String::len).sum::<usize>() > 4_194_304 {
        return Err("template bundle exceeds 4 MiB".into());
    }
    Ok(())
}
pub fn validate_for_model(
    files: &BTreeMap<String, String>,
    model: &PhoneModel,
) -> Result<(), String> {
    validate_manifest(model, files)?;
    for count in [0, 1, model.accounts] {
        if count == 0 && model.family == crate::PhoneFamily::App {
            continue;
        }
        let accounts = (1..=count)
            .map(|index| crate::model::Account {
                index,
                label: format!("Line {index}"),
                display_name: "Example & Office".into(),
                username: format!("phone{index}"),
                password: format!("secret-{index:03}-end"),
                codecs: Default::default(),
            })
            .collect();
        let mut setup = PhoneSetup {
            name: "Test & Office".into(),
            mac: "805ec0010203".into(),
            model,
            accounts,
            keys: vec![],
            sip_host: "pbx.invalid".into(),
            sip_port: 5060,
            firmware_url: None,
            voicemail_code: "*97".into(),
            ringtone: None,
            wallpaper: None,
            phonebooks: vec![],
            dial_now: vec![],
            settings: vec![],
            provisioning: Default::default(),
            asset_base: "https://pbx.invalid/provisioning".into(),
        };
        let provisioning = crate::model::Provisioning {
            base_url: setup.asset_base.clone(),
            username: "REDACTED".into(),
            password: "REDACTED".into(),
        };
        if model.vendor == "acrobits" {
            setup
                .provisioning
                .vendor_settings
                .insert("code".into(), "fixture-code".into());
        }
        let contacts = [
            crate::phonebook::Entry {
                name: "Office & Entry".into(),
                numbers: vec![
                    ("Work".into(), "**101".into()),
                    ("Mobile".into(), "**103".into()),
                ],
            },
            crate::phonebook::Entry {
                name: "Internal & Entry".into(),
                numbers: vec![("Extension".into(), "**102".into())],
            },
        ];
        let common = Common {
            provisioning: &provisioning,
            admin_password: "REDACTED",
            timezone: "Europe/Berlin",
            language: "en",
            ntp_server: "pool.ntp.org",
            contacts: &contacts,
            directory_url: "",
            cisco_locale: None,
        };
        if count > 0 {
            setup.provisioning.keys = profile(&model.id)
                .ok_or("unknown profile")?
                .key_groups
                .iter()
                .map(|group| crate::keys::Key {
                    native_function: None,
                    group: group.clone(),
                    key: 1,
                    kind: crate::yealink::KeyType::Blf,
                    value: "101".into(),
                    label: "Office & Entry".into(),
                    account: 1,
                    history_type: 0,
                    phonebook: 0,
                })
                .collect();
        }
        let mut rendered = String::new();
        let mut has_contact_export = false;
        for artifact in &profile(&model.id).ok_or("unknown profile")?.artifacts {
            let body = render(&setup, &common, Some(&artifact.name), files)
                .map_err(|e| format!("{} / {} / {count} accounts: {e}", model.id, artifact.name))?
                .0;
            has_contact_export |= files
                .get(&artifact.name)
                .is_some_and(|source| source.contains("(contacts | default("));
            // Decode XML entities before checking URLs. HTML escaping also
            // encodes slashes; scanning the wire representation misses them.
            let inspected = crate::parameters::inspection_xml(&body);
            let wrapped = format!(
                "<fragments>{}</fragments>",
                inspected
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("<?xml"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            let references = if artifact.format.starts_with("xml") {
                let document = roxmltree::Document::parse(&wrapped).map_err(|e| e.to_string())?;
                document
                    .descendants()
                    .flat_map(|node| {
                        let text = if node.is_text() { node.text() } else { None };
                        text.into_iter()
                            .chain(node.attributes().map(|a| a.value()))
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                body.lines()
                    .filter(|line| {
                        !line.trim_start().starts_with(['#', ';'])
                            && !line.trim_start().starts_with("//")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            for base in [
                common.provisioning.base_url.clone(),
                common.provisioning.authenticated_base(),
            ] {
                let prefix = format!("{}/devices/{}/", base.trim_end_matches('/'), setup.mac);
                for (_, tail) in references
                    .match_indices(&prefix)
                    .map(|(index, _)| (index, &references[index + prefix.len()..]))
                {
                    let end = tail
                        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''))
                        .unwrap_or(tail.len());
                    let path = tail[..end].split(['?', '#']).next().unwrap_or("");
                    if !path.is_empty()
                        && self::artifact(
                            profile(&model.id).ok_or("unknown profile")?,
                            path,
                            &setup.mac,
                        )
                        .is_none()
                    {
                        return Err(format!(
                            "{} / {} references undeclared device file: {path}",
                            model.id, artifact.name
                        ));
                    }
                    if tail.starts_with("?file=") || tail.starts_with("index.php?") {
                        return Err(format!(
                            "{} / {} retains a FusionPBX PHP query",
                            model.id, artifact.name
                        ));
                    }
                }
            }
            rendered.push_str(&body);
        }
        if has_contact_export && !rendered.contains("**101") && !rendered.contains("**102") {
            return Err(format!("{} omits its contact fixtures", model.id));
        }
        if model.id == "grandstream-gxv3240"
            && !["**101", "**102", "**103"]
                .iter()
                .all(|number| rendered.contains(number))
        {
            return Err(format!("{} loses grouped contact numbers", model.id));
        }
        for account in &setup.accounts {
            let ha1 = format!(
                "{:x}",
                Md5::digest(format!(
                    "{}:{}:{}",
                    account.username, setup.sip_host, account.password
                ))
            );
            let password_present = rendered.contains(&account.password)
                || model.vendor == "linphone" && rendered.contains(&ha1);
            if !rendered.contains(&account.username) || !password_present {
                return Err(format!(
                    "{}: account {} is not represented in its configuration",
                    model.id, account.index
                ));
            }
        }
    }
    Ok(())
}
