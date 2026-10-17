//! Vendor-neutral device, account, provisioning and asset context.
use crate::{PhoneModel, yealink::LineKey};
use serde::Serialize;

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
pub(crate) fn url_part(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
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
    /// [`crate::yealink::PROFILE_CODECS`]. TalkOps selects the codec of each call.
    Profiles,
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
