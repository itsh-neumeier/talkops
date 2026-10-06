//! Yealink configuration files: the common file (`y0000000000XX.cfg`) and
//! the per-phone file (`<mac>.cfg`).

use std::sync::OnceLock;

use minijinja::{Environment, context};
use serde::{Deserialize, Serialize};

use crate::catalog::{PhoneFamily, PhoneModel};
use crate::cfg_value;

const COMMON: &str = include_str!("../templates/yealink/common.cfg");
const MAC: &str = include_str!("../templates/yealink/mac.cfg");

fn env() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);
        env.add_template("common.cfg", COMMON)
            .expect("valid template");
        env.add_template("mac.cfg", MAC).expect("valid template");
        env
    })
}

#[derive(Debug, thiserror::Error)]
#[error("template rendering failed: {0}")]
pub struct RenderError(#[from] minijinja::Error);

/// Where phones fetch configuration and report events.
#[derive(Debug, Clone, Serialize)]
pub struct Provisioning {
    /// Provisioning base URL without credentials, e.g. `http://pbx:8080/provisioning`.
    pub base_url: String,
    pub username: String,
    pub password: String,
}

impl Provisioning {
    /// Base URL with embedded credentials, for URLs the phone fetches outside
    /// the auto-provisioning mechanism (phonebook, firmware).
    pub fn authenticated_base(&self) -> String {
        match self.base_url.split_once("://") {
            Some((scheme, rest)) => format!(
                "{scheme}://{}:{}@{rest}",
                url_part(&self.username),
                url_part(&self.password)
            ),
            None => self.base_url.clone(),
        }
    }
}

/// Percent-encodes characters that are not allowed in URL userinfo.
fn url_part(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Locale {
    pub time_zone: String,
    pub time_zone_name: String,
    pub language: String,
    pub ntp_server: String,
}

impl Locale {
    /// Yealink time zone settings for an IANA zone; language `de` or `en`.
    /// Unknown zones fall back to Central European Time.
    pub fn new(iana_zone: &str, language: &str) -> Self {
        let (tz, name) = match iana_zone {
            "Europe/Vienna" => ("+1", "Austria(Vienna)"),
            "Europe/London" => ("0", "UK(London)"),
            "Europe/Paris" => ("+1", "France(Paris)"),
            "Europe/Rome" => ("+1", "Italy(Rome)"),
            "Europe/Madrid" => ("+1", "Spain(Madrid)"),
            "Europe/Amsterdam" => ("+1", "Netherlands(Amsterdam)"),
            // Switzerland is not in Yealink's list; Berlin has identical rules.
            _ => ("+1", "Germany(Berlin)"),
        };
        Self {
            time_zone: tz.into(),
            time_zone_name: name.into(),
            language: if language == "de" {
                "German"
            } else {
                "English"
            }
            .into(),
            ntp_server: "pool.ntp.org".into(),
        }
    }
}

/// Renders the common configuration shared by all phones.
pub fn render_common(
    prov: &Provisioning,
    admin_password: &str,
    locale: &Locale,
    language: &str,
) -> Result<String, RenderError> {
    let (internal_name, contacts_name) = if language == "de" {
        ("Intern", "Kontakte")
    } else {
        ("Internal", "Contacts")
    };
    let auth = prov.authenticated_base();
    Ok(env().get_template("common.cfg")?.render(context! {
        prov => context! {
            url => cfg_value(&prov.base_url),
            username => cfg_value(&prov.username),
            password => cfg_value(&prov.password),
            base_url => cfg_value(&auth),
            events_url => cfg_value(&format!("{}/events?key={}", prov.base_url, url_part(&prov.password))),
        },
        admin_password => cfg_value(admin_password),
        locale => locale,
        phonebook => context! { internal_name, contacts_name },
    })?)
}

/// A SIP account placed on the phone (account N / DECT handset N).
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub index: u16,
    pub label: String,
    pub display_name: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyType {
    /// Unused key.
    None,
    /// Line key for one of the phone's accounts.
    Line,
    /// Busy lamp field for an extension (with pickup on press while ringing).
    Blf,
    SpeedDial,
}

impl KeyType {
    /// Yealink `linekey.X.type` codes (Auto Provisioning Guide V80).
    pub fn code(self) -> u8 {
        match self {
            KeyType::None => 0,
            KeyType::SpeedDial => 13,
            KeyType::Line => 15,
            KeyType::Blf => 16,
        }
    }
}

/// A programmable key as configured in TalkOps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineKey {
    pub key: u16,
    #[serde(rename = "type")]
    pub kind: KeyType,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub label: String,
    /// Account the key uses (1-based).
    #[serde(default = "first_account")]
    pub account: u16,
}

fn first_account() -> u16 {
    1
}

#[derive(Debug, Clone)]
pub struct PhoneSetup<'a> {
    pub name: String,
    pub mac: String,
    pub model: &'a PhoneModel,
    pub accounts: Vec<Account>,
    pub keys: Vec<LineKey>,
    /// SIP server host and port the phone registers to.
    pub sip_host: String,
    pub sip_port: u16,
    pub firmware_url: Option<String>,
    pub voicemail_code: String,
}

#[derive(Serialize)]
struct RenderedKey {
    key: u16,
    type_code: u8,
    account: u16,
    value: String,
    label: String,
    pickup_value: String,
}

/// Effective key layout: configured keys win; keys 1..n default to the line
/// keys of the phone's accounts; all other keys are cleared.
pub fn effective_keys(setup: &PhoneSetup<'_>) -> Vec<LineKey> {
    let mut keys = Vec::new();
    for key in 1..=setup.model.line_keys {
        if let Some(k) = setup.keys.iter().find(|k| k.key == key) {
            keys.push(k.clone());
        } else if let Some(a) = setup.accounts.iter().find(|a| a.index == key) {
            keys.push(LineKey {
                key,
                kind: KeyType::Line,
                value: String::new(),
                label: a.label.clone(),
                account: a.index,
            });
        } else {
            keys.push(LineKey {
                key,
                kind: KeyType::None,
                value: String::new(),
                label: String::new(),
                account: 1,
            });
        }
    }
    keys
}

/// Renders the per-phone configuration (`<mac>.cfg`).
pub fn render_phone(setup: &PhoneSetup<'_>) -> Result<String, RenderError> {
    let accounts: Vec<Account> = setup
        .accounts
        .iter()
        .filter(|a| a.index >= 1 && a.index <= setup.model.accounts)
        .map(|a| Account {
            index: a.index,
            label: cfg_value(&a.label),
            display_name: cfg_value(&a.display_name),
            username: cfg_value(&a.username),
            password: cfg_value(&a.password),
        })
        .collect();
    let used: Vec<u16> = accounts.iter().map(|a| a.index).collect();
    // DECT bases with many handsets: only clear the first 10 unused slots.
    let max_clear = if setup.model.family == PhoneFamily::Dect {
        setup.model.accounts.min(10)
    } else {
        setup.model.accounts
    };
    let unused: Vec<u16> = (1..=max_clear).filter(|i| !used.contains(i)).collect();
    let keys: Vec<RenderedKey> = effective_keys(setup)
        .into_iter()
        .map(|k| {
            let value = cfg_value(&k.value);
            RenderedKey {
                key: k.key,
                type_code: k.kind.code(),
                account: k.account.clamp(1, setup.model.accounts),
                pickup_value: if k.kind == KeyType::Blf && !value.starts_with("park+") {
                    format!("**{value}")
                } else {
                    String::new()
                },
                value,
                label: cfg_value(&k.label),
            }
        })
        .collect();
    Ok(env().get_template("mac.cfg")?.render(context! {
        phone => context! { name => cfg_value(&setup.name), mac => setup.mac.clone() },
        model => setup.model,
        accounts => accounts,
        unused_accounts => unused,
        keys => keys,
        server => context! { host => cfg_value(&setup.sip_host), port => setup.sip_port },
        firmware_url => setup.firmware_url.as_deref().map(cfg_value),
        voicemail_code => cfg_value(&setup.voicemail_code),
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::PhoneCatalog;
    use std::path::Path;

    fn catalog() -> PhoneCatalog {
        PhoneCatalog::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/phones"))
            .unwrap()
    }

    /// Parses `key = value` lines into a map; asserts every line is well formed.
    fn parse(cfg: &str) -> std::collections::HashMap<String, String> {
        assert!(
            cfg.starts_with("#!version:1.0.0.1\n"),
            "header must be the first line"
        );
        cfg.lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| {
                let (k, v) = l
                    .split_once(" = ")
                    .unwrap_or_else(|| panic!("malformed line: {l:?}"));
                (k.to_owned(), v.to_owned())
            })
            .collect()
    }

    fn prov() -> Provisioning {
        Provisioning {
            base_url: "http://pbx.lan:8080/provisioning".into(),
            username: "provision".into(),
            password: "p@ss:w".into(),
        }
    }

    #[test]
    fn common_config() {
        let cfg = render_common(
            &prov(),
            "adm\nin",
            &Locale::new("Europe/Berlin", "de"),
            "de",
        )
        .unwrap();
        let m = parse(&cfg);
        assert_eq!(
            m["static.auto_provision.server.url"],
            "http://pbx.lan:8080/provisioning"
        );
        assert_eq!(m["static.security.user_password"], "admin:admin");
        assert_eq!(m["local_time.time_zone_name"], "Germany(Berlin)");
        assert_eq!(m["lang.gui"], "German");
        assert_eq!(
            m["remote_phonebook.data.1.url"],
            "http://provision:p%40ss%3Aw@pbx.lan:8080/provisioning/phonebook/internal.xml"
        );
        assert_eq!(m["remote_phonebook.data.2.name"], "Kontakte");
        assert!(m["action_url.dnd_on"].starts_with(
            "http://pbx.lan:8080/provisioning/events?key=p%40ss%3Aw&event=dnd_on&mac=$mac"
        ));
    }

    #[test]
    fn desk_phone_config() {
        let c = catalog();
        let setup = PhoneSetup {
            name: "Empfang".into(),
            mac: "805ec0123456".into(),
            model: c.get("t54w").unwrap(),
            accounts: vec![Account {
                index: 1,
                label: "20 Büro".into(),
                display_name: "Büro\r\nAnna".into(),
                username: "20-1".into(),
                password: "secret".into(),
            }],
            keys: vec![
                LineKey {
                    key: 3,
                    kind: KeyType::Blf,
                    value: "21".into(),
                    label: "Lab".into(),
                    account: 1,
                },
                LineKey {
                    key: 4,
                    kind: KeyType::SpeedDial,
                    value: "0891234".into(),
                    label: "Taxi".into(),
                    account: 1,
                },
            ],
            sip_host: "192.168.1.10".into(),
            sip_port: 5060,
            firmware_url: Some("http://u:p@pbx/provisioning/firmware/x/T54W.rom".into()),
            voicemail_code: "*97".into(),
        };
        let cfg = render_phone(&setup).unwrap();
        let m = parse(&cfg);
        assert_eq!(m["account.1.user_name"], "20-1");
        assert_eq!(m["account.1.display_name"], "BüroAnna");
        assert_eq!(m["account.1.sip_server.1.address"], "192.168.1.10");
        assert_eq!(m["account.2.enable"], "0", "unused accounts are disabled");
        assert_eq!(m["account.16.enable"], "0");
        assert_eq!(m["linekey.1.type"], "15");
        assert_eq!(m["linekey.3.type"], "16");
        assert_eq!(m["linekey.3.pickup_value"], "**21");
        assert_eq!(m["linekey.4.type"], "13");
        assert_eq!(m["linekey.27.type"], "0", "unconfigured keys are cleared");
        assert!(!m.contains_key("linekey.28.type"));
        assert_eq!(
            m["static.firmware.url"],
            "http://u:p@pbx/provisioning/firmware/x/T54W.rom"
        );
        assert!(!m.contains_key("handset.1.name"));
    }

    #[test]
    fn dect_base_config() {
        let c = catalog();
        let setup = PhoneSetup {
            name: "DECT".into(),
            mac: "805ec0abcdef".into(),
            model: c.get("w70b").unwrap(),
            accounts: (1..=2)
                .map(|i| Account {
                    index: i,
                    label: format!("2{i}"),
                    display_name: format!("H{i}"),
                    username: format!("2{i}-1"),
                    password: "pw".into(),
                })
                .collect(),
            keys: vec![],
            sip_host: "pbx".into(),
            sip_port: 5060,
            firmware_url: None,
            voicemail_code: "*97".into(),
        };
        let m = parse(&render_phone(&setup).unwrap());
        assert_eq!(m["handset.2.incoming_lines"], "2");
        assert_eq!(m["handset.2.dial_out_default_line"], "2");
        assert_eq!(m["account.3.enable"], "0");
        assert!(!m.contains_key("linekey.1.type"));
        assert!(!m.contains_key("static.firmware.url"));
    }
}
