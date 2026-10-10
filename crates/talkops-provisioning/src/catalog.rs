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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VendorFile {
    vendor: String,
    #[allow(dead_code)]
    status: String,
    models: Vec<PhoneModel>,
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
}

impl PhoneCatalog {
    pub fn load_dir(dir: &Path) -> Result<Self, CatalogError> {
        let err = |file: &Path, message: String| CatalogError {
            file: file.display().to_string(),
            message,
        };
        let mut models = BTreeMap::new();
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
        }
        Ok(Self { models })
    }

    pub fn get(&self, id: &str) -> Option<&PhoneModel> {
        self.models.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &PhoneModel> {
        self.models.values()
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
