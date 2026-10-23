//! Changes to the UniFi voice SSID (ADR 0042).
//!
//! `unifi.manage` selects the voice SSID and reads proposals and the change
//! history. `unifi.write` (sensitive) switches writes on, applies changes
//! and rolls them back. TalkOps writes only when the write switch is on,
//! the Network version is 10.6.106 or later, no survey of the tenant is
//! active and the user confirmed the values in the diff dialog. Audit
//! entries and stored rows contain only allowlist values, never secrets.
//!
//! The proposal also carries the handset part and the band check of
//! ADR 0043 (`wifi_handset`). A change that removes a band from the voice
//! SSID passes the band check first.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use talkops_core::audit;
use talkops_core::error::CoreError;
use talkops_core::roles::Permission;
use talkops_core::unifi::{self, UnifiConnection};
use talkops_core::unifi_changes::{self as changes, NewChange, Outcome, WifiChange};
use talkops_core::wifi_survey as ws;
use talkops_unifi::allowlist::{self, Field, Slot, Snapshot, WRITE_MIN_VERSION};
use talkops_unifi::write::{self, Guard, WriteError};
use talkops_unifi::{UnifiError, WifiSummary, proposal};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::routes::wifi_handset::{self as handset, HandsetBand, HandsetProposal};
use crate::routes::wifi_survey::UnifiView;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(wifi_broadcasts))
        .routes(routes!(put_voice_ssid))
        .routes(routes!(survey_proposal))
        .routes(routes!(list_changes, apply_change))
        .routes(routes!(rollback_change))
}

/// An error of this area: an API error or a refused write with a code.
pub enum Refusal {
    Api(ApiError),
    /// HTTP status, machine-readable code, English message.
    Gate(StatusCode, &'static str, String),
    /// Rollback: fields that changed in UniFi after the TalkOps change.
    Foreign(Vec<ForeignField>),
    /// Band check: a code (`handset_band`, `handset_unknown`) and the result
    /// for each Wi-Fi handset.
    Handsets(&'static str, Vec<HandsetBand>),
}

impl From<ApiError> for Refusal {
    fn from(e: ApiError) -> Self {
        Refusal::Api(e)
    }
}

impl From<CoreError> for Refusal {
    fn from(e: CoreError) -> Self {
        Refusal::Api(e.into())
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        match self {
            Refusal::Api(e) => e.into_response(),
            Refusal::Gate(status, error, message) => {
                (status, Json(json!({ "error": error, "message": message }))).into_response()
            }
            Refusal::Foreign(fields) => (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "foreign_change",
                    "message": "the SSID changed in UniFi after this change",
                    "fields": fields,
                })),
            )
                .into_response(),
            Refusal::Handsets(code, handsets) => (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": code,
                    "message": if code == "handset_band" {
                        "a Wi-Fi handset is connected on a band that the change removes"
                    } else {
                        "confirm each Wi-Fi handset that UniFi does not report"
                    },
                    "handsets": handsets,
                })),
            )
                .into_response(),
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, Refusal>;

pub(crate) fn gate(code: &'static str) -> Refusal {
    let (status, message) = match code {
        "write_disabled" => (StatusCode::FORBIDDEN, "write access to UniFi is off"),
        "permission" => (StatusCode::FORBIDDEN, "insufficient permissions"),
        "version_unsupported" => (
            StatusCode::CONFLICT,
            "UniFi Network 10.6.106 or later is necessary; run the connection test",
        ),
        "no_voice_ssid" => (StatusCode::CONFLICT, "select the voice SSID first"),
        "survey_active" => (StatusCode::CONFLICT, "a Wi-Fi survey is active"),
        "change_in_progress" => (StatusCode::CONFLICT, "another UniFi change is in progress"),
        "ssid_changed" => (
            StatusCode::CONFLICT,
            "the SSID changed in UniFi since the proposal; open the proposal again",
        ),
        "ssid_not_voice" => (StatusCode::CONFLICT, "the SSID is not the voice SSID"),
        "ssid_type" => (StatusCode::CONFLICT, "TalkOps writes only standard SSIDs"),
        "ssid_origin" => (
            StatusCode::CONFLICT,
            "the SSID is managed centrally (not user-defined)",
        ),
        "secret_not_readable" => (
            StatusCode::CONFLICT,
            "UniFi did not return the SSID credentials; a full update would remove them",
        ),
        "not_revertible" => (StatusCode::CONFLICT, "this change cannot be rolled back"),
        "unifi_forbidden" => (
            StatusCode::BAD_GATEWAY,
            "the UniFi API key may read but not write",
        ),
        "verify_mismatch" => (
            StatusCode::BAD_GATEWAY,
            "UniFi did not apply all values; check the change history",
        ),
        "not_wifi_handset" => (
            StatusCode::CONFLICT,
            "the phone is not a provisioned Wi-Fi handset with a roaming threshold",
        ),
        "handset_changed" => (
            StatusCode::CONFLICT,
            "the handset setting changed since the proposal; open the proposal again",
        ),
        _ => (StatusCode::CONFLICT, "the change is not possible"),
    };
    Refusal::Gate(status, code, message.to_owned())
}

fn unifi_error(e: UnifiError) -> Refusal {
    Refusal::Api(ApiError::Device(e.to_string()))
}

/// The UniFi client for the stored connection.
struct Unifi {
    conn: UnifiConnection,
    client: talkops_unifi::Client,
    site_id: String,
}

async fn connect(state: &AppState, auth: &AuthUser) -> Result<Unifi> {
    let conn = unifi::get(&state.db, auth.tenant)
        .await?
        .ok_or_else(|| ApiError::Conflict("connect UniFi first".into()))?;
    let key = unifi::api_key(&state.db, auth.tenant, &state.secrets).await?;
    let client = talkops_unifi::Client::new(talkops_unifi::Config {
        url: conn.url.clone(),
        site: conn.site.clone(),
        api_key: key,
        cert_sha256: conn.cert_sha256.clone(),
    })
    .map_err(unifi_error)?;
    let site_id = match conn.site_id() {
        Some(s) => s.to_owned(),
        None => client
            .site_id()
            .await
            .map_err(unifi_error)?
            .ok_or_else(|| ApiError::Conflict("run the UniFi connection test first".into()))?,
    };
    Ok(Unifi {
        conn,
        client,
        site_id,
    })
}

fn version_ok(conn: &UnifiConnection) -> bool {
    conn.version
        .as_deref()
        .is_some_and(|v| allowlist::version_at_least(v, WRITE_MIN_VERSION))
}

/// Whether the user can apply a change now, and if not, why.
#[derive(Debug, Serialize, ToSchema)]
pub struct WriteState {
    pub allowed: bool,
    /// `write_disabled`, `permission`, `version_unsupported`,
    /// `no_voice_ssid`, `survey_active` or `change_in_progress`.
    pub reason: Option<&'static str>,
    pub version: Option<String>,
    pub min_version: &'static str,
}

async fn write_state(
    state: &AppState,
    auth: &AuthUser,
    conn: &UnifiConnection,
) -> Result<WriteState> {
    let reason = if !conn.write_enabled {
        Some("write_disabled")
    } else if !auth.can(Permission::UnifiWrite) {
        Some("permission")
    } else if !version_ok(conn) {
        Some("version_unsupported")
    } else if conn.voice_wifi_id.is_none() {
        Some("no_voice_ssid")
    } else if ws::any_active(&state.db, auth.tenant).await? {
        Some("survey_active")
    } else if changes::applying(&state.db, auth.tenant).await? {
        Some("change_in_progress")
    } else {
        None
    };
    Ok(WriteState {
        allowed: reason.is_none(),
        reason,
        version: conn.version.clone(),
        min_version: WRITE_MIN_VERSION,
    })
}

pub(crate) async fn audit(
    state: &AppState,
    auth: &AuthUser,
    action: &str,
    id: Option<Uuid>,
    data: Value,
) {
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        action,
        "unifi",
        id.map(|i| i.to_string()),
        data,
    )
    .await;
}

/// SSIDs of the site (overview fields, no secrets).
#[utoipa::path(get, path = "/api/v1/unifi/wifi-broadcasts", tag = "wifi-survey", responses((status = 200, body = Vec<Object>)))]
pub async fn wifi_broadcasts(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<WifiSummary>>> {
    auth.require_perm(Permission::UnifiManage)?;
    let u = connect(&state, &auth).await?;
    Ok(Json(
        u.client
            .wifi_broadcasts(&u.site_id)
            .await
            .map_err(unifi_error)?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VoiceSsidInput {
    /// Integration API ID of the voice SSID; `null` clears it.
    pub wifi_id: Option<String>,
    /// "Allow write access" (needs `unifi.write`).
    #[serde(default)]
    pub write_enabled: bool,
}

/// Selects the voice SSID and sets the write switch.
#[utoipa::path(put, path = "/api/v1/unifi/voice-ssid", tag = "wifi-survey", request_body = VoiceSsidInput, responses((status = 200, body = UnifiView)))]
pub async fn put_voice_ssid(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<VoiceSsidInput>,
) -> Result<Json<UnifiView>> {
    auth.require_perm(Permission::UnifiManage)?;
    let conn = unifi::get(&state.db, auth.tenant)
        .await?
        .ok_or(ApiError::NotFound)?;
    // Writes on, or another SSID while writes are on: only with `unifi.write`.
    let wifi_id = input
        .wifi_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let write_on = input.write_enabled && wifi_id.is_some();
    if (write_on || conn.write_enabled) && !auth.can(Permission::UnifiWrite) {
        return Err(gate("permission"));
    }
    if write_on && !version_ok(&conn) {
        return Err(gate("version_unsupported"));
    }
    let name = match wifi_id {
        Some(id) => {
            talkops_unifi::check_id(id)
                .map_err(|_| ApiError::BadRequest("invalid SSID ID".into()))?;
            let u = connect(&state, &auth).await?;
            let b = u
                .client
                .wifi_broadcast(&u.site_id, id)
                .await
                .map_err(unifi_error)?;
            if write_on {
                if let Some(reason) = allowlist::not_writable(b.object()) {
                    return Err(gate(reason));
                }
            }
            Some(b.name().chars().take(64).collect::<String>())
        }
        None => None,
    };
    let saved = unifi::set_voice_wifi(
        &state.db,
        auth.tenant,
        wifi_id.zip(name.as_deref()),
        write_on,
    )
    .await?;
    audit(
        &state,
        &auth,
        "unifi.voice_ssid",
        None,
        json!({ "wifi_id": saved.voice_wifi_id, "wifi_name": saved.voice_wifi_name }),
    )
    .await;
    if saved.write_enabled != conn.write_enabled {
        audit(
            &state,
            &auth,
            "unifi.write_access",
            None,
            json!({ "enabled": saved.write_enabled, "wifi_id": saved.voice_wifi_id }),
        )
        .await;
    }
    Ok(Json(UnifiView {
        connection: Some(saved),
        has_api_key: true,
    }))
}

/// Proposal for the voice SSID from a survey result. Writes nothing.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProposalView {
    pub wifi_id: String,
    pub wifi_name: String,
    /// Proposed changes: `field`, `current`, `proposed`, `recommendation`,
    /// `selected`, `warning`.
    #[schema(value_type = Vec<Object>)]
    pub changes: Value,
    /// Recommendations that TalkOps does not write: `id`, `recommendation`.
    #[schema(value_type = Vec<Object>)]
    pub hints: Value,
    /// Hash of the allowlist values; the apply request sends it back.
    pub base_hash: String,
    /// Why TalkOps cannot write this SSID (`ssid_type`, `ssid_origin`,
    /// `secret_not_readable`).
    pub not_writable: Option<&'static str>,
    pub write: WriteState,
    /// Band check of the Wi-Fi handsets, if a proposed change removes a band.
    pub band_check: Option<Vec<HandsetBand>>,
    /// Proposed roaming threshold of the survey handset (ADR 0043).
    pub handset: Option<HandsetProposal>,
}

#[utoipa::path(get, path = "/api/v1/wifi-surveys/{id}/unifi-proposal", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 200, body = ProposalView)))]
pub async fn survey_proposal(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProposalView>> {
    auth.require_perm(Permission::UnifiManage)?;
    let survey = readable_survey(&state, &auth, id).await?;
    let summary = survey
        .summary
        .as_ref()
        .ok_or_else(|| ApiError::Conflict("the survey has no result yet".into()))?;
    let ids: Vec<&str> = summary
        .get("recommendations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| r.get("id").and_then(Value::as_str))
        .collect();
    let u = connect(&state, &auth).await?;
    let wifi_id = u
        .conn
        .voice_wifi_id
        .clone()
        .ok_or_else(|| gate("no_voice_ssid"))?;
    let b = u
        .client
        .wifi_broadcast(&u.site_id, &wifi_id)
        .await
        .map_err(unifi_error)?;
    let p = proposal::build(
        b.object(),
        &proposal::Input {
            recommendations: &ids,
            sticky_rssi: survey.thresholds().sticky_rssi,
        },
    );
    let new_bands = p
        .changes
        .iter()
        .find(|c| c.field == Field::Frequencies)
        .map(|c| &c.proposed);
    let removed = handset::removed_bands(b.object().get("broadcastingFrequenciesGHz"), new_bands);
    let band_check = if removed.is_empty() {
        None
    } else {
        Some(handset::band_check(&state, &auth, &u.client, b.name(), &removed).await?)
    };
    Ok(Json(ProposalView {
        band_check,
        handset: handset::handset_proposal(&state, &auth, &survey, &ids).await?,
        wifi_id,
        wifi_name: b.name().to_owned(),
        changes: serde_json::to_value(&p.changes).unwrap_or_default(),
        hints: serde_json::to_value(&p.hints).unwrap_or_default(),
        base_hash: allowlist::hash(b.object()),
        not_writable: allowlist::not_writable(b.object()),
        write: write_state(&state, &auth, &u.conn).await?,
    }))
}

/// Own surveys, or all with `wifi_survey.manage`.
pub(crate) async fn readable_survey(
    state: &AppState,
    auth: &AuthUser,
    id: Uuid,
) -> Result<ws::Survey> {
    auth.require_perm(Permission::WifiSurveyRun)?;
    let s = ws::get(&state.db, auth.tenant, id).await?;
    if s.started_by == Some(auth.id) || auth.can(Permission::WifiSurveyManage) {
        Ok(s)
    } else {
        Err(ApiError::NotFound.into())
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListQuery {
    pub limit: Option<i64>,
}

#[utoipa::path(get, path = "/api/v1/unifi/wifi-changes", tag = "wifi-survey", params(ListQuery), responses((status = 200, body = Vec<WifiChange>)))]
pub async fn list_changes(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<WifiChange>>> {
    auth.require_perm(Permission::UnifiManage)?;
    Ok(Json(
        changes::list(&state.db, auth.tenant, q.limit.unwrap_or(100)).await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FieldValue {
    /// Allowlist key, e.g. `uapsdEnabled`.
    pub field: String,
    #[schema(value_type = Object)]
    pub value: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplyInput {
    pub survey_id: Option<Uuid>,
    pub wifi_id: String,
    /// The rows that the user selected in the diff dialog.
    pub changes: Vec<FieldValue>,
    /// `base_hash` of the proposal.
    pub base_hash: String,
    /// The user confirmed the diff dialog.
    #[serde(default)]
    pub confirm: bool,
    /// Wi-Fi handsets without a UniFi report that the user confirmed for a
    /// band removal (ADR 0043).
    #[serde(default)]
    pub confirm_unknown_handsets: Vec<Uuid>,
}

/// Band check before a change that removes a band from the SSID `wifi_id`.
async fn protect_bands(
    state: &AppState,
    auth: &AuthUser,
    u: &Unifi,
    site_id: &str,
    wifi_id: &str,
    fields: &[(Field, Slot)],
    confirmed: &[Uuid],
) -> Result<()> {
    let Some((_, slot)) = fields.iter().find(|(f, _)| *f == Field::Frequencies) else {
        return Ok(());
    };
    let b = u
        .client
        .wifi_broadcast(site_id, wifi_id)
        .await
        .map_err(unifi_error)?;
    let removed =
        handset::removed_bands(b.object().get("broadcastingFrequenciesGHz"), slot.value());
    if removed.is_empty() {
        return Ok(());
    }
    let list = handset::band_check(state, auth, &u.client, b.name(), &removed).await?;
    handset::check_bands(&list, confirmed)
}

/// Applies confirmed changes to the voice SSID.
#[utoipa::path(post, path = "/api/v1/unifi/wifi-changes", tag = "wifi-survey", request_body = ApplyInput, responses((status = 201, body = WifiChange)))]
pub async fn apply_change(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<ApplyInput>,
) -> Result<(StatusCode, Json<WifiChange>)> {
    auth.require_perm(Permission::UnifiWrite)?;
    if !input.confirm {
        return Err(ApiError::BadRequest("confirm the change in the diff dialog".into()).into());
    }
    let mut fields = Vec::new();
    for c in &input.changes {
        let f = Field::from_key(&c.field)
            .ok_or_else(|| ApiError::BadRequest(format!("{} cannot be changed", c.field)))?;
        fields.push((
            f,
            Slot::Present {
                value: c.value.clone(),
            },
        ));
    }
    if fields.is_empty() {
        return Err(ApiError::BadRequest("select at least one change".into()).into());
    }
    if let Some(survey) = input.survey_id {
        readable_survey(&state, &auth, survey).await?;
    }
    let conn = unifi::get(&state.db, auth.tenant)
        .await?
        .ok_or_else(|| gate("write_disabled"))?;
    if let Some(reason) = write_state(&state, &auth, &conn).await?.reason {
        return Err(gate(reason));
    }
    if conn.voice_wifi_id.as_deref() != Some(input.wifi_id.as_str()) {
        return Err(gate("ssid_not_voice"));
    }
    let u = connect(&state, &auth).await?;
    protect_bands(
        &state,
        &auth,
        &u,
        &u.site_id,
        &input.wifi_id,
        &fields,
        &input.confirm_unknown_handsets,
    )
    .await?;
    let id = begin(
        &state,
        &auth,
        &u,
        (&u.site_id, &input.wifi_id),
        None,
        input.survey_id,
        &fields,
    )
    .await?;
    let result = u
        .client
        .writer()
        .apply(
            &u.site_id,
            &input.wifi_id,
            &fields,
            &Guard::Hash(input.base_hash.clone()),
        )
        .await;
    let row = finish(&state, &auth, id, &result).await?;
    audit(
        &state,
        &auth,
        "unifi.wifi.apply",
        Some(id),
        json!({
            "ok": result.is_ok(),
            "error": result.as_ref().err().map(WriteError::class),
            "wifi_id": row.wifi_id,
            "wifi_name": row.wifi_name,
            "survey_id": row.survey_id,
            "fields": row.fields,
            "before": row.before,
            "after": row.after,
        }),
    )
    .await;
    match result {
        Ok(_) => Ok((StatusCode::CREATED, Json(row))),
        Err(e) => Err(write_refusal(e, None)),
    }
}

async fn begin(
    state: &AppState,
    auth: &AuthUser,
    u: &Unifi,
    (site_id, wifi_id): (&str, &str),
    rollback_of: Option<Uuid>,
    survey_id: Option<Uuid>,
    fields: &[(Field, Slot)],
) -> Result<Uuid> {
    changes::begin(
        &state.db,
        auth.tenant,
        &NewChange {
            target: "ssid",
            site_id: Some(site_id),
            wifi_id: Some(wifi_id),
            wifi_name: u.conn.voice_wifi_name.as_deref().unwrap_or_default(),
            phone_id: None,
            phone_name: "",
            kind: if rollback_of.is_some() {
                "rollback"
            } else {
                "apply"
            },
            rollback_of,
            survey_id,
            fields: fields.iter().map(|(f, _)| f.key().to_owned()).collect(),
            network_version: u.conn.version.as_deref(),
            created_by: Some(auth.id),
        },
    )
    .await
    .map_err(|e| match e {
        CoreError::Conflict(_) => gate("change_in_progress"),
        other => other.into(),
    })
}

/// Stores the result of a write in the change row.
async fn finish(
    state: &AppState,
    auth: &AuthUser,
    id: Uuid,
    result: &std::result::Result<write::Applied, WriteError>,
) -> Result<WifiChange> {
    let (before, after) = match result {
        Ok(a) => (Some(&a.before), Some(&a.after)),
        Err(WriteError::Verify { before, after }) => (Some(before), Some(after)),
        Err(_) => (None, None),
    };
    let before = before.map(allowlist::to_json);
    let after = after.map(allowlist::to_json);
    changes::finish(
        &state.db,
        auth.tenant,
        id,
        &Outcome {
            state: if result.is_ok() {
                changes::state::APPLIED
            } else {
                changes::state::FAILED
            },
            wifi_name: result.as_ref().ok().map(|a| a.name.as_str()),
            before: before.as_ref(),
            after: after.as_ref(),
            error: result.as_ref().err().map(WriteError::class),
        },
    )
    .await?;
    Ok(changes::get(&state.db, auth.tenant, id).await?)
}

fn write_refusal(e: WriteError, after: Option<&Snapshot>) -> Refusal {
    match e {
        WriteError::Conflict { current } => match after {
            Some(after) => Refusal::Foreign(foreign(after, None, &current)),
            None => gate("ssid_changed"),
        },
        WriteError::NotWritable(reason) => gate(reason),
        WriteError::Forbidden => gate("unifi_forbidden"),
        WriteError::Verify { .. } => gate("verify_mismatch"),
        WriteError::Invalid(m) => ApiError::BadRequest(m).into(),
        WriteError::Rejected { .. } => ApiError::Device(e.to_string()).into(),
        WriteError::Unifi(e) => unifi_error(e),
    }
}

/// One field that changed in UniFi after the TalkOps change.
#[derive(Debug, Serialize, ToSchema)]
pub struct ForeignField {
    pub field: String,
    /// Value after the TalkOps change.
    #[schema(value_type = Object)]
    pub after: Slot,
    /// Value in UniFi now.
    #[schema(value_type = Object)]
    pub current: Slot,
    /// Value that the rollback writes.
    #[schema(value_type = Option<Object>)]
    pub before: Option<Slot>,
}

pub(crate) fn foreign(
    after: &Snapshot,
    before: Option<&Snapshot>,
    current: &Snapshot,
) -> Vec<ForeignField> {
    after
        .iter()
        .filter_map(|(k, a)| {
            let now = current.get(k).cloned().unwrap_or_else(Slot::absent);
            (!now.same(a)).then(|| ForeignField {
                field: k.clone(),
                after: a.clone(),
                current: now,
                before: before.and_then(|b| b.get(k).cloned()),
            })
        })
        .collect()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RollbackInput {
    #[serde(default)]
    pub confirm: bool,
    /// Roll back also when another person changed the fields since.
    #[serde(default)]
    pub accept_foreign_changes: bool,
    /// Wi-Fi handsets without a UniFi report that the user confirmed for a
    /// rollback that removes a band (ADR 0043).
    #[serde(default)]
    pub confirm_unknown_handsets: Vec<Uuid>,
}

/// Restores the values before a change, for the changed fields only.
#[utoipa::path(post, path = "/api/v1/unifi/wifi-changes/{id}/rollback", tag = "wifi-survey", params(("id" = Uuid, Path)), request_body = RollbackInput, responses((status = 201, body = WifiChange), (status = 409, description = "foreign_change: the fields changed in UniFi")))]
pub async fn rollback_change(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<RollbackInput>,
) -> Result<(StatusCode, Json<WifiChange>)> {
    auth.require_perm(Permission::UnifiWrite)?;
    if !input.confirm {
        return Err(ApiError::BadRequest("confirm the rollback".into()).into());
    }
    let change = changes::get(&state.db, auth.tenant, id).await?;
    if !change.can_roll_back() {
        return Err(gate("not_revertible"));
    }
    if change.target == "handset" {
        return handset::rollback_handset(&state, &auth, &change, input.accept_foreign_changes)
            .await;
    }
    let (Some(site_id), Some(wifi_id)) = (change.site_id.clone(), change.wifi_id.clone()) else {
        return Err(gate("not_revertible"));
    };
    let before = allowlist::from_json(change.before.as_ref().unwrap_or(&Value::Null));
    let after = allowlist::from_json(change.after.as_ref().unwrap_or(&Value::Null));
    if before.is_empty() || before.len() != change.fields.len() {
        return Err(gate("not_revertible"));
    }
    let conn = unifi::get(&state.db, auth.tenant)
        .await?
        .ok_or_else(|| gate("write_disabled"))?;
    // The rollback goes to the SSID of the change, also after another
    // voice SSID was selected.
    match write_state(&state, &auth, &conn).await?.reason {
        None | Some("no_voice_ssid") => {}
        Some(reason) => return Err(gate(reason)),
    }
    let u = connect(&state, &auth).await?;
    if !input.accept_foreign_changes {
        let current = u
            .client
            .wifi_broadcast(&site_id, &wifi_id)
            .await
            .map_err(unifi_error)?;
        let fields: Vec<Field> = after.keys().filter_map(|k| Field::from_key(k)).collect();
        let diff = foreign(&after, Some(&before), &current.snapshot(&fields));
        if !diff.is_empty() {
            return Err(Refusal::Foreign(diff));
        }
    }
    let restore = write::rollback_changes(&before);
    protect_bands(
        &state,
        &auth,
        &u,
        &site_id,
        &wifi_id,
        &restore,
        &input.confirm_unknown_handsets,
    )
    .await?;
    let new_id = begin(
        &state,
        &auth,
        &u,
        (&site_id, &wifi_id),
        Some(change.id),
        change.survey_id,
        &restore,
    )
    .await?;
    let guard = if input.accept_foreign_changes {
        Guard::None
    } else {
        Guard::Values(after.clone())
    };
    let result = u
        .client
        .writer()
        .apply(&site_id, &wifi_id, &restore, &guard)
        .await;
    let row = finish(&state, &auth, new_id, &result).await?;
    if result.is_ok() {
        changes::mark_rolled_back(&state.db, auth.tenant, change.id).await?;
    }
    audit(
        &state,
        &auth,
        "unifi.wifi.rollback",
        Some(new_id),
        json!({
            "ok": result.is_ok(),
            "error": result.as_ref().err().map(WriteError::class),
            "rollback_of": change.id,
            "wifi_id": row.wifi_id,
            "wifi_name": row.wifi_name,
            "fields": row.fields,
            "before": row.before,
            "after": row.after,
            "foreign_changes_accepted": input.accept_foreign_changes,
        }),
    )
    .await;
    match result {
        Ok(_) => Ok((StatusCode::CREATED, Json(row))),
        Err(e) => Err(write_refusal(e, Some(&after))),
    }
}
