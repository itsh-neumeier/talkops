//! SIP trunk provider presets (`presets/trunks/*.yaml`).
//!
//! A preset describes how to talk to a provider: registrar, transport,
//! number formats, caller ID handling, credential model. A trunk stores the
//! preset id plus optional per-trunk overrides of the [`SipSettings`].

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::dialing::NumberFormat;

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("cannot read presets from {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid preset {file}: {message}")]
    Invalid { file: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetStatus {
    /// Tested end-to-end by the TalkOps maintainers.
    Verified,
    /// Reported working by community members.
    Community,
    /// Derived from provider documentation, not yet tested.
    Untested,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalizedText {
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub de: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountMode {
    /// Every number registers with its own credentials (e.g. LEONET, Telekom IP-Anschluss).
    PerNumber,
    /// One account carries all numbers of the trunk (classic SIP trunk).
    Shared,
    /// No registration; the provider authenticates by source IP or digest on INVITE only.
    NoRegistration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialModel {
    pub mode: AccountMode,
    /// Template for the SIP username of an account. Placeholders:
    /// `{e164}` (+4989…), `{e164_digits}` (4989…), `{input}` (value entered by the user).
    #[serde(default = "default_username_template")]
    pub username_template: String,
    /// Template for the authentication username; empty = same as username.
    #[serde(default)]
    pub auth_username_template: String,
    /// Shown next to the username field in the UI.
    #[serde(default)]
    pub username_hint: LocalizedText,
}

fn default_username_template() -> String {
    "{input}".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    #[default]
    Udp,
    Tcp,
    Tls,
}

impl Transport {
    pub fn as_str(self) -> &'static str {
        match self {
            Transport::Udp => "udp",
            Transport::Tcp => "tcp",
            Transport::Tls => "tls",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Srtp {
    #[default]
    Off,
    Optional,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CallerIdHeader {
    /// Caller number in the From header.
    #[default]
    From,
    /// P-Asserted-Identity.
    Pai,
    /// P-Preferred-Identity (common for German trunks, "CLIP no screening").
    Ppi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FromUser {
    /// From user = account username (the provider identifies the account by it).
    #[default]
    Username,
    /// From user = selected caller number.
    Number,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Dtmf {
    #[default]
    Rfc2833,
    Info,
    Inband,
}

/// Connection parameters. Every field can be overridden per trunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SipSettings {
    /// Registrar / SIP domain (`host` or `host:port`). `None`: the customer
    /// receives an individual registrar and must enter it.
    #[serde(default)]
    pub registrar: Option<String>,
    /// Authentication realm if it differs from the registrar host.
    #[serde(default)]
    pub realm: Option<String>,
    /// Proxy used for calls if it differs from the registrar.
    #[serde(default)]
    pub proxy: Option<String>,
    /// Outbound proxy all SIP traffic is sent to.
    #[serde(default)]
    pub outbound_proxy: Option<String>,
    #[serde(default)]
    pub transport: Transport,
    #[serde(default)]
    pub srtp: Srtp,
    #[serde(default = "yes")]
    pub register: bool,
    #[serde(default = "default_expire")]
    pub expire_seconds: u32,
    /// OPTIONS keepalive interval; `None` disables pinging.
    #[serde(default)]
    pub ping_seconds: Option<u32>,
    /// Format of the dialed number in the Request-URI.
    #[serde(default)]
    pub number_format: NumberFormat,
    /// Format of the caller number in From/PAI/PPI.
    #[serde(default)]
    pub caller_id_format: NumberFormat,
    #[serde(default)]
    pub caller_id_header: CallerIdHeader,
    #[serde(default)]
    pub from_user: FromUser,
    /// Domain in the From header; defaults to the registrar host.
    #[serde(default)]
    pub from_domain: Option<String>,
    #[serde(default = "default_codecs")]
    pub codecs: Vec<String>,
    #[serde(default)]
    pub dtmf: Dtmf,
    #[serde(default)]
    pub t38: bool,
}

fn yes() -> bool {
    true
}
fn default_expire() -> u32 {
    600
}
fn default_codecs() -> Vec<String> {
    vec!["G722".into(), "PCMA".into(), "PCMU".into()]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrunkPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub product: String,
    /// ISO 3166 country code(s) the preset is meant for, e.g. `DE`.
    pub country: String,
    pub status: PresetStatus,
    /// Official documentation the parameters were taken from.
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub notes: LocalizedText,
    pub credentials: CredentialModel,
    pub sip: SipSettings,
}

impl TrunkPreset {
    /// Applies per-trunk overrides (a JSON object with [`SipSettings`] keys).
    pub fn effective_sip(&self, overrides: &serde_json::Value) -> Result<SipSettings, String> {
        let mut base = serde_json::to_value(&self.sip).map_err(|e| e.to_string())?;
        if let (Some(base), Some(over)) = (base.as_object_mut(), overrides.as_object()) {
            for (key, value) in over {
                base.insert(key.clone(), value.clone());
            }
        } else if !overrides.is_null() {
            return Err("overrides must be a JSON object".into());
        }
        serde_json::from_value(base).map_err(|e| format!("invalid override: {e}"))
    }
}

/// Renders a username template for an account.
pub fn render_template(template: &str, e164: Option<&str>, input: &str) -> String {
    let e164 = e164.unwrap_or_default();
    template
        .replace("{e164_digits}", e164.trim_start_matches('+'))
        .replace("{e164}", e164)
        .replace("{input}", input)
}

/// All presets, keyed by id.
#[derive(Debug, Clone, Default)]
pub struct PresetCatalog {
    presets: BTreeMap<String, TrunkPreset>,
}

impl PresetCatalog {
    /// Loads every `*.yaml` file of `dir`. The file name (without extension)
    /// must equal the preset id.
    pub fn load_dir(dir: &Path) -> Result<Self, PresetError> {
        let io = |source| PresetError::Io {
            path: dir.display().to_string(),
            source,
        };
        let mut presets = BTreeMap::new();
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(io)?
            .collect::<Result<_, _>>()
            .map_err(io)?;
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let file = path.display().to_string();
            let text = std::fs::read_to_string(&path).map_err(|source| PresetError::Io {
                path: file.clone(),
                source,
            })?;
            let preset = Self::parse(&text).map_err(|message| PresetError::Invalid {
                file: file.clone(),
                message,
            })?;
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if preset.id != stem {
                return Err(PresetError::Invalid {
                    file,
                    message: format!("id `{}` must match file name `{stem}`", preset.id),
                });
            }
            presets.insert(preset.id.clone(), preset);
        }
        Ok(Self { presets })
    }

    /// Parses and validates one preset.
    pub fn parse(text: &str) -> Result<TrunkPreset, String> {
        let preset: TrunkPreset = serde_yaml_ng::from_str(text).map_err(|e| e.to_string())?;
        if !preset
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!(
                "id `{}` may only contain a-z, 0-9 and '-'",
                preset.id
            ));
        }
        if preset.status != PresetStatus::Untested && preset.sources.is_empty() {
            return Err("verified/community presets must list sources".into());
        }
        if preset.sip.codecs.is_empty() {
            return Err("at least one codec is required".into());
        }
        if preset.credentials.mode == AccountMode::NoRegistration && preset.sip.register {
            return Err("mode no_registration requires sip.register: false".into());
        }
        if preset.credentials.mode == AccountMode::PerNumber
            && !preset.credentials.username_template.contains('{')
        {
            return Err("per_number presets need a username template with a placeholder".into());
        }
        Ok(preset)
    }

    pub fn get(&self, id: &str) -> Option<&TrunkPreset> {
        self.presets.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &TrunkPreset> {
        self.presets.values()
    }

    pub fn len(&self) -> usize {
        self.presets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.presets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEONET_LIKE: &str = r#"
id: example
name: Example
country: DE
status: untested
credentials:
  mode: per_number
  username_template: "leo{e164_digits}"
sip:
  registrar: sip.example.net
  number_format: e164_no_plus
"#;

    #[test]
    fn parses_and_applies_overrides() {
        let preset = PresetCatalog::parse(LEONET_LIKE).unwrap();
        assert_eq!(preset.sip.transport, Transport::Udp);
        assert_eq!(preset.sip.expire_seconds, 600);
        assert_eq!(preset.sip.number_format, NumberFormat::E164NoPlus);

        let eff = preset
            .effective_sip(&serde_json::json!({"transport": "tls", "registrar": "other.example"}))
            .unwrap();
        assert_eq!(eff.transport, Transport::Tls);
        assert_eq!(eff.registrar.as_deref(), Some("other.example"));
        assert!(
            preset
                .effective_sip(&serde_json::json!({"transport": "smoke"}))
                .is_err()
        );
        assert!(preset.effective_sip(&serde_json::json!({})).is_ok());
    }

    #[test]
    fn rejects_invalid_presets() {
        assert!(PresetCatalog::parse(&LEONET_LIKE.replace("example\n", "Bad_Id\n")).is_err());
        assert!(PresetCatalog::parse(&LEONET_LIKE.replace("untested", "verified")).is_err());
        assert!(PresetCatalog::parse(&LEONET_LIKE.replace("leo{e164_digits}", "fixed")).is_err());
        assert!(PresetCatalog::parse(&format!("{LEONET_LIKE}  bogus: 1\n")).is_err());
    }

    #[test]
    fn renders_templates() {
        assert_eq!(
            render_template("leo{e164_digits}", Some("+49891234"), ""),
            "leo49891234"
        );
        assert_eq!(
            render_template("{e164}", Some("+49891234"), ""),
            "+49891234"
        );
        assert_eq!(render_template("{input}", None, "user1"), "user1");
    }

    #[test]
    fn repository_presets_are_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/trunks");
        let catalog = PresetCatalog::load_dir(&dir).unwrap();
        assert!(
            catalog.get("generic").is_some(),
            "generic preset is required"
        );
        assert!(catalog.get("leonet").is_some());
    }
}
