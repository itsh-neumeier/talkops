//! Supported phone models (`presets/phones/*.yaml`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhoneFamily {
    Desk,
    Dect,
    Conference,
    /// Wi-Fi handsets (Yealink AX series): no base station, no line keys.
    Wifi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhoneModel {
    pub id: String,
    pub name: String,
    pub family: PhoneFamily,
    /// SIP accounts (DECT: handsets).
    pub accounts: u16,
    /// Programmable line keys managed by TalkOps.
    pub line_keys: u16,
    #[serde(default)]
    pub video: bool,
    /// Largest custom ringtone the phone accepts in KiB; 0 = no custom
    /// ringtones managed by TalkOps.
    #[serde(default)]
    pub ringtone_max_kb: u32,
    /// The phone takes a custom wallpaper (`wallpaper_upload.url`).
    #[serde(default)]
    pub wallpaper: bool,
    /// Filled from the file header.
    #[serde(default)]
    pub vendor: String,
}

impl PhoneModel {
    /// Line keys can be speed dials (`linekey.X.type = 13`); the AX Wi-Fi
    /// handsets only know line, BLF, DTMF and XML browser keys.
    pub fn speed_dial_keys(&self) -> bool {
        self.family != PhoneFamily::Wifi
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingKind {
    /// `0` (off) or `1` (on).
    Bool,
    /// One of `values`.
    Choice,
}

/// A comfort setting an admin can set for phones (key tone, display …).
/// It is only written to a phone when set in TalkOps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhoneSetting {
    /// Stable name used in the database and the UI texts.
    pub key: String,
    /// UI section (`tones`, `display` …).
    pub group: String,
    /// Configuration parameter on the phone.
    pub param: String,
    pub kind: SettingKind,
    /// Allowed values of a `choice`, in display order.
    #[serde(default)]
    pub values: Vec<String>,
    /// The phone's factory value, if documented.
    #[serde(default)]
    pub default: Option<String>,
    /// Phone families that take the parameter.
    pub families: Vec<PhoneFamily>,
}

impl PhoneSetting {
    pub fn allows(&self, value: &str) -> bool {
        match self.kind {
            SettingKind::Bool => value == "0" || value == "1",
            SettingKind::Choice => self.values.iter().any(|v| v == value),
        }
    }
}

/// Values of comfort settings by key (`{"key_tone": "0"}`).
pub type SettingValues = BTreeMap<String, String>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VendorFile {
    vendor: String,
    #[allow(dead_code)]
    status: String,
    models: Vec<PhoneModel>,
    #[serde(default)]
    settings: Vec<PhoneSetting>,
}

#[derive(Debug, thiserror::Error)]
#[error("phone catalog {file}: {message}")]
pub struct CatalogError {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct PhoneCatalog {
    models: BTreeMap<String, PhoneModel>,
    settings: Vec<PhoneSetting>,
}

impl PhoneCatalog {
    pub fn load_dir(dir: &Path) -> Result<Self, CatalogError> {
        let err = |file: &Path, message: String| CatalogError {
            file: file.display().to_string(),
            message,
        };
        let mut models = BTreeMap::new();
        let mut settings: Vec<PhoneSetting> = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| err(dir, e.to_string()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yaml"))
            .collect();
        entries.sort();
        for path in entries {
            let text = std::fs::read_to_string(&path).map_err(|e| err(&path, e.to_string()))?;
            let file: VendorFile =
                serde_yaml_ng::from_str(&text).map_err(|e| err(&path, e.to_string()))?;
            for mut model in file.models {
                if model.accounts == 0 {
                    return Err(err(
                        &path,
                        format!("model {} needs at least one account", model.id),
                    ));
                }
                model.vendor = file.vendor.clone();
                if models.insert(model.id.clone(), model).is_some() {
                    return Err(err(&path, "duplicate model id".into()));
                }
            }
            for setting in file.settings {
                let problem = if settings.iter().any(|s| s.key == setting.key) {
                    Some("duplicate key")
                } else if setting.kind == SettingKind::Choice && setting.values.is_empty() {
                    Some("a choice needs values")
                } else if setting.families.is_empty() {
                    Some("no phone families")
                } else if setting
                    .default
                    .as_deref()
                    .is_some_and(|d| !setting.allows(d))
                {
                    Some("default is not an allowed value")
                } else {
                    None
                };
                if let Some(problem) = problem {
                    return Err(err(&path, format!("setting {}: {problem}", setting.key)));
                }
                settings.push(setting);
            }
        }
        Ok(Self { models, settings })
    }

    pub fn get(&self, id: &str) -> Option<&PhoneModel> {
        self.models.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &PhoneModel> {
        self.models.values()
    }

    /// Comfort settings in display order.
    pub fn settings(&self) -> &[PhoneSetting] {
        &self.settings
    }

    /// Checks keys and values against the catalog.
    pub fn check_settings(&self, values: &SettingValues) -> Result<(), String> {
        for (key, value) in values {
            let setting = self
                .settings
                .iter()
                .find(|s| &s.key == key)
                .ok_or_else(|| format!("unknown phone setting `{key}`"))?;
            if !setting.allows(value) {
                return Err(format!("invalid value `{value}` for phone setting `{key}`"));
            }
        }
        Ok(())
    }

    /// Parameters to write for a phone: its own value, else the global one;
    /// settings its family does not take and unset ones are left out.
    pub fn effective_settings(
        &self,
        model: &PhoneModel,
        global: &SettingValues,
        phone: &SettingValues,
    ) -> Vec<(String, String)> {
        self.settings
            .iter()
            .filter(|s| s.families.contains(&model.family))
            .filter_map(|s| {
                let value = phone.get(&s.key).or_else(|| global.get(&s.key))?;
                s.allows(value).then(|| (s.param.clone(), value.clone()))
            })
            .collect()
    }

    /// Guesses the model from a Yealink User-Agent such as
    /// `Yealink SIP-T54W 96.86.0.70 80:5e:c0:12:34:56`.
    pub fn from_user_agent(&self, user_agent: &str) -> Option<&PhoneModel> {
        let product = user_agent.split_whitespace().nth(1)?.to_ascii_lowercase();
        let product = product.strip_prefix("sip-").unwrap_or(&product);
        self.models.values().find(|m| m.id == product)
    }
}

/// Firmware version from a Yealink User-Agent (third token).
pub fn firmware_from_user_agent(user_agent: &str) -> Option<String> {
    let mut parts = user_agent.split_whitespace();
    (parts.next()?.eq_ignore_ascii_case("yealink")).then_some(())?;
    parts
        .nth(1)
        .filter(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn catalog() -> PhoneCatalog {
        PhoneCatalog::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/phones"))
            .unwrap()
    }

    #[test]
    fn loads_repository_catalog() {
        let c = catalog();
        let t54w = c.get("t54w").unwrap();
        assert_eq!(t54w.vendor, "yealink");
        assert_eq!(c.get("w70b").unwrap().family, PhoneFamily::Dect);
        assert_eq!(
            c.from_user_agent("Yealink SIP-T54W 96.86.0.70 80:5e:c0:12:34:56")
                .unwrap()
                .id,
            "t54w"
        );
        assert_eq!(
            c.from_user_agent("Yealink W70B 146.85.0.20").unwrap().id,
            "w70b"
        );
        let ax = c
            .from_user_agent("Yealink AX83H 180.86.0.5 24:9a:d8:12:34:56")
            .unwrap();
        assert_eq!(
            (ax.id.as_str(), ax.family, ax.accounts, ax.line_keys),
            ("ax83h", PhoneFamily::Wifi, 4, 16)
        );
        assert_eq!(
            c.from_user_agent("Yealink AX86R 180.86.0.5").unwrap().id,
            "ax86r"
        );
        assert!(c.from_user_agent("Snom D785").is_none());

        // Comfort settings: phone value wins over the global one, unknown
        // or invalid values and other families are left out.
        assert!(c.settings().iter().any(|s| s.key == "key_tone"));
        let global = SettingValues::from([
            ("key_tone".to_owned(), "0".to_owned()),
            ("backlight_time".to_owned(), "60".to_owned()),
        ]);
        let phone = SettingValues::from([("backlight_time".to_owned(), "15".to_owned())]);
        let ax_settings = c.effective_settings(ax, &global, &phone);
        assert!(ax_settings.contains(&("features.send_key_tone".into(), "0".into())));
        assert!(ax_settings.contains(&("phone_setting.backlight_time".into(), "15".into())));
        assert_eq!(ax_settings.len(), 2);
        assert!(c.effective_settings(t54w, &global, &phone).is_empty());
        assert!(c.check_settings(&global).is_ok());
        let bad = SettingValues::from([("backlight_time".to_owned(), "45".to_owned())]);
        assert!(c.check_settings(&bad).is_err());
        let unknown = SettingValues::from([("nope".to_owned(), "1".to_owned())]);
        assert!(c.check_settings(&unknown).is_err());
        assert_eq!((t54w.ringtone_max_kb, t54w.wallpaper), (8192, true));
        assert_eq!(c.get("t53w").unwrap().ringtone_max_kb, 100);
        assert_eq!((ax.ringtone_max_kb, ax.wallpaper), (8192, true));
        assert!(!ax.speed_dial_keys() && t54w.speed_dial_keys());
        assert_eq!(
            firmware_from_user_agent("Yealink SIP-T54W 96.86.0.70 80:5e"),
            Some("96.86.0.70".into())
        );
        assert_eq!(firmware_from_user_agent("curl/8"), None);
    }
}
