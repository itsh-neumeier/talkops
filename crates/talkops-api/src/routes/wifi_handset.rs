//! Wi-Fi handset roaming threshold and band protection (ADR 0043).
//!
//! The survey proposal also proposes the roaming threshold of the survey
//! handset. Applying it stores the phone setting and asks the handset to
//! fetch its configuration (`check-sync`). The change waits in the history
//! until the handset fetches it. Before TalkOps removes a band from the
//! voice SSID, the band check reads the radio of each Wi-Fi handset from
//! UniFi and refuses a change that would disconnect one.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use talkops_core::extensions::{self, DeviceKind};
use talkops_core::phones::{self, Phone};
use talkops_core::roles::Permission;
use talkops_core::unifi_changes::{self as changes, NewChange, Outcome, WifiChange};
use talkops_core::wifi_survey::{self as ws, Survey};
use talkops_provisioning::SettingValues;
use talkops_unifi::allowlist::{Slot, Snapshot};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::unifi_write::{Refusal, Result, audit, foreign, gate, readable_survey};
use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;

/// Catalog key of the roaming threshold (`static.network.wifi.roaming_threshold`).
pub const ROAMING_THRESHOLD: &str = "wifi_roaming_threshold";

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(apply_handset_change))
}

/// Proposed roaming threshold: the survey threshold for a sticky client plus
/// 5 dB, so the handset scans before the access point pushes it away.
pub fn proposed_threshold(recommendations: &[&str], sticky_rssi: i32) -> Option<i32> {
    recommendations
        .contains(&"min_rssi")
        .then(|| (sticky_rssi + 5).clamp(-75, -60))
}

/// Whether the user can change the handset now, and if not, why.
#[derive(Debug, Serialize, ToSchema)]
pub struct HandsetWriteState {
    pub allowed: bool,
    /// `permission`, `survey_active` or `change_in_progress`.
    pub reason: Option<&'static str>,
}

/// Proposed roaming threshold for the survey handset.
#[derive(Debug, Serialize, ToSchema)]
pub struct HandsetProposal {
    pub phone_id: Uuid,
    pub phone_name: String,
    pub model: String,
    /// Catalog key, `wifi_roaming_threshold`.
    pub setting: &'static str,
    /// Own value of the phone (`{"absent": true}` if it uses the default).
    #[schema(value_type = Object)]
    pub current: Slot,
    /// Value the handset has now: own value, tenant value or factory value.
    pub effective: String,
    pub proposed: String,
    /// Recommendation ID that caused the change.
    pub recommendation: &'static str,
    pub write: HandsetWriteState,
}

async fn handset_write_state(state: &AppState, auth: &AuthUser) -> Result<HandsetWriteState> {
    let reason = if !auth.can(Permission::UnifiWrite) || !auth.can(Permission::PhonesManage) {
        Some("permission")
    } else if ws::any_active(&state.db, auth.tenant).await? {
        Some("survey_active")
    } else if changes::applying(&state.db, auth.tenant).await? {
        Some("change_in_progress")
    } else {
        None
    };
    Ok(HandsetWriteState {
        allowed: reason.is_none(),
        reason,
    })
}

/// The phone of a survey if it is a provisioned handset that takes the
/// roaming threshold.
async fn survey_handset(state: &AppState, auth: &AuthUser, phone: Option<Uuid>) -> Option<Phone> {
    let phone = phones::get(&state.db, auth.tenant, phone?).await.ok()?;
    let model = state.phone_catalog.get(&phone.model)?;
    state
        .phone_catalog
        .settings()
        .iter()
        .any(|s| {
            s.key == ROAMING_THRESHOLD
                && s.vendor == model.vendor
                && s.families.contains(&model.family)
                && (s.models.is_empty() || s.models.contains(&model.id))
        })
        .then_some(phone)
}

fn own_value(phone: &Phone) -> Option<String> {
    phone
        .settings
        .get(ROAMING_THRESHOLD)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn slot(value: Option<&str>) -> Slot {
    match value {
        Some(v) => Slot::Present { value: json!(v) },
        None => Slot::absent(),
    }
}

/// The handset part of the survey proposal; `None` if nothing changes.
pub async fn handset_proposal(
    state: &AppState,
    auth: &AuthUser,
    survey: &Survey,
    recommendations: &[&str],
) -> Result<Option<HandsetProposal>> {
    let Some(proposed) = proposed_threshold(recommendations, survey.thresholds().sticky_rssi)
    else {
        return Ok(None);
    };
    let Some(phone) = survey_handset(state, auth, survey.phone_id).await else {
        return Ok(None);
    };
    let own = own_value(&phone);
    let tenant = phones::phone_defaults(&state.db, auth.tenant).await?;
    let factory = state
        .phone_catalog
        .settings()
        .iter()
        .find(|s| s.key == ROAMING_THRESHOLD)
        .and_then(|s| s.default.clone());
    let effective = own
        .clone()
        .or_else(|| {
            tenant
                .get(ROAMING_THRESHOLD)
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or(factory)
        .unwrap_or_default();
    if effective == proposed.to_string() {
        return Ok(None);
    }
    Ok(Some(HandsetProposal {
        phone_id: phone.id,
        phone_name: phone.name.clone(),
        model: phone.model.clone(),
        setting: ROAMING_THRESHOLD,
        current: slot(own.as_deref()),
        effective,
        proposed: proposed.to_string(),
        recommendation: "min_rssi",
        write: handset_write_state(state, auth).await?,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct HandsetApplyInput {
    pub survey_id: Option<Uuid>,
    pub phone_id: Uuid,
    /// New roaming threshold (dBm), e.g. `"-72"`.
    pub value: String,
    /// The phone's own value when the proposal was made (`null` = unset).
    pub base_value: Option<String>,
    /// The user confirmed the diff dialog.
    #[serde(default)]
    pub confirm: bool,
}

/// Stores the roaming threshold of a handset and asks it to fetch its
/// configuration. The change waits until the handset fetches it.
#[utoipa::path(post, path = "/api/v1/wifi-handset-changes", tag = "wifi-survey", request_body = HandsetApplyInput, responses((status = 201, body = WifiChange)))]
pub async fn apply_handset_change(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<HandsetApplyInput>,
) -> Result<(StatusCode, Json<WifiChange>)> {
    if !auth.can(Permission::UnifiWrite) || !auth.can(Permission::PhonesManage) {
        return Err(ApiError::Forbidden.into());
    }
    if !input.confirm {
        return Err(ApiError::BadRequest("confirm the change in the diff dialog".into()).into());
    }
    if let Some(survey) = input.survey_id {
        readable_survey(&state, &auth, survey).await?;
    }
    let phone = survey_handset(&state, &auth, Some(input.phone_id))
        .await
        .ok_or_else(|| gate("not_wifi_handset"))?;
    state
        .phone_catalog
        .check_settings(&SettingValues::from([(
            ROAMING_THRESHOLD.to_owned(),
            input.value.clone(),
        )]))
        .map_err(ApiError::BadRequest)?;
    if let Some(reason) = handset_write_state(&state, &auth).await?.reason {
        return Err(gate(reason));
    }
    if own_value(&phone) != input.base_value {
        return Err(gate("handset_changed"));
    }
    let row = change_handset(
        &state,
        &auth,
        &phone,
        Some(&input.value),
        None,
        input.survey_id,
    )
    .await?;
    audit(
        &state,
        &auth,
        "wifi.handset.apply",
        Some(row.id),
        json!({
            "phone_id": phone.id,
            "phone_name": phone.name,
            "survey_id": input.survey_id,
            "fields": row.fields,
            "before": row.before,
            "after": row.after,
        }),
    )
    .await;
    Ok((StatusCode::CREATED, Json(row)))
}

/// Writes the setting, records the change as `waiting` and sends `check-sync`.
async fn change_handset(
    state: &AppState,
    auth: &AuthUser,
    phone: &Phone,
    value: Option<&str>,
    rollback_of: Option<Uuid>,
    survey_id: Option<Uuid>,
) -> Result<WifiChange> {
    let id = changes::begin(
        &state.db,
        auth.tenant,
        &NewChange {
            target: "handset",
            site_id: None,
            wifi_id: None,
            wifi_name: "",
            phone_id: Some(phone.id),
            phone_name: &phone.name,
            kind: if rollback_of.is_some() {
                "rollback"
            } else {
                "apply"
            },
            rollback_of,
            survey_id,
            fields: vec![ROAMING_THRESHOLD.to_owned()],
            network_version: None,
            created_by: Some(auth.id),
        },
    )
    .await
    .map_err(|e| match e {
        talkops_core::error::CoreError::Conflict(_) => gate("change_in_progress"),
        other => other.into(),
    })?;
    let old = match phones::set_setting(&state.db, auth.tenant, phone.id, ROAMING_THRESHOLD, value)
        .await
    {
        Ok(old) => old,
        Err(e) => {
            let _ = changes::finish(
                &state.db,
                auth.tenant,
                id,
                &Outcome {
                    state: changes::state::FAILED,
                    wifi_name: None,
                    before: None,
                    after: None,
                    error: Some("internal"),
                },
            )
            .await;
            return Err(e.into());
        }
    };
    let before = snapshot(old.as_deref());
    let after = snapshot(value);
    changes::finish(
        &state.db,
        auth.tenant,
        id,
        &Outcome {
            state: changes::state::WAITING,
            wifi_name: None,
            before: Some(&before),
            after: Some(&after),
            error: None,
        },
    )
    .await?;
    // One NOTIFY is enough; try the accounts until one is reachable. An
    // unreachable handset fetches the value at its next interval.
    for user in phones::device_usernames(&state.db, phone.id).await? {
        if state.telephony.check_sync(&user, false).await {
            break;
        }
    }
    Ok(changes::get(&state.db, auth.tenant, id).await?)
}

fn snapshot(value: Option<&str>) -> Value {
    json!({ ROAMING_THRESHOLD: slot(value) })
}

fn from_json(v: Option<&Value>) -> Option<Slot> {
    serde_json::from_value(v?.get(ROAMING_THRESHOLD)?.clone()).ok()
}

/// Rollback of a handset row: restores the earlier value.
pub async fn rollback_handset(
    state: &AppState,
    auth: &AuthUser,
    change: &WifiChange,
    accept_foreign_changes: bool,
) -> Result<(StatusCode, Json<WifiChange>)> {
    if !auth.can(Permission::PhonesManage) {
        return Err(ApiError::Forbidden.into());
    }
    let (Some(before), Some(after)) = (
        from_json(change.before.as_ref()),
        from_json(change.after.as_ref()),
    ) else {
        return Err(gate("not_revertible"));
    };
    let phone = survey_handset(state, auth, change.phone_id)
        .await
        .ok_or_else(|| gate("not_revertible"))?;
    if let Some(reason) = handset_write_state(state, auth).await?.reason {
        return Err(gate(reason));
    }
    let current = slot(own_value(&phone).as_deref());
    if !accept_foreign_changes && !current.same(&after) {
        let key = ROAMING_THRESHOLD.to_owned();
        return Err(Refusal::Foreign(foreign(
            &Snapshot::from([(key.clone(), after)]),
            Some(&Snapshot::from([(key.clone(), before)])),
            &Snapshot::from([(key, current)]),
        )));
    }
    let value = before.value().and_then(Value::as_str).map(str::to_owned);
    let row = change_handset(
        state,
        auth,
        &phone,
        value.as_deref(),
        Some(change.id),
        change.survey_id,
    )
    .await?;
    changes::mark_rolled_back(&state.db, auth.tenant, change.id).await?;
    audit(
        state,
        auth,
        "wifi.handset.rollback",
        Some(row.id),
        json!({
            "rollback_of": change.id,
            "phone_id": phone.id,
            "phone_name": phone.name,
            "fields": row.fields,
            "before": row.before,
            "after": row.after,
            "foreign_changes_accepted": accept_foreign_changes,
        }),
    )
    .await;
    Ok((StatusCode::CREATED, Json(row)))
}

/// Band check result for one Wi-Fi handset.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HandsetBand {
    pub phone_id: Uuid,
    pub name: String,
    /// `ok`, `blocked` (connected to the voice SSID on a removed band) or
    /// `unknown` (UniFi does not report the handset).
    pub state: &'static str,
    /// Band of the connection in GHz (`2.4`, `5`, `6`), if connected.
    pub band: Option<f64>,
}

fn radio_band(radio: &str) -> Option<f64> {
    match radio {
        "ng" => Some(2.4),
        "na" => Some(5.0),
        "6e" => Some(6.0),
        _ => None,
    }
}

/// Bands in `current` that are not in `new` (`broadcastingFrequenciesGHz`).
pub fn removed_bands(current: Option<&Value>, new: Option<&Value>) -> Vec<f64> {
    let list = |v: Option<&Value>| -> Vec<f64> {
        v.and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default()
    };
    let new = list(new);
    list(current)
        .into_iter()
        .filter(|b| !new.contains(b))
        .collect()
}

/// Reads the band of each Wi-Fi handset of the tenant from UniFi.
pub async fn band_check(
    state: &AppState,
    auth: &AuthUser,
    client: &talkops_unifi::Client,
    ssid_name: &str,
    removed: &[f64],
) -> Result<Vec<HandsetBand>> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for d in extensions::list_all_devices(&state.db, auth.tenant).await? {
        let Some(phone_id) = d.phone_id.filter(|_| d.kind == DeviceKind::Wifi) else {
            continue;
        };
        if seen.contains(&phone_id) {
            continue;
        }
        seen.push(phone_id);
        let Ok(phone) = phones::get(&state.db, auth.tenant, phone_id).await else {
            continue;
        };
        let station = client.station(&phone.mac).await.ok().flatten();
        let band = station
            .as_ref()
            .and_then(|s| s.radio.as_deref())
            .and_then(radio_band);
        let on_voice = station
            .as_ref()
            .and_then(|s| s.essid.as_deref())
            .is_none_or(|e| e == ssid_name);
        let state = match band {
            None => "unknown",
            Some(b) if on_voice && removed.contains(&b) => "blocked",
            Some(_) => "ok",
        };
        out.push(HandsetBand {
            phone_id,
            name: phone.name,
            state,
            band,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Refuses a band removal that would disconnect a handset. Unknown handsets
/// need an explicit confirmation each.
pub fn check_bands(list: &[HandsetBand], confirmed: &[Uuid]) -> Result<()> {
    if list.iter().any(|h| h.state == "blocked") {
        return Err(Refusal::Handsets("handset_band", list.to_vec()));
    }
    if list
        .iter()
        .any(|h| h.state == "unknown" && !confirmed.contains(&h.phone_id))
    {
        return Err(Refusal::Handsets("handset_unknown", list.to_vec()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_and_bands() {
        assert_eq!(proposed_threshold(&["min_rssi"], -75), Some(-70));
        assert_eq!(proposed_threshold(&["min_rssi"], -85), Some(-75));
        assert_eq!(proposed_threshold(&["min_rssi"], -50), Some(-60));
        assert_eq!(
            proposed_threshold(&["fast_roaming", "voice_ssid"], -75),
            None
        );
        assert_eq!(
            removed_bands(Some(&json!([2.4, 5])), Some(&json!([5]))),
            [2.4]
        );
        assert!(removed_bands(Some(&json!([5])), Some(&json!([2.4, 5]))).is_empty());
        assert!(removed_bands(None, Some(&json!([5]))).is_empty());
        assert_eq!(radio_band("ng"), Some(2.4));
        assert_eq!(radio_band("na"), Some(5.0));
        assert_eq!(radio_band("x"), None);
        let h = |state| HandsetBand {
            phone_id: Uuid::nil(),
            name: "AX83H".into(),
            state,
            band: None,
        };
        assert!(check_bands(&[h("ok")], &[]).is_ok());
        assert!(check_bands(&[h("blocked")], &[Uuid::nil()]).is_err());
        assert!(check_bands(&[h("unknown")], &[]).is_err());
        assert!(check_bands(&[h("unknown")], &[Uuid::nil()]).is_ok());
    }
}
