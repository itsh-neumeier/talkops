//! Proposal: survey recommendations to changes of the voice SSID (ADR 0042).
//!
//! A pure function. It reads the current object and the recommendation IDs
//! of `talkops_core::wifi_survey::analysis` and writes nothing.

use serde::Serialize;
use serde_json::{Value, json};

use crate::allowlist::{self, Field, Slot};

/// One proposed field change.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    pub field: Field,
    pub current: Slot,
    pub proposed: Value,
    /// Recommendation ID that caused the change.
    pub recommendation: &'static str,
    /// Selected in the diff dialog when it opens.
    pub selected: bool,
    /// Extra warning ID for the dialog (`fast_roaming_compat`, `band_2g_removed`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<&'static str>,
}

/// A recommendation that TalkOps does not write in this stage.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hint {
    /// `fast_roaming_80211r`, `fast_roaming_80211k`, `min_data_rate`,
    /// `tx_power`, `pingpong`, `coverage`.
    pub id: &'static str,
    pub recommendation: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Proposal {
    pub changes: Vec<Change>,
    pub hints: Vec<Hint>,
}

/// Input from the survey.
pub struct Input<'a> {
    /// Recommendation IDs of the survey result.
    pub recommendations: &'a [&'a str],
    /// Survey threshold for a sticky client (dBm).
    pub sticky_rssi: i32,
}

/// DTIM period that TalkOps proposes for a voice SSID.
const VOICE_DTIM: i64 = 1;

pub fn build(obj: &Value, input: &Input) -> Proposal {
    let mut p = Proposal::default();
    let available = allowlist::available(obj);
    let has = |r: &str| input.recommendations.contains(&r);
    let mut propose = |field: Field,
                       value: Value,
                       rec: &'static str,
                       warning: Option<&'static str>,
                       selected: bool| {
        if !available.contains(&field) {
            return;
        }
        let current = allowlist::read(obj, field);
        let new = Slot::Present {
            value: value.clone(),
        };
        if current.same(&new) || allowlist::check(field, &new, obj).is_err() {
            return;
        }
        if p.changes.iter().any(|c| c.field == field) {
            return;
        }
        p.changes.push(Change {
            field,
            current,
            proposed: value,
            recommendation: rec,
            selected,
            warning,
        });
    };
    let ft = obj.pointer("/securityConfiguration/fastRoamingEnabled") == Some(&Value::Bool(true));

    if has("fast_roaming") {
        propose(
            Field::BssTransition,
            json!(true),
            "fast_roaming",
            None,
            true,
        );
        if ft {
            propose(
                Field::Wpa3FastRoaming,
                json!(true),
                "fast_roaming",
                Some("fast_roaming_compat"),
                true,
            );
        }
    }
    if has("min_rssi") {
        let freqs = frequencies(obj);
        if freqs.contains(&5.0) {
            let mut h = obj
                .get("handoffSuggestionsConfiguration")
                .filter(|v| v.is_object())
                .cloned()
                .unwrap_or_else(|| json!({}));
            h["band5GHzRssiThreshold"] = json!(input.sticky_rssi.clamp(-80, -60));
            if h.get("band6GHzRssiThreshold").is_none() {
                h["band6GHzRssiThreshold"] = Value::Null;
            }
            propose(Field::HandoffSuggestions, h, "min_rssi", None, true);
        }
    }
    if has("band_5ghz") {
        if let Some(list) = obj
            .get("broadcastingFrequenciesGHz")
            .and_then(Value::as_array)
        {
            let keep: Vec<Value> = list
                .iter()
                .filter(|v| v.as_f64() != Some(2.4))
                .cloned()
                .collect();
            if keep.iter().any(|v| v.as_f64() == Some(5.0)) {
                // Off by default: 2.4 GHz only clients lose the SSID.
                propose(
                    Field::Frequencies,
                    Value::Array(keep),
                    "band_5ghz",
                    Some("band_2g_removed"),
                    false,
                );
            }
        }
    }
    if has("voice_ssid") {
        propose(Field::Uapsd, json!(true), "voice_ssid", None, true);
        propose(
            Field::MulticastToUnicast,
            json!(true),
            "voice_ssid",
            None,
            true,
        );
        propose(Field::ArpProxy, json!(true), "voice_ssid", None, true);
        let locked = obj.get("dtimPeriod2gLockedTo3") == Some(&Value::Bool(true));
        let dtim_24 = if locked { 3 } else { VOICE_DTIM };
        propose(
            Field::DtimOverride,
            json!({"2.4": dtim_24, "5": VOICE_DTIM, "6": VOICE_DTIM}),
            "voice_ssid",
            None,
            true,
        );
        // Only from "off" to "optional"; TalkOps never lowers "required".
        if obj
            .pointer("/securityConfiguration/pmfMode")
            .is_none_or(Value::is_null)
        {
            propose(Field::PmfMode, json!("OPTIONAL"), "voice_ssid", None, true);
        }
    }

    let mut hint = |id, rec| {
        p.hints.push(Hint {
            id,
            recommendation: rec,
        })
    };
    if has("fast_roaming") {
        if !ft {
            hint("fast_roaming_80211r", "fast_roaming");
        }
        hint("fast_roaming_80211k", "fast_roaming");
    }
    if has("voice_ssid") {
        hint("min_data_rate", "voice_ssid");
    }
    if has("coverage") {
        hint("tx_power", "coverage");
        hint("coverage", "coverage");
    }
    if has("pingpong") {
        hint("pingpong", "pingpong");
    }
    p
}

fn frequencies(obj: &Value) -> Vec<f64> {
    obj.get("broadcastingFrequenciesGHz")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail() -> Value {
        json!({
            "type": "STANDARD", "id": "6f1c", "name": "Voice", "enabled": true,
            "metadata": {"origin": "USER_DEFINED"},
            "securityConfiguration": {"type": "WPA2_WPA3_PERSONAL", "passphrase": "correct-horse",
                "pmfMode": null, "fastRoamingEnabled": false, "wpa3FastRoamingEnabled": false},
            "broadcastingFrequenciesGHz": [2.4, 5], "bssTransitionEnabled": false,
            "uapsdEnabled": false, "multicastToUnicastConversionEnabled": true,
            "arpProxyEnabled": false, "channel2gLockedTo6": false, "dtimPeriod2gLockedTo3": false
        })
    }

    fn keys(p: &Proposal) -> Vec<&'static str> {
        p.changes.iter().map(|c| c.field.key()).collect()
    }

    #[test]
    fn maps_each_recommendation() {
        let all = [
            "fast_roaming",
            "min_rssi",
            "band_5ghz",
            "voice_ssid",
            "coverage",
            "pingpong",
            "interference",
        ];
        let p = build(
            &detail(),
            &Input {
                recommendations: &all,
                sticky_rssi: -85,
            },
        );
        assert_eq!(
            keys(&p),
            [
                "bssTransitionEnabled",
                "handoffSuggestionsConfiguration",
                "broadcastingFrequenciesGHz",
                "uapsdEnabled",
                "arpProxyEnabled",
                "dtimPeriodByFrequencyGHzOverride",
                "securityConfiguration.pmfMode",
            ]
        );
        // Multicast to unicast is already on; no change.
        let handoff = &p.changes[1];
        assert_eq!(
            handoff.proposed,
            json!({"band5GHzRssiThreshold": -80, "band6GHzRssiThreshold": null})
        );
        let band = &p.changes[2];
        assert_eq!(band.proposed, json!([5]));
        assert!(!band.selected);
        assert_eq!(band.warning, Some("band_2g_removed"));
        assert_eq!(p.changes[5].proposed, json!({"2.4": 1, "5": 1, "6": 1}));
        assert_eq!(p.changes[6].proposed, json!("OPTIONAL"));
        let hints: Vec<&str> = p.hints.iter().map(|h| h.id).collect();
        assert_eq!(
            hints,
            [
                "fast_roaming_80211r",
                "fast_roaming_80211k",
                "min_data_rate",
                "tx_power",
                "coverage",
                "pingpong"
            ]
        );
        // The proposal contains no secret.
        assert!(!serde_json::to_string(&p).unwrap().contains("correct-horse"));
    }

    #[test]
    fn wpa3_fast_roaming_needs_80211r() {
        let mut d = detail();
        d["securityConfiguration"]["fastRoamingEnabled"] = json!(true);
        let p = build(
            &d,
            &Input {
                recommendations: &["fast_roaming"],
                sticky_rssi: -75,
            },
        );
        assert_eq!(
            keys(&p),
            [
                "bssTransitionEnabled",
                "securityConfiguration.wpa3FastRoamingEnabled"
            ]
        );
        assert_eq!(p.changes[1].warning, Some("fast_roaming_compat"));
        assert!(p.hints.iter().all(|h| h.id != "fast_roaming_80211r"));
    }

    #[test]
    fn respects_locks_and_existing_values() {
        let mut d = detail();
        d["dtimPeriod2gLockedTo3"] = json!(true);
        d["securityConfiguration"]["pmfMode"] = json!("REQUIRED");
        d["handoffSuggestionsConfiguration"] =
            json!({"band5GHzRssiThreshold": -70, "band6GHzRssiThreshold": -80});
        let p = build(
            &d,
            &Input {
                recommendations: &["voice_ssid", "min_rssi"],
                sticky_rssi: -75,
            },
        );
        let dtim = p
            .changes
            .iter()
            .find(|c| c.field == Field::DtimOverride)
            .unwrap();
        assert_eq!(dtim.proposed, json!({"2.4": 3, "5": 1, "6": 1}));
        assert!(p.changes.iter().all(|c| c.field != Field::PmfMode));
        let h = p
            .changes
            .iter()
            .find(|c| c.field == Field::HandoffSuggestions)
            .unwrap();
        assert_eq!(
            h.proposed,
            json!({"band5GHzRssiThreshold": -75, "band6GHzRssiThreshold": -80})
        );
    }

    #[test]
    fn older_schema_and_single_band() {
        // Before 10.6.106: no handoff or DTIM override fields.
        let mut d = detail();
        d.as_object_mut().unwrap().remove("channel2gLockedTo6");
        d["broadcastingFrequenciesGHz"] = json!([2.4]);
        let p = build(
            &d,
            &Input {
                recommendations: &["min_rssi", "band_5ghz", "voice_ssid"],
                sticky_rssi: -75,
            },
        );
        assert_eq!(
            keys(&p),
            [
                "uapsdEnabled",
                "arpProxyEnabled",
                "securityConfiguration.pmfMode"
            ]
        );
    }
}
