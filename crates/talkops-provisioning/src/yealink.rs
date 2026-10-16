//! Yealink configuration files: the common file (`y0000000000XX.cfg`) and
//! the per-phone file (`<mac>.cfg`).

use std::sync::OnceLock;

use minijinja::{Environment, context};
use serde::{Deserialize, Serialize};

use crate::catalog::{PhoneFamily, PhoneModel};
use crate::cfg_value;

pub const COMMON: &str = include_str!("../templates/yealink/common.cfg");
pub const MAC: &str = include_str!("../templates/yealink/mac.cfg");

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

fn render_bounded(
    template: minijinja::Template<'_, '_>,
    context: minijinja::Value,
) -> Result<String, RenderError> {
    struct Limited(Vec<u8>);
    impl std::io::Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len() + bytes.len() > 262144 {
                return Err(std::io::Error::other("configuration exceeds 256 KiB"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Limited(Vec::new());
    template.render_captured_to(context, &mut writer)?;
    Ok(String::from_utf8(writer.0).expect("MiniJinja emits UTF-8"))
}

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
    render_common_with(
        prov,
        admin_password,
        locale,
        language,
        &std::collections::BTreeMap::new(),
    )
}
pub fn render_common_with(
    prov: &Provisioning,
    admin_password: &str,
    locale: &Locale,
    language: &str,
    templates: &std::collections::BTreeMap<String, String>,
) -> Result<String, RenderError> {
    let (internal_name, contacts_name) = if language == "de" {
        ("Intern", "Kontakte")
    } else {
        ("Internal", "Contacts")
    };
    let auth = prov.authenticated_base();
    let custom_env = if templates.is_empty() {
        None
    } else {
        Some(crate::library::template_env(templates)?)
    };
    let renderer = custom_env.as_ref().unwrap_or_else(|| env());
    render_bounded(
        renderer.get_template("common.cfg")?,
        context! {
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
        },
    )
}

/// A SIP account placed on the phone (account N / DECT handset N).
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub index: u16,
    pub label: String,
    pub display_name: String,
    pub username: String,
    pub password: String,
    pub codecs: AccountCodecs,
}

/// Audio codecs of a provisioned account.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountCodecs {
    /// Leave the codec settings of the phone unchanged.
    #[default]
    PhoneDefault,
    /// The codecs of the TalkOps codec profiles (ADR 0023), see
    /// [`PROFILE_CODECS`]. TalkOps selects the codec of each call.
    Profiles,
}

/// `account.X.codec.<payload>` settings for [`AccountCodecs::Profiles`] as
/// (payload, priority); priority 1 is the first choice, 0 disables the codec.
///
/// Payload names: Yealink AX83H Administrator's Manual, "Audio Codecs
/// Configuration" (`g722`, `pcmu`, `pcma`, `g729`, `g726_*`, `ilbc`) and
/// Yealink SIP-T2 Administrator's Manual V85, same section (`g723_53`,
/// `g723_63` and the two iLBC rates, spelled `kpbs` there). Firmware that
/// shows two iLBC entries uses the rate names. A phone ignores payload names
/// it does not know.
pub const PROFILE_CODECS: &[(&str, u8)] = &[
    ("g722", 1),
    ("ilbc", 2),
    ("ilbc_15_2kpbs", 2),
    ("ilbc_13_33kpbs", 3),
    ("pcma", 4),
    ("pcmu", 5),
    ("g729", 0),
    ("g726_16", 0),
    ("g726_24", 0),
    ("g726_32", 0),
    ("g726_40", 0),
    ("g723_53", 0),
    ("g723_63", 0),
];

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
    Conference,
    Transfer,
    Hold,
    Forward,
    Dnd,
    Redial,
    CallReturn,
    Pickup,
    Voicemail,
    Dtmf,
    Intercom,
    XmlBrowser,
    History,
    Directory,
    Menu,
    Status,
    Prefix,
    LocalPhonebook,
    XmlPhonebook,
    SwitchAccountUp,
    SwitchAccountDown,
    KeypadLock,
}

impl KeyType {
    /// Yealink `linekey.X.type` codes (Auto Provisioning Guide V80).
    pub fn code(self) -> u8 {
        match self {
            KeyType::None => 0,
            KeyType::SpeedDial => 13,
            KeyType::Line => 15,
            KeyType::Blf => 16,
            KeyType::Conference => 1,
            KeyType::Transfer => 3,
            KeyType::Hold => 4,
            KeyType::Forward => 2,
            KeyType::Dnd => 5,
            KeyType::Redial => 6,
            KeyType::CallReturn => 7,
            KeyType::Pickup => 9,
            KeyType::Voicemail => 12,
            KeyType::Dtmf => 11,
            KeyType::Intercom => 14,
            KeyType::XmlBrowser => 27,
            KeyType::History => 28,
            KeyType::Directory => 61,
            KeyType::Menu => 30,
            KeyType::Status => 33,
            KeyType::Prefix => 40,
            KeyType::LocalPhonebook => 43,
            KeyType::XmlPhonebook => 22,
            KeyType::SwitchAccountUp => 51,
            KeyType::SwitchAccountDown => 52,
            KeyType::KeypadLock => 50,
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

/// A file the phone downloads (custom ringtone or wallpaper).
#[derive(Debug, Clone, Serialize)]
pub struct MediaFile {
    /// Download URL including provisioning credentials.
    pub url: String,
    /// File name the phone stores and selects it by.
    pub filename: String,
}

/// An additional remote phone book (a phone book section).
#[derive(Debug, Clone, Serialize)]
pub struct RemotePhonebook {
    pub name: String,
    pub url: String,
}

/// Remote phone book slots for sections; 1 and 2 are set in the common file.
pub const SECTION_SLOTS: std::ops::RangeInclusive<u16> = 3..=5;

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
    /// Custom ringtone (only used on models with `ringtone_max_kb > 0`).
    pub ringtone: Option<MediaFile>,
    /// Custom wallpaper (only used on models with `wallpaper`).
    pub wallpaper: Option<MediaFile>,
    /// Phone book sections shown on this phone (slots 3–5).
    pub phonebooks: Vec<RemotePhonebook>,
    /// Dial-now rules (see [`crate::dial_now`]).
    pub dial_now: Vec<String>,
    /// Comfort settings as (parameter, value), see
    /// [`crate::PhoneCatalog::effective_settings`].
    pub settings: Vec<(String, String)>,
    pub provisioning: crate::keys::Overrides,
    pub asset_base: String,
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
    render_phone_with(setup, &std::collections::BTreeMap::new())
}

pub fn render_phone_with(
    setup: &PhoneSetup<'_>,
    templates: &std::collections::BTreeMap<String, String>,
) -> Result<String, RenderError> {
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
            codecs: a.codecs,
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
                type_code: crate::keys::type_code(k.kind, &setup.provisioning.firmware_profile),
                account: if k.account == 0
                    && matches!(k.kind, KeyType::SpeedDial | KeyType::Intercom)
                {
                    0
                } else {
                    k.account.clamp(1, setup.model.accounts)
                },
                pickup_value: if k.kind == KeyType::Blf && !value.starts_with("park+") {
                    format!("*8*{value}")
                } else {
                    String::new()
                },
                value,
                label: cfg_value(&k.label),
            }
        })
        .collect();
    let media = |m: &Option<MediaFile>, supported: bool| {
        m.as_ref().filter(|_| supported).map(|m| MediaFile {
            url: cfg_value(&m.url),
            filename: cfg_value(&m.filename),
        })
    };
    let slots: Vec<u16> = SECTION_SLOTS.collect();
    let phonebooks: Vec<_> = setup
        .phonebooks
        .iter()
        .zip(&slots)
        .map(|(pb, index)| context! { index, name => cfg_value(&pb.name), url => cfg_value(&pb.url) })
        .collect();
    let unused_phonebooks: Vec<u16> = slots[phonebooks.len()..].to_vec();
    let profile_codecs: Vec<_> = PROFILE_CODECS
        .iter()
        .map(|(name, priority)| context! { name, priority, enable => u8::from(*priority > 0) })
        .collect();
    let custom_env = if templates.is_empty() {
        None
    } else {
        Some(crate::library::template_env(templates)?)
    };
    let renderer = custom_env.as_ref().unwrap_or_else(|| env());
    render_bounded(
        renderer.get_template("mac.cfg")?,
        context! {
            extra_parameters => extra_parameters(setup),
            profile_codecs,
            ringtone => media(&setup.ringtone, setup.model.ringtone_max_kb > 0),
            wallpaper => media(&setup.wallpaper, setup.model.wallpaper),
            phonebooks,
            unused_phonebooks,
            settings => setup
                .settings
                .iter()
                .map(|(param, value)| context! { param => cfg_value(param), value => cfg_value(value) })
                .collect::<Vec<_>>(),
            dial_now => (1..=crate::dial_now::MAX_RULES)
                .map(|i| context! { index => i, rule => setup.dial_now.get(i - 1).map(|r| cfg_value(r)).unwrap_or_default() })
                .collect::<Vec<_>>(),
            phone => context! { name => cfg_value(&setup.name), mac => setup.mac.clone() },
            model => setup.model,
            accounts => accounts,
            unused_accounts => unused,
            keys => keys,
            server => context! { host => cfg_value(&setup.sip_host), port => setup.sip_port },
            firmware_url => setup.firmware_url.as_deref().map(cfg_value),
            voicemail_code => cfg_value(&setup.voicemail_code),
        },
    )
}

fn extra_parameters(setup: &PhoneSetup<'_>) -> Vec<(String, String)> {
    let mut params = Vec::new();
    let caps = crate::keys::capabilities(setup.model);
    let mut effective = setup.provisioning.keys.clone();
    // Restore explicit defaults when an override is removed; omission would retain
    // the previous provisioned value on the handset.
    for slot in &caps.programmable {
        if !effective
            .iter()
            .any(|key| key.group == "programmable" && key.key == *slot)
        {
            let kind = match slot {
                1 | 5 => KeyType::History,
                2 | 6 => KeyType::Directory,
                3 => KeyType::Dnd,
                4 => KeyType::Menu,
                7 => KeyType::SwitchAccountUp,
                8 => KeyType::SwitchAccountDown,
                9 => KeyType::Status,
                _ => KeyType::None,
            };
            effective.push(crate::keys::Key {
                group: "programmable".into(),
                key: *slot,
                kind,
                value: String::new(),
                label: String::new(),
                account: 0,
                history_type: 0,
                phonebook: 0,
            });
        }
    }
    for module in 1..=caps.expansion_modules {
        for slot in 1..=caps.expansion_keys {
            let group = format!("expansion_{module}");
            if !effective
                .iter()
                .any(|key| key.group == group && key.key == slot)
            {
                effective.push(crate::keys::Key {
                    group,
                    key: slot,
                    kind: KeyType::None,
                    value: String::new(),
                    label: String::new(),
                    account: 0,
                    history_type: 0,
                    phonebook: 0,
                });
            }
        }
    }
    for key in &effective {
        let prefix = if key.group == "programmable" {
            format!("programablekey.{}", key.key)
        } else {
            format!(
                "expansion_module.{}.key.{}",
                key.group.trim_start_matches("expansion_"),
                key.key
            )
        };
        for (field, value) in [
            (
                "type",
                crate::keys::type_code(key.kind, &setup.provisioning.firmware_profile).to_string(),
            ),
            ("line", key.account.to_string()),
            ("value", cfg_value(&key.value)),
            ("label", cfg_value(&key.label)),
            ("history_type", key.history_type.to_string()),
            ("xml_phonebook", key.phonebook.to_string()),
            (
                "extension",
                if key.kind == KeyType::Blf {
                    format!("*8*{}", key.value)
                } else {
                    String::new()
                },
            ),
        ] {
            params.push((format!("{prefix}.{field}"), value));
        }
    }
    if !crate::keys::capabilities(setup.model)
        .softkey_states
        .is_empty()
    {
        params.push((
            "phone_setting.custom_softkey_enable".into(),
            u8::from(!setup.provisioning.softkeys.is_empty()).to_string(),
        ));
        for state in crate::keys::capabilities(setup.model).softkey_states.keys() {
            let url = if setup.provisioning.softkeys.contains_key(state) {
                format!("{}/softkeys/{}/{state}.xml", setup.asset_base, setup.mac)
            } else {
                String::new()
            };
            params.push((format!("custom_softkey_{state}.url"), url));
        }
    }
    params
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
        assert_eq!(m["features.pickup.direct_pickup_code"], "*8*");
        assert!(m["action_url.dnd_on"].starts_with(
            "http://pbx.lan:8080/provisioning/events?key=p%40ss%3Aw&event=dnd_on&mac=$mac"
        ));
    }

    #[test]
    fn desk_phone_config() {
        let c = catalog();
        let setup = PhoneSetup {
            provisioning: Default::default(),
            asset_base: String::new(),
            name: "Empfang".into(),
            mac: "805ec0123456".into(),
            model: c.get("t54w").unwrap(),
            accounts: vec![Account {
                index: 1,
                label: "20 Büro".into(),
                display_name: "Büro\r\nAnna".into(),
                username: "20-1".into(),
                password: "secret".into(),
                codecs: AccountCodecs::PhoneDefault,
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
            ringtone: Some(MediaFile {
                url: "http://u:p@pbx/provisioning/media/1/talkops-1.wav".into(),
                filename: "talkops-1.wav".into(),
            }),
            wallpaper: Some(MediaFile {
                url: "http://u:p@pbx/provisioning/media/2/talkops-2.jpg".into(),
                filename: "talkops-2.jpg".into(),
            }),
            phonebooks: vec![RemotePhonebook {
                name: "Familie\n".into(),
                url: "http://u:p@pbx/provisioning/phonebook/section/3.xml".into(),
            }],
            dial_now: vec!["11[0,2]".into(), "*9[7,8]".into()],
            settings: vec![],
        };
        let cfg = render_phone(&setup).unwrap();
        let m = parse(&cfg);
        assert_eq!(m["account.1.user_name"], "20-1");
        assert_eq!(m["account.1.display_name"], "BüroAnna");
        assert_eq!(m["account.1.sip_server.1.address"], "192.168.1.10");
        assert_eq!(m["account.2.enable"], "0", "unused accounts are disabled");
        assert!(
            !cfg.contains(".codec."),
            "standard profile keeps the codecs of the phone"
        );
        assert_eq!(m["account.16.enable"], "0");
        assert_eq!(m["linekey.1.type"], "15");
        assert_eq!(m["linekey.3.type"], "16");
        assert_eq!(m["linekey.3.pickup_value"], "*8*21");
        assert_eq!(m["linekey.4.type"], "13");
        assert_eq!(m["linekey.27.type"], "0", "unconfigured keys are cleared");
        assert!(!m.contains_key("linekey.28.type"));
        assert_eq!(
            m["static.firmware.url"],
            "http://u:p@pbx/provisioning/firmware/x/T54W.rom"
        );
        assert!(!m.contains_key("handset.1.name"));
        assert_eq!(
            m["ringtone.url"],
            "http://u:p@pbx/provisioning/media/1/talkops-1.wav"
        );
        assert_eq!(m["phone_setting.ring_type"], "talkops-1.wav");
        assert_eq!(
            m["wallpaper_upload.url"],
            "http://u:p@pbx/provisioning/media/2/talkops-2.jpg"
        );
        assert_eq!(m["phone_setting.backgrounds"], "talkops-2.jpg");
        assert!(
            !m.contains_key("features.send_key_tone"),
            "unset: phone keeps its value"
        );
        assert_eq!(m["dialplan.dialnow.rule.1"], "11[0,2]");
        assert_eq!(m["dialplan.dialnow.rule.2"], "*9[7,8]");
        assert_eq!(m["dialplan.dialnow.rule.3"], "", "unused rules are cleared");
        assert_eq!(m["dialplan.dialnow.rule.20"], "");
        assert!(!m.contains_key("dialplan.dialnow.rule.21"));
        assert_eq!(m["remote_phonebook.data.3.name"], "Familie");
        assert_eq!(
            m["remote_phonebook.data.3.url"],
            "http://u:p@pbx/provisioning/phonebook/section/3.xml"
        );
        assert_eq!(
            m["remote_phonebook.data.4.url"], "",
            "unused slots are cleared"
        );
        assert_eq!(m["remote_phonebook.data.5.name"], "");
    }

    #[test]
    fn every_desk_model_renders_its_last_key_and_reset() {
        let c = catalog();
        for model in c.all().filter(|m| m.family == PhoneFamily::Desk) {
            let mut setup = PhoneSetup {
                provisioning: Default::default(),
                asset_base: String::new(),
                name: model.name.clone(),
                mac: "805ec0123456".into(),
                model,
                accounts: vec![Account {
                    index: 1,
                    label: "Office".into(),
                    display_name: "Office".into(),
                    username: "101-1".into(),
                    password: "test".into(),
                    codecs: AccountCodecs::PhoneDefault,
                }],
                keys: vec![LineKey {
                    key: model.line_keys,
                    kind: KeyType::SpeedDial,
                    value: "**103".into(),
                    label: "Workshop\n".into(),
                    account: 1,
                }],
                sip_host: "pbx.lan".into(),
                sip_port: 5060,
                firmware_url: None,
                voicemail_code: "*97".into(),
                ringtone: None,
                wallpaper: None,
                phonebooks: vec![],
                dial_now: vec![],
                settings: vec![],
            };
            let m = parse(&render_phone(&setup).unwrap());
            assert_eq!(
                m[&format!("linekey.{}.type", model.line_keys)],
                "13",
                "{}",
                model.id
            );
            assert_eq!(m[&format!("linekey.{}.value", model.line_keys)], "**103");
            assert_eq!(m[&format!("linekey.{}.label", model.line_keys)], "Workshop");
            assert!(!m.contains_key(&format!("linekey.{}.type", model.line_keys + 1)));
            assert_eq!(
                m.get("features.keep_switch_page_key.enable")
                    .map(String::as_str),
                model.layout.as_ref().unwrap().page_key.then_some("1")
            );
            setup.keys.clear();
            let reset = parse(&render_phone(&setup).unwrap());
            assert_eq!(reset["linekey.1.type"], "15");
            assert_eq!(reset[&format!("linekey.{}.type", model.line_keys)], "0");
            setup.keys.push(LineKey {
                key: 1,
                kind: KeyType::None,
                value: String::new(),
                label: String::new(),
                account: 1,
            });
            assert_eq!(parse(&render_phone(&setup).unwrap())["linekey.1.type"], "0");
        }
    }

    #[test]
    fn wifi_handset_config() {
        let c = catalog();
        for id in ["ax83h", "ax86r"] {
            let setup = PhoneSetup {
                provisioning: Default::default(),
                asset_base: String::new(),
                name: "Lager".into(),
                mac: "249ad8123456".into(),
                model: c.get(id).unwrap(),
                accounts: vec![Account {
                    index: 1,
                    label: "30 Lager".into(),
                    display_name: "Lager".into(),
                    username: "30-1".into(),
                    password: "secret".into(),
                    codecs: AccountCodecs::Profiles,
                }],
                keys: vec![LineKey {
                    key: 1,
                    kind: KeyType::Blf,
                    value: "21".into(),
                    label: "Lab".into(),
                    account: 1,
                }],
                sip_host: "192.168.1.10".into(),
                sip_port: 5060,
                firmware_url: None,
                voicemail_code: "*97".into(),
                ringtone: Some(MediaFile {
                    url: "http://pbx/r.wav".into(),
                    filename: "r.wav".into(),
                }),
                wallpaper: None,
                phonebooks: vec![],
                dial_now: vec![],
                settings: vec![("features.send_key_tone".into(), "0".into())],
            };
            let m = parse(&render_phone(&setup).unwrap());
            assert_eq!(m["account.1.user_name"], "30-1", "{id}");
            assert_eq!(m["account.1.sip_server.1.address"], "192.168.1.10");
            assert_eq!(m["account.4.enable"], "0");
            // Codec profiles: G.722 first, then iLBC, G.711 as fallback.
            assert_eq!(m["account.1.codec.g722.enable"], "1", "{id}");
            assert_eq!(m["account.1.codec.g722.priority"], "1");
            assert_eq!(m["account.1.codec.ilbc.enable"], "1");
            assert_eq!(m["account.1.codec.ilbc_15_2kpbs.priority"], "2");
            assert_eq!(m["account.1.codec.ilbc_13_33kpbs.priority"], "3");
            assert_eq!(m["account.1.codec.pcma.enable"], "1");
            assert_eq!(m["account.1.codec.pcmu.priority"], "5");
            assert_eq!(m["account.1.codec.g729.enable"], "0");
            assert_eq!(m["account.1.codec.g729.priority"], "0");
            assert!(!m.contains_key("account.5.enable"), "{id} has 4 accounts");
            assert!(!m.contains_key("handset.1.name"), "not a DECT base");
            // 16 line keys; the BLF pickup code goes into `extension`.
            assert_eq!(m["dsskey.enable"], "1");
            assert_eq!(m["linekey.1.type"], "16");
            assert_eq!(m["linekey.1.value"], "21");
            assert_eq!(m["linekey.1.extension"], "*8*21", "{id}");
            assert!(!m.contains_key("linekey.1.pickup_value"));
            assert_eq!(m["linekey.16.type"], "0");
            assert!(!m.contains_key("linekey.17.type"));
            assert_eq!(m["ringtone.url"], "http://pbx/r.wav", "{id}");
            assert_eq!(m["phone_setting.ring_type"], "r.wav");
            assert_eq!(m["features.send_key_tone"], "0", "{id}: comfort setting");
        }
    }

    #[test]
    fn dect_base_config() {
        let c = catalog();
        let setup = PhoneSetup {
            provisioning: Default::default(),
            asset_base: String::new(),
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
                    codecs: AccountCodecs::PhoneDefault,
                })
                .collect(),
            keys: vec![],
            sip_host: "pbx".into(),
            sip_port: 5060,
            firmware_url: None,
            voicemail_code: "*97".into(),
            ringtone: None,
            wallpaper: None,
            phonebooks: vec![],
            dial_now: vec![],
            settings: vec![],
        };
        let m = parse(&render_phone(&setup).unwrap());
        assert_eq!(m["handset.2.incoming_lines"], "2");
        assert_eq!(m["handset.2.dial_out_default_line"], "2");
        assert_eq!(m["account.3.enable"], "0");
        assert!(!m.contains_key("linekey.1.type"));
        assert!(!m.contains_key("static.firmware.url"));
    }
}
