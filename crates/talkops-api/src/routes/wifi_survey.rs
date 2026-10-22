//! Wi-Fi voice survey and the read-only UniFi connection (ADR 0041).
//!
//! `wifi_survey.run` starts surveys and reads own surveys;
//! `wifi_survey.manage` reads all surveys, changes the thresholds and
//! deletes surveys; `unifi.manage` changes the UniFi connection. Each
//! change writes an audit entry without secrets.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::error::CoreError;
use talkops_core::extensions::{self, DeviceKind};
use talkops_core::roles::Permission;
use talkops_core::unifi::{self, UnifiConnection, UnifiInput};
use talkops_core::wifi_survey::{
    self as ws, Event, MediaSample, NewSurvey, RadioSample, Survey, Thresholds, Timeline,
    analysis::{self, Analysis, PairComparison, Roam},
    state,
};
use talkops_core::{audit, phones};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::wifi_survey as control;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_surveys, start_survey))
        .routes(routes!(survey_devices))
        .routes(routes!(device_unifi_state))
        .routes(routes!(get_settings, put_settings))
        .routes(routes!(compare))
        .routes(routes!(get_survey, delete_survey))
        .routes(routes!(stop_survey))
        .routes(routes!(analyse_again))
        .routes(routes!(survey_timeline))
        .routes(routes!(survey_live))
        .routes(routes!(add_marker))
        .routes(routes!(set_marker_note))
        .routes(routes!(survey_audio))
        .routes(routes!(export_survey))
        .routes(routes!(get_unifi, put_unifi, delete_unifi))
        .routes(routes!(test_unifi))
        .routes(routes!(unifi_certificate))
}

/// Conflicts of this area are not "already exists".
fn conflict(err: CoreError) -> ApiError {
    match err {
        CoreError::Conflict(m) => ApiError::Conflict(m),
        other => other.into(),
    }
}

async fn audit(
    state: &AppState,
    auth: &AuthUser,
    action: &str,
    id: Option<Uuid>,
    data: serde_json::Value,
) {
    let _ = audit::record(
        &state.db,
        &auth.actor(),
        action,
        "wifi_survey",
        id.map(|i| i.to_string()),
        data,
    )
    .await;
}

/// A survey the user may read: own surveys, or all with `wifi_survey.manage`.
async fn readable(state: &AppState, auth: &AuthUser, id: Uuid) -> ApiResult<Survey> {
    auth.require_perm(Permission::WifiSurveyRun)?;
    let s = ws::get(&state.db, auth.tenant, id).await?;
    if s.started_by == Some(auth.id) || auth.can(Permission::WifiSurveyManage) {
        Ok(s)
    } else {
        Err(ApiError::NotFound)
    }
}

/// A Wi-Fi device for the start dialog.
#[derive(Debug, Serialize, ToSchema)]
pub struct SurveyDevice {
    pub device_id: Uuid,
    pub extension_id: Uuid,
    pub extension_number: String,
    pub extension_name: String,
    pub device_name: String,
    pub enabled: bool,
    pub phone_id: Option<Uuid>,
    pub phone_model: Option<String>,
    pub mac: Option<String>,
}

/// Wi-Fi devices that can run a survey.
#[utoipa::path(get, path = "/api/v1/wifi-surveys/devices", tag = "wifi-survey", responses((status = 200, body = Vec<SurveyDevice>)))]
pub async fn survey_devices(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<SurveyDevice>>> {
    auth.require_perm(Permission::WifiSurveyRun)?;
    let exts = extensions::list(&state.db, auth.tenant).await?;
    let mut out = Vec::new();
    for d in extensions::list_all_devices(&state.db, auth.tenant).await? {
        if d.kind != DeviceKind::Wifi {
            continue;
        }
        let Some(ext) = exts.iter().find(|e| e.id == d.extension_id) else {
            continue;
        };
        let phone = match d.phone_id {
            Some(p) => phones::get(&state.db, auth.tenant, p).await.ok(),
            None => None,
        };
        out.push(SurveyDevice {
            device_id: d.id,
            extension_id: ext.id,
            extension_number: ext.number.clone(),
            extension_name: ext.display_name.clone(),
            device_name: d.name,
            enabled: d.enabled && ext.enabled,
            phone_id: d.phone_id,
            phone_model: phone.as_ref().map(|p| p.model.clone()),
            mac: phone.map(|p| p.mac),
        });
    }
    out.sort_by(|a, b| a.extension_number.cmp(&b.extension_number));
    Ok(Json(out))
}

/// Current UniFi state of a Wi-Fi device (start dialog).
#[utoipa::path(get, path = "/api/v1/wifi-surveys/devices/{id}/unifi", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 200, body = Object)))]
pub async fn device_unifi_state(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    auth.require_perm(Permission::WifiSurveyRun)?;
    let target = ws::target(&state.db, auth.tenant, id).await?;
    Ok(Json(
        control::unifi_state(
            &state.db,
            &state.secrets,
            auth.tenant,
            target.mac.as_deref(),
        )
        .await,
    ))
}

/// Thresholds, retention and limits.
#[utoipa::path(get, path = "/api/v1/wifi-surveys/settings", tag = "wifi-survey", responses((status = 200, body = Thresholds)))]
pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Thresholds>> {
    auth.require_perm(Permission::WifiSurveyRun)?;
    Ok(Json(ws::settings(&state.db, auth.tenant).await?))
}

#[utoipa::path(put, path = "/api/v1/wifi-surveys/settings", tag = "wifi-survey", request_body = Thresholds, responses((status = 200, body = Thresholds)))]
pub async fn put_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<Thresholds>,
) -> ApiResult<Json<Thresholds>> {
    auth.require_perm(Permission::WifiSurveyManage)?;
    ws::save_settings(&state.db, auth.tenant, &input).await?;
    audit(&state, &auth, "wifi_survey.settings", None, json!(input)).await;
    Ok(Json(ws::settings(&state.db, auth.tenant).await?))
}

/// Surveys, newest first (own surveys, or all with `wifi_survey.manage`).
#[utoipa::path(get, path = "/api/v1/wifi-surveys", tag = "wifi-survey", responses((status = 200, body = Vec<Survey>)))]
pub async fn list_surveys(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Survey>>> {
    auth.require_perm(Permission::WifiSurveyRun)?;
    let owner = (!auth.can(Permission::WifiSurveyManage)).then_some(auth.id);
    Ok(Json(ws::list(&state.db, auth.tenant, owner).await?))
}

/// Starts a survey: TalkOps calls the device, or arms `*39` for 10 minutes.
#[utoipa::path(post, path = "/api/v1/wifi-surveys", tag = "wifi-survey", request_body = NewSurvey, responses((status = 201, body = Survey)))]
pub async fn start_survey(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<NewSurvey>,
) -> ApiResult<(StatusCode, Json<Survey>)> {
    auth.require_perm(Permission::WifiSurveyRun)?;
    let target = ws::target(&state.db, auth.tenant, input.device_id).await?;
    let snapshot = control::unifi_state(
        &state.db,
        &state.secrets,
        auth.tenant,
        target.mac.as_deref(),
    )
    .await;
    let survey = ws::create(
        &state.db,
        auth.tenant,
        &input,
        &target,
        Some(auth.id),
        snapshot,
    )
    .await
    .map_err(conflict)?;
    audit(
        &state,
        &auth,
        "wifi_survey.start",
        Some(survey.id),
        json!({ "device_id": input.device_id, "mode": input.mode, "label": survey.label }),
    )
    .await;
    if survey.mode == "originate" {
        control::originate(&state, auth.tenant, &survey)
            .await
            .map_err(conflict)?;
    }
    Ok((
        StatusCode::CREATED,
        Json(ws::get(&state.db, auth.tenant, survey.id).await?),
    ))
}

/// A survey with its roams.
#[derive(Debug, Serialize, ToSchema)]
pub struct SurveyDetails {
    pub survey: Survey,
    pub roams: Vec<Roam>,
}

#[utoipa::path(get, path = "/api/v1/wifi-surveys/{id}", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 200, body = SurveyDetails)))]
pub async fn get_survey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SurveyDetails>> {
    let survey = readable(&state, &auth, id).await?;
    let roams = ws::roams(&state.db, auth.tenant, id).await?;
    Ok(Json(SurveyDetails { survey, roams }))
}

/// Deletes a finished survey with its values and recording.
#[utoipa::path(delete, path = "/api/v1/wifi-surveys/{id}", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_survey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::WifiSurveyManage)?;
    let file = ws::delete(&state.db, auth.tenant, id)
        .await
        .map_err(conflict)?;
    if let Some(file) = file {
        control::remove_recordings(&state.media.recordings, &[file]).await;
    }
    audit(&state, &auth, "wifi_survey.delete", Some(id), json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Ends the call of a running survey, or cancels an armed one.
#[utoipa::path(post, path = "/api/v1/wifi-surveys/{id}/stop", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn stop_survey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let survey = readable(&state, &auth, id).await?;
    if !survey.is_active() {
        return Err(ApiError::Conflict("the survey is not running".into()));
    }
    match survey.call_uuid {
        Some(call) if survey.state != state::ARMED => {
            control::hang_up(&state.telephony.esl, call).await
        }
        _ => {
            ws::fail(&state.db, auth.tenant, id, state::CANCELLED, "cancelled").await?;
        }
    }
    if survey.state == state::CALLING {
        ws::fail(&state.db, auth.tenant, id, state::CANCELLED, "cancelled").await?;
    }
    audit(&state, &auth, "wifi_survey.stop", Some(id), json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Runs the analysis again with the current thresholds.
#[utoipa::path(post, path = "/api/v1/wifi-surveys/{id}/analyse", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn analyse_again(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::WifiSurveyManage)?;
    ws::reanalyse(&state.db, auth.tenant, id)
        .await
        .map_err(conflict)?;
    audit(&state, &auth, "wifi_survey.analyse", Some(id), json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

/// All samples, markers and error periods.
#[utoipa::path(get, path = "/api/v1/wifi-surveys/{id}/timeline", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 200, body = Timeline)))]
pub async fn survey_timeline(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Timeline>> {
    readable(&state, &auth, id).await?;
    Ok(Json(ws::timeline(&state.db, auth.tenant, id, -1).await?))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct LiveQuery {
    /// Offset (ms) of the newest value the page has; values after it follow.
    #[serde(default = "minus_one")]
    pub since: i32,
}

fn minus_one() -> i32 {
    -1
}

/// Values of the last 10 seconds for the live tiles.
#[derive(Debug, Default, Serialize, ToSchema)]
pub struct LiveNow {
    /// Offset (ms) of the newest media value.
    pub offset_ms: i32,
    /// Packets in the newest interval.
    pub received: Option<i32>,
    pub expected: Option<i32>,
    /// Uplink loss in the last 10 s (percent).
    pub loss_10s_pct: f32,
    /// Longest uplink gap in the last 10 s (ms).
    pub gap_10s_ms: i32,
    /// Rating of the last 5 s (traffic light).
    pub rating_5s: Option<analysis::Rating>,
    pub mos: Option<f32>,
    pub rtcp_fraction_lost: Option<f32>,
    pub rtcp_jitter_ms: Option<f32>,
    pub radio: Option<RadioSample>,
}

/// Compact live data for the dashboard; the page polls it each second.
#[derive(Debug, Serialize, ToSchema)]
pub struct Live {
    pub survey: Survey,
    pub now: LiveNow,
    /// New values after `since`.
    pub timeline: Timeline,
    /// Preliminary analysis of all values so far.
    pub analysis: Analysis,
}

fn live_now(media: &[MediaSample], radio: &[RadioSample], ptime: i32, t: &Thresholds) -> LiveNow {
    let Some(last) = media.last() else {
        return LiveNow {
            radio: radio.last().cloned(),
            ..LiveNow::default()
        };
    };
    let window = |ms: i32| -> Vec<MediaSample> {
        media
            .iter()
            .filter(|s| s.offset_ms > last.offset_ms - ms)
            .cloned()
            .collect()
    };
    let w10 = window(10_000);
    let expected: i64 = w10.iter().map(|s| s.expected as i64).sum();
    let missing: i64 = w10
        .iter()
        .map(|s| (s.expected - s.received).max(0) as i64)
        .sum();
    let gap = |w: &[MediaSample]| {
        let mut best = 0;
        let mut run = 0;
        for s in w {
            let m = (s.expected - s.received).max(0);
            run = if m > 0 { run + m * ptime } else { 0 };
            best = best.max(run);
        }
        best
    };
    let w5 = window(5_000);
    let e5: i64 = w5.iter().map(|s| s.expected as i64).sum();
    let m5: i64 = w5
        .iter()
        .map(|s| (s.expected - s.received).max(0) as i64)
        .sum();
    let loss5 = if e5 > 0 {
        m5 as f32 * 100.0 / e5 as f32
    } else {
        0.0
    };
    LiveNow {
        offset_ms: last.offset_ms,
        received: Some(last.received),
        expected: Some(last.expected),
        loss_10s_pct: if expected > 0 {
            missing as f32 * 100.0 / expected as f32
        } else {
            0.0
        },
        gap_10s_ms: gap(&w10),
        rating_5s: Some(analysis::Rating::of(gap(&w5), loss5, t)),
        mos: last.mos,
        rtcp_fraction_lost: media.iter().rev().find_map(|s| s.rtcp_fraction_lost),
        rtcp_jitter_ms: media.iter().rev().find_map(|s| s.rtcp_jitter_ms),
        radio: radio.last().cloned(),
    }
}

#[utoipa::path(get, path = "/api/v1/wifi-surveys/{id}/live", tag = "wifi-survey", params(("id" = Uuid, Path), LiveQuery), responses((status = 200, body = Live)))]
pub async fn survey_live(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<LiveQuery>,
) -> ApiResult<Json<Live>> {
    let survey = readable(&state, &auth, id).await?;
    let all = ws::timeline(&state.db, auth.tenant, id, -1).await?;
    let t = survey.thresholds();
    let analysis = analysis::analyse(&all.media, &all.radio, &all.events, survey.ptime_ms, &t);
    let now = live_now(&all.media, &all.radio, survey.ptime_ms, &t);
    let since = q.since;
    let timeline = Timeline {
        media: all
            .media
            .into_iter()
            .filter(|s| s.offset_ms > since)
            .collect(),
        radio: all
            .radio
            .into_iter()
            .filter(|s| s.offset_ms > since)
            .collect(),
        events: all
            .events
            .into_iter()
            .filter(|e| e.offset_ms > since)
            .collect(),
    };
    Ok(Json(Live {
        survey,
        now,
        timeline,
        analysis,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MarkerInput {
    /// `disturbance`, `location` or `note`.
    pub marker: String,
    #[serde(default)]
    pub note: String,
}

/// Adds a marker from the web interface (the same as DTMF 1, 2 or 3).
#[utoipa::path(post, path = "/api/v1/wifi-surveys/{id}/markers", tag = "wifi-survey", params(("id" = Uuid, Path)), request_body = MarkerInput, responses((status = 201, body = Event)))]
pub async fn add_marker(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<MarkerInput>,
) -> ApiResult<(StatusCode, Json<Event>)> {
    let survey = readable(&state, &auth, id).await?;
    if survey.state != state::RUNNING {
        return Err(ApiError::Conflict("the survey is not running".into()));
    }
    let ev = ws::add_marker(
        &state.db,
        auth.tenant,
        &survey,
        &input.marker,
        "ui",
        &input.note,
        chrono::Utc::now(),
    )
    .await?;
    // The note can contain a place name: not in the audit entry.
    audit(
        &state,
        &auth,
        "wifi_survey.marker",
        Some(id),
        json!({ "marker": input.marker }),
    )
    .await;
    Ok((StatusCode::CREATED, Json(ev)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NoteInput {
    pub note: String,
}

/// Changes the note of a marker.
#[utoipa::path(put, path = "/api/v1/wifi-surveys/{id}/markers/{marker_id}", tag = "wifi-survey", params(("id" = Uuid, Path), ("marker_id" = Uuid, Path)), request_body = NoteInput, responses((status = 204)))]
pub async fn set_marker_note(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, marker)): Path<(Uuid, Uuid)>,
    Json(input): Json<NoteInput>,
) -> ApiResult<StatusCode> {
    readable(&state, &auth, id).await?;
    ws::set_note(&state.db, auth.tenant, id, marker, &input.note).await?;
    audit(
        &state,
        &auth,
        "wifi_survey.marker_note",
        Some(id),
        json!({ "marker_id": marker }),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// The survey recording (stereo WAV, left: handset, right: test signal),
/// with HTTP range support for playback from a position.
#[utoipa::path(get, path = "/api/v1/wifi-surveys/{id}/audio", tag = "wifi-survey", params(("id" = Uuid, Path)), responses((status = 200, content_type = "audio/wav", body = Vec<u8>), (status = 206, content_type = "audio/wav", body = Vec<u8>)))]
pub async fn survey_audio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let survey = readable(&state, &auth, id).await?;
    let file = survey.recording_file.ok_or(ApiError::NotFound)?;
    if file.split('/').any(|p| p == ".." || p.is_empty()) {
        return Err(ApiError::NotFound);
    }
    super::voicemail::wav_range(state.media.recordings.join(file), &headers).await
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ExportQuery {
    /// `json` (default) or `csv`.
    #[serde(default)]
    pub format: Option<String>,
}

fn csv_field(v: impl ToString) -> String {
    let s = v.to_string();
    // No formulas in spreadsheet programs.
    let s = if s.starts_with(['=', '+', '-', '@']) && s.parse::<f64>().is_err() {
        format!("'{s}")
    } else {
        s
    };
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn opt<T: ToString>(v: &Option<T>) -> String {
    v.as_ref()
        .map(|x| csv_field(x.to_string()))
        .unwrap_or_default()
}

/// Timeline and roams of a survey as one CSV table (one row each second).
pub fn export_csv(tl: &Timeline, roams: &[Roam]) -> String {
    let mut out = String::from(
        "offset_s,received,expected,flaws,mos,rtcp_loss_pct,rtcp_jitter_ms,ap_mac,ap_name,radio,channel,rssi_dbm,tx_kbps,rx_kbps,marker,note,roam,rating,gap_ms\n",
    );
    let mut offsets: Vec<i32> = tl
        .media
        .iter()
        .map(|m| m.offset_ms)
        .chain(tl.radio.iter().map(|r| r.offset_ms))
        .chain(
            tl.events
                .iter()
                .filter(|e| e.kind == "marker")
                .map(|e| e.offset_ms),
        )
        .chain(roams.iter().map(|r| r.offset_ms))
        .collect();
    offsets.sort_unstable();
    offsets.dedup();
    for at in offsets {
        let m = tl.media.iter().find(|m| m.offset_ms == at);
        let r = tl.radio.iter().find(|r| r.offset_ms == at);
        let e = tl
            .events
            .iter()
            .find(|e| e.offset_ms == at && e.kind == "marker");
        let roam = roams.iter().find(|x| x.offset_ms == at);
        let row = [
            format!("{:.3}", at as f64 / 1000.0),
            opt(&m.map(|m| m.received)),
            opt(&m.map(|m| m.expected)),
            opt(&m.map(|m| m.flaws)),
            opt(&m.and_then(|m| m.mos)),
            opt(&m.and_then(|m| m.rtcp_fraction_lost)),
            opt(&m.and_then(|m| m.rtcp_jitter_ms)),
            opt(&r.and_then(|r| r.ap_mac.clone())),
            opt(&r.and_then(|r| r.ap_name.clone())),
            opt(&r.and_then(|r| r.radio.clone())),
            opt(&r.and_then(|r| r.channel)),
            opt(&r.and_then(|r| r.rssi)),
            opt(&r.and_then(|r| r.tx_rate_kbps)),
            opt(&r.and_then(|r| r.rx_rate_kbps)),
            opt(&e.and_then(|e| e.marker.clone())),
            opt(&e.map(|e| e.note.clone())),
            opt(&roam.map(|x| {
                format!(
                    "{} > {}",
                    x.from_ap_name
                        .clone()
                        .or(x.from_ap_mac.clone())
                        .unwrap_or_default(),
                    x.to_ap_name
                        .clone()
                        .or(x.to_ap_mac.clone())
                        .unwrap_or_default()
                )
            })),
            opt(&roam.map(|x| x.rating.as_str())),
            opt(&roam.map(|x| x.gap_ms)),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

/// Export of the timeline and the roams.
#[utoipa::path(get, path = "/api/v1/wifi-surveys/{id}/export", tag = "wifi-survey", params(("id" = Uuid, Path), ExportQuery), responses((status = 200, body = Object)))]
pub async fn export_survey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    let survey = readable(&state, &auth, id).await?;
    let tl = ws::timeline(&state.db, auth.tenant, id, -1).await?;
    let roams = ws::roams(&state.db, auth.tenant, id).await?;
    audit(
        &state,
        &auth,
        "wifi_survey.export",
        Some(id),
        json!({ "format": q.format }),
    )
    .await;
    let name = format!("wifi-survey-{}", survey.created_at.format("%Y%m%d-%H%M"));
    if q.format.as_deref() == Some("csv") {
        return Ok((
            [
                (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{name}.csv\""),
                ),
            ],
            export_csv(&tl, &roams),
        )
            .into_response());
    }
    let body = json!({ "survey": survey, "roams": roams, "timeline": tl });
    Ok((
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}.json\""),
        )],
        Json(body),
    )
        .into_response())
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct CompareQuery {
    pub a: Uuid,
    pub b: Uuid,
}

/// Comparison of two surveys (before and after).
#[derive(Debug, Serialize, ToSchema)]
pub struct Comparison {
    pub a: Survey,
    pub b: Survey,
    pub pairs: Vec<PairComparison>,
}

#[utoipa::path(get, path = "/api/v1/wifi-surveys/compare", tag = "wifi-survey", params(CompareQuery), responses((status = 200, body = Comparison)))]
pub async fn compare(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<CompareQuery>,
) -> ApiResult<Json<Comparison>> {
    let a = readable(&state, &auth, q.a).await?;
    let b = readable(&state, &auth, q.b).await?;
    let ra = ws::roams(&state.db, auth.tenant, a.id).await?;
    let rb = ws::roams(&state.db, auth.tenant, b.id).await?;
    Ok(Json(Comparison {
        pairs: analysis::compare_pairs(&ra, &rb),
        a,
        b,
    }))
}

/// The UniFi connection without the API key.
#[derive(Debug, Serialize, ToSchema)]
pub struct UnifiView {
    pub connection: Option<UnifiConnection>,
    pub has_api_key: bool,
}

#[utoipa::path(get, path = "/api/v1/unifi/connection", tag = "wifi-survey", responses((status = 200, body = UnifiView)))]
pub async fn get_unifi(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<UnifiView>> {
    auth.require_perm(Permission::UnifiManage)?;
    let connection = unifi::get(&state.db, auth.tenant).await?;
    Ok(Json(UnifiView {
        has_api_key: connection.is_some(),
        connection,
    }))
}

#[utoipa::path(put, path = "/api/v1/unifi/connection", tag = "wifi-survey", request_body = UnifiInput, responses((status = 200, body = UnifiView)))]
pub async fn put_unifi(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<UnifiInput>,
) -> ApiResult<Json<UnifiView>> {
    auth.require_perm(Permission::UnifiManage)?;
    let conn = unifi::save(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit(
        &state,
        &auth,
        "unifi.save",
        None,
        json!({
            "url": conn.url,
            "site": conn.site,
            "cert_pinned": !conn.cert_sha256.is_empty(),
            "api_key_changed": input.api_key.as_deref().is_some_and(|k| !k.trim().is_empty()),
        }),
    )
    .await;
    Ok(Json(UnifiView {
        connection: Some(conn),
        has_api_key: true,
    }))
}

#[utoipa::path(delete, path = "/api/v1/unifi/connection", tag = "wifi-survey", responses((status = 204)))]
pub async fn delete_unifi(State(state): State<AppState>, auth: AuthUser) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::UnifiManage)?;
    unifi::delete(&state.db, auth.tenant).await?;
    audit(&state, &auth, "unifi.delete", None, json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Tests the connection and stores version and available endpoints.
#[utoipa::path(post, path = "/api/v1/unifi/connection/test", tag = "wifi-survey", responses((status = 200, body = UnifiView)))]
pub async fn test_unifi(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<UnifiView>> {
    auth.require_perm(Permission::UnifiManage)?;
    let conn = unifi::get(&state.db, auth.tenant)
        .await?
        .ok_or(ApiError::NotFound)?;
    let key = unifi::api_key(&state.db, auth.tenant, &state.secrets).await?;
    let client = talkops_unifi::Client::new(talkops_unifi::Config {
        url: conn.url.clone(),
        site: conn.site.clone(),
        api_key: key,
        cert_sha256: conn.cert_sha256.clone(),
    });
    let result = match client {
        Ok(c) => c.probe(None).await,
        Err(e) => Err(e),
    };
    match &result {
        Ok(caps) => {
            let caps_json = json!(caps);
            unifi::record_check(
                &state.db,
                auth.tenant,
                Ok((caps.version.as_deref().unwrap_or(""), &caps_json)),
            )
            .await?;
        }
        Err(e) => unifi::record_check(&state.db, auth.tenant, Err(&e.to_string())).await?,
    }
    audit(
        &state,
        &auth,
        "unifi.test",
        None,
        json!({ "ok": result.is_ok(), "error": result.as_ref().err().map(|e| e.class()) }),
    )
    .await;
    Ok(Json(UnifiView {
        connection: unifi::get(&state.db, auth.tenant).await?,
        has_api_key: true,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CertificateQuery {
    pub url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CertificateInfo {
    /// SHA-256 of the server certificate (hex).
    pub cert_sha256: String,
}

/// Reads the certificate fingerprint of a console (sends no key). The
/// administrator compares it with the console before pinning it.
#[utoipa::path(post, path = "/api/v1/unifi/certificate", tag = "wifi-survey", request_body = CertificateQuery, responses((status = 200, body = CertificateInfo)))]
pub async fn unifi_certificate(
    State(_state): State<AppState>,
    auth: AuthUser,
    Json(q): Json<CertificateQuery>,
) -> ApiResult<Json<CertificateInfo>> {
    auth.require_perm(Permission::UnifiManage)?;
    let url = unifi::check_url(&q.url)?;
    let cert_sha256 = talkops_unifi::probe_certificate(&url)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(Json(CertificateInfo { cert_sha256 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(lost: &[(i32, i32)]) -> Vec<MediaSample> {
        (1..=20)
            .map(|s| {
                let m = lost.iter().find(|(x, _)| *x == s).map_or(0, |(_, m)| *m);
                MediaSample {
                    offset_ms: s * 1000,
                    interval_ms: 1000,
                    received: 50 - m,
                    expected: 50,
                    flaws: m,
                    mos: Some(4.2),
                    jitter_max_ms: None,
                    rtcp_fraction_lost: (s == 3).then_some(2.0),
                    rtcp_jitter_ms: None,
                }
            })
            .collect()
    }

    #[test]
    fn live_values() {
        let t = Thresholds::default();
        let now = live_now(&media(&[(18, 3), (19, 5)]), &[], 20, &t);
        assert_eq!(now.offset_ms, 20_000);
        assert_eq!(now.gap_10s_ms, 160);
        assert_eq!(now.rating_5s, Some(analysis::Rating::Bad));
        assert!((now.loss_10s_pct - 1.6).abs() < 0.01);
        assert_eq!(now.rtcp_fraction_lost, Some(2.0));
        let quiet = live_now(&media(&[(2, 3)]), &[], 20, &t);
        assert_eq!(quiet.rating_5s, Some(analysis::Rating::Clean));
        assert_eq!(live_now(&[], &[], 20, &t).received, None);
    }

    #[test]
    fn csv_export_is_safe() {
        let tl = Timeline {
            media: media(&[]),
            radio: Vec::new(),
            events: vec![Event {
                id: Uuid::nil(),
                offset_ms: 2000,
                kind: "marker".into(),
                marker: Some("note".into()),
                source: Some("ui".into()),
                note: "=HYPERLINK(\"x\"), hall".into(),
                data: json!({}),
            }],
        };
        let csv = export_csv(&tl, &[]);
        assert_eq!(csv.lines().count(), 21);
        let row = csv.lines().find(|l| l.starts_with("2.000")).unwrap();
        assert!(row.contains("\"'=HYPERLINK(\"\"x\"\"), hall\""), "{row}");
    }

    #[test]
    fn byte_ranges() {
        use super::super::voicemail::byte_range;
        assert_eq!(byte_range(None, 100), Ok(None));
        assert_eq!(byte_range(Some("bytes=0-"), 100), Ok(Some((0, 99))));
        assert_eq!(byte_range(Some("bytes=10-19"), 100), Ok(Some((10, 19))));
        assert_eq!(byte_range(Some("bytes=90-200"), 100), Ok(Some((90, 99))));
        assert_eq!(byte_range(Some("bytes=-10"), 100), Ok(Some((90, 99))));
        assert_eq!(byte_range(Some("bytes=100-"), 100), Err(()));
        assert_eq!(byte_range(Some("bytes=5-1"), 100), Err(()));
        assert_eq!(byte_range(Some("bytes=0-1,5-6"), 100), Ok(None));
        assert_eq!(byte_range(Some("items=0-1"), 100), Ok(None));
    }
}
