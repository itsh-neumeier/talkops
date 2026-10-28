//! Semantic key assignments and explicit vendor/model capability profiles.
use crate::{PhoneModel, yealink::KeyType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overrides {
    pub account_slots: Option<u16>,
    pub language: String,
    pub time_zone: String,
    pub ntp_server: String,
    pub defer_unassigned_keys: bool,
    pub vendor_settings: BTreeMap<String, String>,
    pub parameters: BTreeMap<String, String>,
    pub endpoint: String,
    pub sip_host: String,
    pub firmware_profile: String,
    pub template_id: Option<String>,
    pub keys: Vec<Key>,
    /// An omitted state keeps the factory layout. An explicit list replaces it.
    pub softkeys: BTreeMap<String, Vec<String>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_function: Option<String>,
    pub group: String,
    pub key: u16,
    #[serde(rename = "type")]
    pub kind: KeyType,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "one")]
    pub account: u16,
    #[serde(default)]
    pub history_type: u8,
    #[serde(default)]
    pub phonebook: u8,
}
fn one() -> u16 {
    1
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Capabilities {
    pub programmable: Vec<u16>,
    pub expansion_modules: u16,
    pub expansion_keys: u16,
    pub softkey_states: BTreeMap<String, Vec<String>>,
    pub programmable_functions: Vec<KeyType>,
    pub line_functions: Vec<KeyType>,
    pub firmware_profiles: Vec<String>,
}

pub fn capabilities(model: &PhoneModel) -> Capabilities {
    static PROFILES: std::sync::OnceLock<BTreeMap<String, Capabilities>> =
        std::sync::OnceLock::new();
    let profiles = PROFILES.get_or_init(|| {
        serde_json::from_str(include_str!("../profiles/yealink.json"))
            .expect("checked-in key profiles")
    });
    if model.vendor == "yealink" {
        profiles
            .get(&model.id)
            .cloned()
            .unwrap_or_else(empty_capabilities)
    } else {
        empty_capabilities()
    }
}
fn empty_capabilities() -> Capabilities {
    Capabilities {
        programmable: vec![],
        expansion_modules: 0,
        expansion_keys: 0,
        softkey_states: BTreeMap::new(),
        programmable_functions: vec![],
        line_functions: vec![
            KeyType::None,
            KeyType::Line,
            KeyType::Blf,
            KeyType::SpeedDial,
        ],
        firmware_profiles: vec![],
    }
}
pub fn requires_account(kind: KeyType) -> bool {
    matches!(
        kind,
        KeyType::Line
            | KeyType::Blf
            | KeyType::SpeedDial
            | KeyType::Pickup
            | KeyType::Voicemail
            | KeyType::Dtmf
            | KeyType::Intercom
    )
}
pub fn requires_number(kind: KeyType) -> bool {
    matches!(
        kind,
        KeyType::Blf
            | KeyType::SpeedDial
            | KeyType::Pickup
            | KeyType::Dtmf
            | KeyType::Intercom
            | KeyType::Prefix
    )
}
pub fn validate_key(
    kind: KeyType,
    value: &str,
    label: &str,
    account: u16,
    accounts: u16,
) -> Result<(), String> {
    if label.chars().count() > 32 || label.chars().any(char::is_control) {
        return Err("invalid key label".into());
    }
    let automatic = matches!(kind, KeyType::SpeedDial | KeyType::Intercom);
    if account > accounts || (requires_account(kind) && account == 0 && !automatic) {
        return Err("invalid key account".into());
    }
    if value.chars().any(char::is_control) || value.len() > 511 {
        return Err("invalid key value".into());
    }
    if requires_number(kind)
        && (value.is_empty()
            || value.len() > 32
            || !value
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '*' | '#' | '+')))
    {
        return Err("invalid key number".into());
    }
    if kind == KeyType::XmlBrowser {
        validate_endpoint(value, false)?;
    }
    Ok(())
}
pub fn validate(model: &PhoneModel, overrides: &Overrides) -> Result<(), String> {
    let effective = effective_model(model, overrides)?;
    let model = &effective;
    crate::parameters::validate(model, &overrides.parameters)?;
    validate_locale(
        &overrides.language,
        &overrides.time_zone,
        &overrides.ntp_server,
    )?;
    if let Some(profile) = crate::vendors::profile(&model.id) {
        for (key, value) in &overrides.vendor_settings {
            if key == "cisco_enterprise_tftp_ip"
                && !value.is_empty()
                && crate::vendors::cisco_tftp_ip(value).is_none()
            {
                return Err("Cisco TFTP client address must be a unicast IPv4 address".into());
            }
            if !profile.settings.iter().any(|s| s.key == *key)
                || value.len() > if value.starts_with("v1:") { 8192 } else { 4096 }
                || value.chars().any(char::is_control)
            {
                return Err("invalid vendor setting".into());
            }
            if profile
                .settings
                .iter()
                .any(|s| s.key == *key && s.kind == "list")
            {
                let items: Vec<String> = serde_json::from_str(value)
                    .map_err(|_| "list setting must be a JSON array of strings")?;
                if items.len() > 64
                    || items
                        .iter()
                        .any(|s| s.len() > 512 || s.chars().any(char::is_control))
                {
                    return Err("invalid list setting".into());
                }
            }
        }
    } else if !overrides.vendor_settings.is_empty() {
        return Err("vendor settings are unavailable for this profile".into());
    }
    if !overrides.endpoint.is_empty() {
        validate_endpoint(&overrides.endpoint, true)?;
    }
    validate_sip_host(&overrides.sip_host)?;
    let caps = capabilities(model);
    if !overrides.firmware_profile.is_empty()
        && !caps.firmware_profiles.contains(&overrides.firmware_profile)
    {
        return Err("unsupported firmware profile".into());
    }
    if overrides.keys.len() > 512 {
        return Err("too many keys".into());
    }
    let mut seen = BTreeSet::new();
    for key in &overrides.keys {
        let native = crate::vendors::profile(&model.id);
        let allowed = if let Some(profile) = native {
            profile
                .key_groups
                .contains(&key.group.replace("expansion_", "expansion-"))
                && key.key > 0
                && key.key <= 512
                && crate::vendors::key_function(&profile.vendor, key).is_some()
        } else if key.group == "programmable" {
            caps.programmable.contains(&key.key) && caps.programmable_functions.contains(&key.kind)
        } else if let Some(module) = key
            .group
            .strip_prefix("expansion_")
            .and_then(|s| s.parse::<u16>().ok())
        {
            module > 0
                && module <= caps.expansion_modules
                && key.key > 0
                && key.key <= caps.expansion_keys
                && caps.line_functions.contains(&key.kind)
        } else {
            false
        };
        if !allowed
            || native.is_none() && key.native_function.is_some()
            || !seen.insert((&key.group, key.key))
        {
            return Err("unsupported or duplicate key assignment".into());
        }
        if key.history_type != 0 || key.phonebook > 4 {
            return Err("invalid history or phonebook selection".into());
        }
        validate_key(
            key.kind,
            &key.value,
            &key.label,
            key.account,
            model.accounts,
        )?;
        if crate::parameters::url_contains_secret(&key.value) {
            return Err("key URLs must not contain credentials".into());
        }
    }
    for (state, actions) in &overrides.softkeys {
        let allowed = caps
            .softkey_states
            .get(state)
            .ok_or("unsupported call state")?;
        if actions.len() != 4 || actions.iter().any(|a| !allowed.contains(a)) {
            return Err("invalid call-state layout".into());
        }
        let unique: BTreeSet<_> = actions.iter().filter(|a| a.as_str() != "Empty").collect();
        if unique.len() != actions.iter().filter(|a| a.as_str() != "Empty").count() {
            return Err("duplicate softkey action".into());
        }
    }
    Ok(())
}
/// Source templates without an account bound do not establish a hardware limit.
/// The administrator can supply the count documented for the selected device.
pub fn effective_model(model: &PhoneModel, overrides: &Overrides) -> Result<PhoneModel, String> {
    let mut model = model.clone();
    if let Some(slots) = overrides.account_slots {
        let profile = crate::vendors::profile(&model.id)
            .ok_or("account count override is unavailable for this model")?;
        if slots == 0
            || slots > 100
            || profile.account_capacity_source != "single_account_baseline"
                && slots > profile.accounts
        {
            return Err("invalid account slot count for this source profile".into());
        }
        model.accounts = slots;
    }
    Ok(model)
}
pub fn validate_locale(language: &str, time_zone: &str, ntp_server: &str) -> Result<(), String> {
    if !["", "en", "de"].contains(&language) {
        return Err("unsupported device language".into());
    }
    if time_zone.len() > 64
        || !time_zone
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_/-+".contains(&b))
    {
        return Err("invalid device time zone".into());
    }
    if !ntp_server.is_empty()
        && (ntp_server.len() > 253
            || url::Host::parse(ntp_server).is_err()
            || ntp_server.chars().any(char::is_whitespace))
    {
        return Err("NTP server must be a host name or IP address".into());
    }
    Ok(())
}
pub fn validate_endpoint(value: &str, provisioning: bool) -> Result<(), String> {
    let u = url::Url::parse(value).map_err(|_| "invalid endpoint URL")?;
    if !matches!(u.scheme(), "http" | "https")
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || (provisioning && u.query().is_some())
        || value.chars().any(char::is_control)
        || value.len() > 400
    {
        return Err("use an HTTP(S) URL without credentials or fragment; provisioning URLs must not have a query".into());
    }
    if provisioning && u.path().trim_end_matches('/') != "/provisioning" {
        return Err("endpoint path must be /provisioning".into());
    }
    Ok(())
}
pub fn validate_sip_host(host: &str) -> Result<(), String> {
    if host.is_empty() {
        return Ok(());
    }
    if host.len() > 253
        || host
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && !matches!(c, '.' | '-' | ':' | '[' | ']'))
    {
        return Err("invalid SIP server host".into());
    }
    let u = url::Url::parse(&format!("sip://{host}")).map_err(|_| "invalid SIP server host")?;
    if u.host_str().is_none() || u.port().is_some() {
        return Err("SIP host must not include a port".into());
    }
    Ok(())
}
pub fn type_code(kind: KeyType, profile: &str) -> u8 {
    if profile == "fusionpbx-legacy" {
        match kind {
            KeyType::SwitchAccountUp => 48,
            KeyType::SwitchAccountDown => 49,
            KeyType::Directory => 29,
            KeyType::XmlPhonebook => 47,
            _ => kind.code(),
        }
    } else {
        kind.code()
    }
}
pub fn softkey_xml(state: &str, actions: &[String], allowed: &[String]) -> String {
    let root = match state {
        "call_in" => "CallIn",
        "call_failed" => "CallFailed",
        "connecting" => "Connecting",
        "dialing" => "Dialing",
        "ring_back" => "RingBack",
        _ => "Talking",
    };
    let keys = |list: Vec<&String>| {
        list.into_iter()
            .map(|s| format!("<Key Type=\"{s}\"/>\n"))
            .collect::<String>()
    };
    let enabled = keys(actions.iter().collect());
    let disabled = keys(allowed.iter().filter(|s| !actions.contains(s)).collect());
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<{root}>\n<Disable>{disabled}</Disable>\n<Enable>{enabled}</Enable>\n<Default>{enabled}</Default>\n</{root}>\n"
    )
}
