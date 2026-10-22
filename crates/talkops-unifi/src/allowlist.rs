//! Field allowlist for changes to the voice SSID (ADR 0042).
//!
//! TalkOps changes only these fields of a Wi-Fi broadcast. All other fields
//! go back to UniFi unchanged. Snapshots contain only these fields, so they
//! never contain the passphrase or other secrets.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::Digest;

/// Network version from which TalkOps can write (required fields
/// `channel2gLockedTo6` and `dtimPeriod2gLockedTo3`).
pub const WRITE_MIN_VERSION: &str = "10.6.106";

macro_rules! fields {
    ($($variant:ident = $key:literal, $pointer:literal;)*) => {
        /// A field that TalkOps may change.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Field {
            $($variant,)*
        }

        impl Field {
            pub const ALL: &'static [Field] = &[$(Field::$variant,)*];

            /// Key in snapshots, the API and the UI.
            pub fn key(self) -> &'static str {
                match self { $(Field::$variant => $key,)* }
            }

            /// JSON pointer in the Wi-Fi broadcast object.
            pub fn pointer(self) -> &'static str {
                match self { $(Field::$variant => $pointer,)* }
            }

            pub fn from_key(key: &str) -> Option<Field> {
                match key { $($key => Some(Field::$variant),)* _ => None }
            }
        }
    };
}

fields! {
    BssTransition = "bssTransitionEnabled", "/bssTransitionEnabled";
    Wpa3FastRoaming = "securityConfiguration.wpa3FastRoamingEnabled", "/securityConfiguration/wpa3FastRoamingEnabled";
    PmfMode = "securityConfiguration.pmfMode", "/securityConfiguration/pmfMode";
    HandoffSuggestions = "handoffSuggestionsConfiguration", "/handoffSuggestionsConfiguration";
    Frequencies = "broadcastingFrequenciesGHz", "/broadcastingFrequenciesGHz";
    Uapsd = "uapsdEnabled", "/uapsdEnabled";
    MulticastToUnicast = "multicastToUnicastConversionEnabled", "/multicastToUnicastConversionEnabled";
    ArpProxy = "arpProxyEnabled", "/arpProxyEnabled";
    DtimOverride = "dtimPeriodByFrequencyGHzOverride", "/dtimPeriodByFrequencyGHzOverride";
}

impl Serialize for Field {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for Field {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let key = String::deserialize(d)?;
        Field::from_key(&key).ok_or_else(|| serde::de::Error::custom("field is not allowed"))
    }
}

/// The value of a field, or a missing field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Slot {
    Present { value: Value },
    Absent { absent: bool },
}

impl Slot {
    pub fn absent() -> Slot {
        Slot::Absent { absent: true }
    }

    pub fn value(&self) -> Option<&Value> {
        match self {
            Slot::Present { value } => Some(value),
            Slot::Absent { .. } => None,
        }
    }

    /// Same value; numbers compare as numbers (`5` equals `5.0`).
    pub fn same(&self, other: &Slot) -> bool {
        match (self.value(), other.value()) {
            (Some(a), Some(b)) => canonical(a) == canonical(b),
            (None, None) => true,
            _ => false,
        }
    }
}

/// Field key to value. Contains no secret.
pub type Snapshot = std::collections::BTreeMap<String, Slot>;

/// Security types whose objects contain `pmfMode`.
const PMF_TYPES: &[&str] = &[
    "WPA2_PERSONAL",
    "WPA2_WPA3_PERSONAL",
    "WPA2_ENTERPRISE",
    "WPA2_WPA3_ENTERPRISE",
];

pub fn security_type(obj: &Value) -> Option<&str> {
    obj.pointer("/securityConfiguration/type")
        .and_then(Value::as_str)
}

pub fn read(obj: &Value, field: Field) -> Slot {
    match obj.pointer(field.pointer()) {
        Some(v) => Slot::Present { value: v.clone() },
        None => Slot::absent(),
    }
}

/// Values of the given fields.
pub fn snapshot(obj: &Value, fields: &[Field]) -> Snapshot {
    fields
        .iter()
        .map(|f| (f.key().to_owned(), read(obj, *f)))
        .collect()
}

/// Fields that this object (and so this Network version) supports.
pub fn available(obj: &Value) -> Vec<Field> {
    let has = |p: &str| obj.pointer(p).is_some();
    // Optional fields of the 10.6.106 schema; `null` or absent means "off".
    let schema_10_6 = has("/channel2gLockedTo6") && has("/dtimPeriod2gLockedTo3");
    Field::ALL
        .iter()
        .copied()
        .filter(|f| match f {
            Field::HandoffSuggestions | Field::DtimOverride => schema_10_6,
            Field::PmfMode => {
                has(f.pointer()) || security_type(obj).is_some_and(|t| PMF_TYPES.contains(&t))
            }
            _ => has(f.pointer()),
        })
        .collect()
}

/// Hash of the allowlist values; a changed hash shows a change in UniFi.
pub fn hash(obj: &Value) -> String {
    let snap = snapshot(obj, Field::ALL);
    let v = canonical(&serde_json::to_value(&snap).unwrap_or_default());
    hex::encode(sha2::Sha256::digest(v.to_string().as_bytes()))
}

/// Sorted keys and numbers as floats, for comparisons.
pub fn canonical(v: &Value) -> Value {
    match v {
        Value::Number(n) => n
            .as_f64()
            .and_then(serde_json::Number::from_f64)
            .map_or(Value::Null, Value::Number),
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), canonical(&o[k])))
                    .collect(),
            )
        }
        other => other.clone(),
    }
}

/// Why TalkOps does not write this SSID.
pub fn not_writable(obj: &Value) -> Option<&'static str> {
    if obj.get("type").and_then(Value::as_str) != Some("STANDARD") {
        return Some("ssid_type");
    }
    if obj.pointer("/metadata/origin").and_then(Value::as_str) != Some("USER_DEFINED") {
        return Some("ssid_origin");
    }
    if !credentials_readable(obj) {
        return Some("secret_not_readable");
    }
    None
}

fn readable_secret(v: Option<&Value>) -> bool {
    v.and_then(Value::as_str)
        .is_some_and(|s| s.chars().count() >= 8 && !s.chars().all(|c| matches!(c, '*' | '•' | '●')))
}

/// The PUT replaces the whole object. A security type with a passphrase
/// needs it in the GET answer, or the PUT fails or removes the credential.
pub fn credentials_readable(obj: &Value) -> bool {
    let sec = obj.get("securityConfiguration");
    let field = |k: &str| sec.and_then(|s| s.get(k));
    match security_type(obj) {
        Some("WPA2_PERSONAL") => match field("presharedKeys").and_then(Value::as_array) {
            Some(keys) if !keys.is_empty() => {
                keys.iter().all(|k| readable_secret(k.get("passphrase")))
            }
            _ => readable_secret(field("passphrase")),
        },
        Some("WPA2_WPA3_PERSONAL" | "WPA3_PERSONAL") => readable_secret(field("passphrase")),
        // Open and enterprise types: no passphrase in the object.
        Some(_) => true,
        None => false,
    }
}

/// Checks a new value against the limits of the API specification.
pub fn check(field: Field, slot: &Slot, obj: &Value) -> Result<(), String> {
    let Some(v) = slot.value() else {
        // Only optional fields can be removed (rollback to "absent").
        return match field {
            Field::HandoffSuggestions | Field::DtimOverride | Field::PmfMode => Ok(()),
            _ => Err(format!("{} cannot be removed", field.key())),
        };
    };
    let int_in = |v: &Value, lo: i64, hi: i64| v.as_i64().is_some_and(|n| (lo..=hi).contains(&n));
    let ok = match field {
        Field::BssTransition | Field::Uapsd | Field::MulticastToUnicast | Field::ArpProxy => {
            v.is_boolean()
        }
        Field::Wpa3FastRoaming => match v {
            Value::Bool(false) => true,
            // The specification: only with the default fast roaming (802.11r).
            Value::Bool(true) => {
                obj.pointer("/securityConfiguration/fastRoamingEnabled") == Some(&Value::Bool(true))
            }
            _ => false,
        },
        Field::PmfMode => matches!(v.as_str(), Some("REQUIRED" | "OPTIONAL")) || v.is_null(),
        Field::HandoffSuggestions => match v {
            Value::Null => true,
            Value::Object(o) => o.iter().all(|(k, x)| match k.as_str() {
                "band5GHzRssiThreshold" => x.is_null() || int_in(x, -80, -60),
                "band6GHzRssiThreshold" => x.is_null() || int_in(x, -90, -70),
                _ => false,
            }),
            _ => false,
        },
        Field::Frequencies => v.as_array().is_some_and(|a| {
            let f: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
            !a.is_empty()
                && f.len() == a.len()
                && f.iter().all(|x| [2.4, 5.0, 6.0].contains(x))
                && f.iter().enumerate().all(|(i, x)| !f[..i].contains(x))
        }),
        Field::DtimOverride => v.as_object().is_some_and(|o| {
            let locked = obj.get("dtimPeriod2gLockedTo3") == Some(&Value::Bool(true));
            o.len() == 3
                && ["2.4", "5", "6"]
                    .iter()
                    .all(|b| o.get(*b).is_some_and(|x| int_in(x, 1, 255)))
                && (!locked || o.get("2.4").and_then(Value::as_i64) == Some(3))
        }),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("invalid value for {}", field.key()))
    }
}

/// Sets or removes the fields in `obj`.
pub fn merge(obj: &mut Value, changes: &[(Field, Slot)]) {
    for (field, slot) in changes {
        let (parent, key) = match field.pointer().rsplit_once('/') {
            Some((p, k)) => (p, k),
            None => continue,
        };
        let target = if parent.is_empty() {
            Some(&mut *obj)
        } else {
            obj.pointer_mut(parent)
        };
        let Some(Value::Object(map)) = target else {
            continue;
        };
        match slot.value() {
            Some(v) => {
                map.insert(key.to_owned(), v.clone());
            }
            None => {
                map.remove(key);
            }
        }
    }
}

/// The PUT body: the object without its read-only fields.
pub fn update_body(detail: &Value) -> Value {
    let mut body = detail.clone();
    if let Value::Object(map) = &mut body {
        map.remove("id");
        map.remove("metadata");
    }
    body
}

/// JSON pointers where `b` differs from `a` (leaf level).
pub fn differences(a: &Value, b: &Value) -> Vec<String> {
    fn walk(a: Option<&Value>, b: Option<&Value>, path: String, out: &mut Vec<String>) {
        match (a, b) {
            (Some(Value::Object(x)), Some(Value::Object(y))) => {
                let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
                keys.sort();
                keys.dedup();
                for k in keys {
                    walk(x.get(k), y.get(k), format!("{path}/{k}"), out);
                }
            }
            (x, y) if x.map(canonical) != y.map(canonical) => out.push(path),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(Some(a), Some(b), String::new(), &mut out);
    out
}

/// True if `path` is an allowlist field or inside one.
pub fn allowed_path(path: &str) -> bool {
    Field::ALL.iter().any(|f| {
        path == f.pointer()
            || path
                .strip_prefix(f.pointer())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Object form of a snapshot for storage and the API.
pub fn to_json(s: &Snapshot) -> Value {
    Value::Object(
        s.iter()
            .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or_default()))
            .collect::<Map<String, Value>>(),
    )
}

/// Reads a stored snapshot; unknown keys are dropped.
pub fn from_json(v: &Value) -> Snapshot {
    v.as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| Field::from_key(k).is_some())
        .filter_map(|(k, v)| Some((k.clone(), serde_json::from_value(v.clone()).ok()?)))
        .collect()
}

/// `a.b.c` >= `min` (numeric parts).
pub fn version_at_least(version: &str, min: &str) -> bool {
    let parts = |s: &str| -> Option<Vec<u64>> {
        s.trim()
            .split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse().ok())
            .collect()
    };
    match (parts(version), parts(min)) {
        (Some(v), Some(m)) if !v.is_empty() => v >= m,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub fn detail() -> Value {
        json!({
            "type": "STANDARD", "id": "6f1c", "name": "Voice", "enabled": true,
            "metadata": {"origin": "USER_DEFINED"},
            "securityConfiguration": {"type": "WPA2_WPA3_PERSONAL", "passphrase": "correct-horse",
                "pmfMode": "OPTIONAL", "fastRoamingEnabled": true, "wpa3FastRoamingEnabled": false,
                "saeConfiguration": {"anticloggingThresholdSeconds": 5, "syncTimeSeconds": 5}},
            "broadcastingFrequenciesGHz": [2.4, 5], "bssTransitionEnabled": false,
            "uapsdEnabled": false, "multicastToUnicastConversionEnabled": false,
            "arpProxyEnabled": false, "channel2gLockedTo6": false, "dtimPeriod2gLockedTo3": true,
            "hideName": false, "clientIsolationEnabled": false, "advertiseDeviceName": false
        })
    }

    #[test]
    fn keys_and_pointers() {
        for f in Field::ALL {
            assert_eq!(Field::from_key(f.key()), Some(*f));
            assert!(allowed_path(f.pointer()));
        }
        assert!(allowed_path(
            "/handoffSuggestionsConfiguration/band5GHzRssiThreshold"
        ));
        assert!(!allowed_path("/securityConfiguration/passphrase"));
        assert!(!allowed_path("/uapsdEnabledX"));
        assert!(serde_json::from_value::<Field>(json!("name")).is_err());
    }

    #[test]
    fn snapshots_hold_no_secret() {
        let snap = snapshot(&detail(), Field::ALL);
        let text = to_json(&snap).to_string();
        assert!(!text.contains("correct-horse"));
        assert_eq!(snap["handoffSuggestionsConfiguration"], Slot::absent());
        assert_eq!(from_json(&to_json(&snap)), snap);
    }

    #[test]
    fn availability_follows_the_object() {
        let all = available(&detail());
        assert_eq!(all.len(), Field::ALL.len());
        // Before 10.6.106: no handoff or DTIM override; WPA3 personal: no PMF field.
        let mut old = detail();
        old.as_object_mut().unwrap().remove("channel2gLockedTo6");
        old["securityConfiguration"] =
            json!({"type": "WPA3_PERSONAL", "passphrase": "x".repeat(9)});
        let a = available(&old);
        assert!(!a.contains(&Field::HandoffSuggestions));
        assert!(!a.contains(&Field::DtimOverride));
        assert!(!a.contains(&Field::PmfMode));
        assert!(!a.contains(&Field::Wpa3FastRoaming));
    }

    #[test]
    fn value_checks() {
        let d = detail();
        let p = |v: Value| Slot::Present { value: v };
        assert!(check(Field::Uapsd, &p(json!(true)), &d).is_ok());
        assert!(check(Field::Uapsd, &p(json!("yes")), &d).is_err());
        assert!(check(Field::Uapsd, &Slot::absent(), &d).is_err());
        assert!(check(Field::Wpa3FastRoaming, &p(json!(true)), &d).is_ok());
        let mut no_ft = d.clone();
        no_ft["securityConfiguration"]["fastRoamingEnabled"] = json!(false);
        assert!(check(Field::Wpa3FastRoaming, &p(json!(true)), &no_ft).is_err());
        assert!(check(Field::PmfMode, &p(json!("REQUIRED")), &d).is_ok());
        assert!(check(Field::PmfMode, &p(json!("DISABLED")), &d).is_err());
        let h = |v| check(Field::HandoffSuggestions, &p(v), &d);
        assert!(h(json!({"band5GHzRssiThreshold": -75, "band6GHzRssiThreshold": null})).is_ok());
        assert!(h(json!({"band5GHzRssiThreshold": -85})).is_err());
        assert!(h(json!({"other": 1})).is_err());
        assert!(h(Value::Null).is_ok());
        let f = |v| check(Field::Frequencies, &p(v), &d);
        assert!(f(json!([5])).is_ok());
        assert!(f(json!([5, 6.0])).is_ok());
        assert!(f(json!([])).is_err());
        assert!(f(json!([5, 5])).is_err());
        assert!(f(json!([3])).is_err());
        let t = |v| check(Field::DtimOverride, &p(v), &d);
        assert!(t(json!({"2.4": 3, "5": 1, "6": 1})).is_ok());
        // `dtimPeriod2gLockedTo3` is true in the detail object.
        assert!(t(json!({"2.4": 1, "5": 1, "6": 1})).is_err());
        assert!(t(json!({"5": 1, "6": 1})).is_err());
        assert!(t(json!({"2.4": 3, "5": 0, "6": 1})).is_err());
        assert!(check(Field::DtimOverride, &Slot::absent(), &d).is_ok());
    }

    #[test]
    fn credentials() {
        let mut d = detail();
        assert!(credentials_readable(&d));
        assert_eq!(not_writable(&d), None);
        d["securityConfiguration"]["passphrase"] = json!("********");
        assert_eq!(not_writable(&d), Some("secret_not_readable"));
        d["securityConfiguration"]
            .as_object_mut()
            .unwrap()
            .remove("passphrase");
        assert!(!credentials_readable(&d));
        let ppsk = json!({"type": "STANDARD", "metadata": {"origin": "USER_DEFINED"},
            "securityConfiguration": {"type": "WPA2_PERSONAL", "presharedKeys": [
                {"network": {"type": "NATIVE"}, "passphrase": "first-key-1"},
                {"network": {"type": "NATIVE"}, "passphrase": "••••••••"}]}});
        assert!(!credentials_readable(&ppsk));
        let open = json!({"type": "STANDARD", "metadata": {"origin": "USER_DEFINED"},
            "securityConfiguration": {"type": "OPEN"}});
        assert_eq!(not_writable(&open), None);
        let mut iot = detail();
        iot["type"] = json!("IOT_OPTIMIZED");
        assert_eq!(not_writable(&iot), Some("ssid_type"));
        let mut orchestrated = detail();
        orchestrated["metadata"]["origin"] = json!("ORCHESTRATED");
        assert_eq!(not_writable(&orchestrated), Some("ssid_origin"));
    }

    #[test]
    fn merge_and_differences() {
        let d = detail();
        let mut body = update_body(&d);
        assert!(body.get("id").is_none() && body.get("metadata").is_none());
        merge(
            &mut body,
            &[
                (Field::Uapsd, Slot::Present { value: json!(true) }),
                (Field::Wpa3FastRoaming, Slot::Present { value: json!(true) }),
                (
                    Field::HandoffSuggestions,
                    Slot::Present {
                        value: json!({"band5GHzRssiThreshold": -75}),
                    },
                ),
                (Field::PmfMode, Slot::absent()),
            ],
        );
        let diff = differences(&update_body(&d), &body);
        assert_eq!(
            diff,
            [
                "/handoffSuggestionsConfiguration",
                "/securityConfiguration/pmfMode",
                "/securityConfiguration/wpa3FastRoamingEnabled",
                "/uapsdEnabled"
            ]
        );
        assert!(diff.iter().all(|p| allowed_path(p)));
        assert_eq!(body["securityConfiguration"]["passphrase"], "correct-horse");
        // Numbers compare as numbers.
        assert!(differences(&json!({"f": [5, 2.4]}), &json!({"f": [5.0, 2.4]})).is_empty());
        assert_ne!(hash(&d), hash(&body));
    }

    #[test]
    fn versions() {
        assert!(version_at_least("10.6.106", WRITE_MIN_VERSION));
        assert!(version_at_least("11.0.89", WRITE_MIN_VERSION));
        assert!(version_at_least("10.10.1", WRITE_MIN_VERSION));
        assert!(!version_at_least("10.6.105", WRITE_MIN_VERSION));
        assert!(!version_at_least("9.4.19", WRITE_MIN_VERSION));
        assert!(!version_at_least("", WRITE_MIN_VERSION));
        assert!(!version_at_least("abc", WRITE_MIN_VERSION));
    }
}
