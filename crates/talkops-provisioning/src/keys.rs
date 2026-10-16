//! Semantic key assignments and explicit vendor/model capability profiles.
use crate::{PhoneModel, yealink::KeyType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overrides {
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
    if overrides.keys.len() > 250 {
        return Err("too many keys".into());
    }
    let mut seen = BTreeSet::new();
    for key in &overrides.keys {
        let allowed = if key.group == "programmable" {
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
        if !allowed || !seen.insert((&key.group, key.key)) {
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
pub fn validate_endpoint(value: &str, provisioning: bool) -> Result<(), String> {
    let u = url::Url::parse(value).map_err(|_| "invalid endpoint URL")?;
    if !matches!(u.scheme(), "http" | "https")
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.query().is_some()
        || value.chars().any(char::is_control)
        || value.len() > 400
    {
        return Err("use an HTTP(S) URL without credentials, query or fragment".into());
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
