//! Wi-Fi voice survey control (ADR 0041).
//!
//! - [`originate`] calls the Wi-Fi device; FreeSWITCH hands the answered
//!   call to the outbound socket ([`handle`]). A self-dial call (`*39`)
//!   reaches the same handler through the dialplan.
//! - [`handle`] records the call and plays the test signal until hangup.
//!   It uses `playback`, not `endless_playback`: only `playback` reads the
//!   DTMF queue, and FreeSWITCH fires `DTMF` events on that read.
//! - [`spawn`] polls the media values of each running survey once a second
//!   (`mediaStats` JSON API) and the Wi-Fi state from UniFi. It follows
//!   `DTMF` events (markers, `#` ends the test) and `RECV_RTCP_MESSAGE`
//!   events (downlink loss and jitter).
//! - The call record ([`ingest_cdr`]) stores the uplink error periods and
//!   queues the analysis in the media worker.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, TimeZone, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::error::{CoreError, CoreResult};
use talkops_core::tenant::TenantId;
use talkops_core::wifi_survey::{self as ws, MediaSample, RadioSample, Survey, state};
use talkops_core::{extensions, prompts, settings, unifi};
use talkops_esl::outbound::OutboundSession;
use talkops_esl::{EslClient, Event};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::AppState;
use crate::esl::EslHandle;
use crate::fsxml::{self, SIP_DOMAIN, codecs};

/// Value of `talkops_app` for survey calls.
pub const APP: &str = "wifi_survey";
/// How long the handset may ring.
const RING_SECS: i64 = 45;
/// Poll interval.
const TICK: Duration = Duration::from_secs(1);

/// No user-controlled text may create channel variable or ESL syntax.
fn safe(value: &str) -> CoreResult<&str> {
    if value.contains([
        '|', '{', '}', '[', ']', '\r', '\n', '\'', '"', '\\', '$', ' ',
    ]) {
        Err(CoreError::Validation("unsafe survey value".into()))
    } else {
        Ok(value)
    }
}

/// Test signal: "Measurement running", then one short tone each second.
pub fn signal(sounds: &std::path::Path, lang: &str) -> String {
    let tones = "tone_stream://L=28;%(100,900,1000)";
    match prompts::path(sounds, "wifi_survey_running", lang).filter(|p| p.is_file()) {
        Some(p) => format!("file_string://{}!silence_stream://500!{tones}", p.display()),
        None => format!("tone_stream://L=30;%(100,900,1000)"),
    }
}

/// Starts the call of an originated survey.
pub async fn originate(s: &AppState, tenant: TenantId, survey: &Survey) -> CoreResult<()> {
    let Some(device) = survey.device_id else {
        return Err(CoreError::Validation("the device no longer exists".into()));
    };
    let device = extensions::get_device(&s.db, tenant, device).await?;
    let Some(esl) = s.telephony.esl.get().await else {
        ws::fail(
            &s.db,
            tenant,
            survey.id,
            state::FAILED,
            "telephony not connected",
        )
        .await?;
        return Err(CoreError::Conflict("telephony is not connected".into()));
    };
    let call = Uuid::new_v4();
    ws::set_call(&s.db, tenant, survey.id, call).await?;
    let user = safe(&device.sip_username)?;
    let mut vars = vec![
        format!("origination_uuid={call}"),
        format!("talkops_tenant_id={tenant}"),
        format!("talkops_survey_id={}", survey.id),
        format!("talkops_app={APP}"),
        "origination_caller_id_name=TalkOps WLAN-Test".into(),
        format!("origination_caller_id_number={}", ws::SELF_DIAL_CODE),
        format!("originate_timeout={RING_SECS}"),
        "ignore_early_media=true".into(),
        // RTCP reports each second: TalkOps sends its own reports, and the
        // handset's reports arrive as RECV_RTCP_MESSAGE events.
        "rtcp_audio_interval_msec=1000".into(),
        "RECORD_STEREO=true".into(),
        "verbose_events=true".into(),
    ];
    // The same codecs and jitter buffer as a normal call to the device.
    let profile = extensions::CodecProfile::effective(device.kind, device.codec_profile);
    if let Some(media) = codecs::leg_media(profile) {
        for (name, value) in media.vars() {
            vars.push(format!("{name}={value}"));
        }
    }
    safe(&s.outbound_socket)?;
    let command = format!(
        "originate '{{^^|{}}}user/{user}@{SIP_DOMAIN}' '&socket({} async full)'",
        vars.join("|"),
        s.outbound_socket
    );
    if let Err(err) = esl.bgapi(&command).await {
        ws::fail(
            &s.db,
            tenant,
            survey.id,
            state::FAILED,
            "call could not start",
        )
        .await?;
        tracing::warn!(error = %err, survey = %survey.id, "survey originate failed");
        return Err(CoreError::Conflict("the call could not start".into()));
    }
    Ok(())
}

/// Ends the call of a survey (stop button, `#`, time limit).
pub async fn hang_up(esl: &EslHandle, call: Uuid) {
    if let Some(client) = esl.get().await {
        let _ = client
            .api(&format!("uuid_kill {call} NORMAL_CLEARING"))
            .await;
    }
}

/// The answered survey call, handed over by FreeSWITCH.
pub async fn handle(mut session: OutboundSession, s: AppState) {
    if let Err(err) = run(&mut session, &s).await {
        tracing::warn!(error = %err, call = session.uuid(), "survey call ended");
    }
    session.hangup("NORMAL_CLEARING").await;
}

async fn run(session: &mut OutboundSession, s: &AppState) -> anyhow::Result<()> {
    let tenant = TenantId(
        session
            .var("talkops_tenant_id")
            .ok_or_else(|| anyhow::anyhow!("missing tenant"))?
            .parse()?,
    );
    let id: Uuid = session
        .var("talkops_survey_id")
        .ok_or_else(|| anyhow::anyhow!("missing survey"))?
        .parse()?;
    let call: Uuid = session.uuid().parse()?;
    let survey = ws::get(&s.db, tenant, id).await?;
    if survey.call_uuid != Some(call) || survey.state != state::CALLING {
        anyhow::bail!("stale survey call");
    }
    let file = ws::recording_file(tenant, id);
    let path = s.media.recordings.join(&file);
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let path = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid recording path"))?
        .to_owned();
    safe(&path)?;
    session.execute("answer", "").await?;
    session.set("RECORD_STEREO", "true").await?;
    session.set("playback_terminators", "none").await?;
    session.execute("record_session", &path).await?;
    if !ws::mark_answered(&s.db, tenant, id, Utc::now(), Some(&file)).await? {
        anyhow::bail!("survey stopped before answer");
    }
    let lang = settings::get(&s.db, tenant).await?.default_language;
    let signal = signal(&s.media.sounds, &lang);
    let limit = Duration::from_secs(survey.thresholds().max_minutes.max(1) as u64 * 60);
    let start = Instant::now();
    while start.elapsed() < limit {
        match session.execute("playback", &signal).await {
            Ok(_) => {}
            Err(talkops_esl::EslError::Hangup) => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    }
    tracing::info!(survey = %id, "survey reached its time limit");
    Ok(())
}

/// Dialplan of a self-dial call (`*39`) from `device`.
pub async fn self_dial(
    pool: &PgPool,
    socket: &str,
    sounds: &std::path::Path,
    tenant: TenantId,
    device: Uuid,
    call: Uuid,
) -> CoreResult<Vec<(&'static str, String)>> {
    let survey = ws::claim_armed(pool, tenant, device, call).await?;
    let Some(survey) = survey else {
        let lang = settings::get(pool, tenant).await?.default_language;
        let mut a = vec![("answer", String::new()), ("sleep", "300".to_owned())];
        if let Some(p) =
            prompts::path(sounds, "wifi_survey_not_armed", &lang).filter(|p| p.is_file())
        {
            a.push(("playback", p.display().to_string()));
        }
        a.push(("hangup", String::new()));
        return Ok(a);
    };
    Ok(vec![
        ("set", format!("talkops_app={APP}")),
        ("set", format!("talkops_survey_id={}", survey.id)),
        ("set", "rtcp_audio_interval_msec=1000".to_owned()),
        ("set", "verbose_events=true".to_owned()),
        ("socket", format!("{socket} async full")),
    ])
}

/// Downlink values from the handset's latest RTCP report.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rtcp {
    pub fraction_lost_pct: f32,
    pub jitter_ms: f32,
}

/// Reads an RTCP receiver report event (the first report block).
pub fn parse_rtcp(ev: &Event) -> Option<Rtcp> {
    let h = &ev.headers;
    let fraction: f32 = h.get("Source0-Fraction")?.parse().ok()?;
    let jitter: f32 = h.get("Source0-Jitter")?.parse().ok()?;
    let rate: f32 = h
        .get("RTP-Rate")
        .and_then(|r| r.parse().ok())
        .filter(|r| *r > 0.0)
        .unwrap_or(8000.0);
    Some(Rtcp {
        // RFC 3550: lost share since the last report, fixed point /256.
        fraction_lost_pct: fraction * 100.0 / 256.0,
        // Jitter in timestamp units.
        jitter_ms: jitter * 1000.0 / rate,
    })
}

/// Cumulative counters of one `mediaStats` answer.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Counters {
    pub received: i64,
    pub flaws: i64,
    pub mos: Option<f32>,
    pub jitter_max_ms: Option<f32>,
}

/// Reads the `json {"command":"mediaStats"}` answer.
pub fn parse_media_stats(body: &str) -> Option<Counters> {
    let v: Value = serde_json::from_str(body.trim()).ok()?;
    let audio = v.get("response")?.get("audio")?;
    let num = |k: &str| audio.get(k).and_then(Value::as_f64);
    Some(Counters {
        received: num("in_media_packet_count")? as i64,
        flaws: num("in_flaw_total").unwrap_or(0.0) as i64,
        mos: num("in_mos").map(|m| m as f32),
        jitter_max_ms: num("in_jitter_max_variance").map(|j| j as f32),
    })
}

/// The media sample between two polls.
pub fn sample(
    prev: Option<(Counters, i32)>,
    now: Counters,
    offset_ms: i32,
    ptime_ms: i32,
    rtcp: Option<Rtcp>,
) -> Option<MediaSample> {
    let (before, before_ms) = prev?;
    let interval = offset_ms - before_ms;
    if interval <= 0 {
        return None;
    }
    Some(MediaSample {
        offset_ms,
        interval_ms: interval,
        received: (now.received - before.received).clamp(0, i32::MAX as i64) as i32,
        expected: (interval as f32 / ptime_ms.max(1) as f32).round() as i32,
        flaws: (now.flaws - before.flaws).clamp(0, i32::MAX as i64) as i32,
        mos: now.mos,
        jitter_max_ms: now.jitter_max_ms,
        rtcp_fraction_lost: rtcp.map(|r| r.fraction_lost_pct),
        rtcp_jitter_ms: rtcp.map(|r| r.jitter_ms),
    })
}

/// Live state shared by the poll and event tasks.
#[derive(Default)]
struct Live {
    /// Calls of running surveys.
    calls: Mutex<HashMap<Uuid, (TenantId, Uuid)>>,
    rtcp: Mutex<HashMap<Uuid, Rtcp>>,
    /// Surveys with a running collector.
    collectors: Mutex<HashSet<Uuid>>,
}

/// Starts the poll and event tasks.
pub fn spawn(state: AppState) {
    let live = Arc::new(Live::default());
    tokio::spawn(events(state.clone(), live.clone()));
    tokio::spawn(async move {
        let mut last_expire = Instant::now() - Duration::from_secs(60);
        loop {
            if last_expire.elapsed() > Duration::from_secs(30) {
                last_expire = Instant::now();
                if let Err(err) = ws::expire_armed(&state.db).await {
                    tracing::warn!(error = %err, "cannot expire armed surveys");
                }
            }
            if let Err(err) = supervise(&state, &live).await {
                tracing::warn!(error = %err, "survey supervision failed");
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

async fn supervise(s: &AppState, live: &Arc<Live>) -> CoreResult<()> {
    let active = ws::active(&s.db).await?;
    let mut calls = HashMap::new();
    for (tenant, survey) in &active {
        let Some(call) = survey.call_uuid else {
            continue;
        };
        calls.insert(call, (*tenant, survey.id));
        match survey.state.as_str() {
            state::RUNNING => {
                let new = live
                    .collectors
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(survey.id);
                if new {
                    tokio::spawn(collect(s.clone(), live.clone(), *tenant, survey.clone()));
                }
            }
            state::CALLING => {
                // No answer: the call is gone after the ring time.
                let ringing = survey
                    .started_at
                    .is_some_and(|t| (Utc::now() - t).num_seconds() < RING_SECS + 15);
                if !ringing && !call_exists(&s.telephony.esl, call).await {
                    ws::fail(&s.db, *tenant, survey.id, state::FAILED, "not answered").await?;
                }
            }
            _ => {}
        }
    }
    *live.calls.lock().unwrap_or_else(|e| e.into_inner()) = calls;
    Ok(())
}

async fn call_exists(esl: &EslHandle, call: Uuid) -> bool {
    match esl.get().await {
        Some(client) => client
            .api(&format!("uuid_exists {call}"))
            .await
            .map(|r| r.trim() == "true")
            // Unknown: keep the survey.
            .unwrap_or(true),
        None => true,
    }
}

/// UniFi access for one survey.
struct Radio {
    client: talkops_unifi::Client,
    mac: String,
    names: HashMap<String, String>,
    names_at: Option<Instant>,
    site_id: Option<String>,
}

async fn radio_client(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    mac: Option<&str>,
) -> Option<Radio> {
    let mac = mac?;
    let conn = unifi::get(db, tenant).await.ok()??;
    let key = unifi::api_key(db, tenant, secrets).await.ok()?;
    let client = talkops_unifi::Client::new(talkops_unifi::Config {
        url: conn.url,
        site: conn.site,
        api_key: key,
        cert_sha256: conn.cert_sha256,
    })
    .ok()?;
    Some(Radio {
        client,
        mac: mac.to_owned(),
        names: HashMap::new(),
        names_at: None,
        site_id: conn
            .capabilities
            .get("site_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

impl Radio {
    async fn poll(&mut self, offset_ms: i32) -> RadioSample {
        if self
            .names_at
            .is_none_or(|t| t.elapsed() > Duration::from_secs(60))
        {
            self.names_at = Some(Instant::now());
            if let Ok(names) = self.client.ap_names(self.site_id.as_deref()).await {
                self.names = names;
            }
        }
        let empty = |error: Option<String>| RadioSample {
            offset_ms,
            ap_mac: None,
            ap_name: None,
            bssid: None,
            radio: None,
            channel: None,
            rssi: None,
            tx_rate_kbps: None,
            rx_rate_kbps: None,
            error,
        };
        match self.client.station(&self.mac).await {
            Ok(Some(st)) => RadioSample {
                offset_ms,
                ap_name: st.ap_mac.as_ref().and_then(|m| self.names.get(m).cloned()),
                ap_mac: st.ap_mac,
                bssid: st.bssid,
                radio: st.radio,
                channel: st.channel,
                rssi: st.rssi,
                tx_rate_kbps: st.tx_rate_kbps,
                rx_rate_kbps: st.rx_rate_kbps,
                error: None,
            },
            Ok(None) => empty(Some("disconnected".into())),
            Err(e) => empty(Some(e.class().into())),
        }
    }
}

/// Collects the samples of one running survey until it ends.
async fn collect(s: AppState, live: Arc<Live>, tenant: TenantId, survey: Survey) {
    let id = survey.id;
    if let Err(err) = collect_loop(&s, &live, tenant, survey).await {
        tracing::warn!(error = %err, survey = %id, "survey collection stopped");
    }
    live.collectors
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
}

async fn collect_loop(
    s: &AppState,
    live: &Live,
    tenant: TenantId,
    mut survey: Survey,
) -> CoreResult<()> {
    let Some(call) = survey.call_uuid else {
        return Ok(());
    };
    let mut radio = radio_client(&s.db, &s.secrets, tenant, survey.mac.as_deref()).await;
    let mut prev: Option<(Counters, i32)> = None;
    let mut media_known = false;
    let mut gone_since: Option<Instant> = None;
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        survey = ws::get(&s.db, tenant, survey.id).await?;
        if survey.state != state::RUNNING {
            return Ok(());
        }
        let Some(esl) = s.telephony.esl.get().await else {
            continue;
        };
        if !media_known {
            media_known = learn_media(&s.db, &esl, tenant, &mut survey, call).await;
        }
        let at = Utc::now();
        let offset = survey.offset_ms(at);
        let radio_poll = async {
            match radio.as_mut() {
                Some(r) => Some(r.poll(offset).await),
                None => None,
            }
        };
        let stats_cmd =
            format!("json {{\"command\":\"mediaStats\",\"data\":{{\"uuid\":\"{call}\"}}}}");
        let media_poll = esl.api(&stats_cmd);
        let (media, radio_sample) = tokio::join!(media_poll, radio_poll);
        match media.ok().as_deref().and_then(parse_media_stats) {
            Some(now) => {
                gone_since = None;
                let rtcp = live
                    .rtcp
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&call)
                    .copied();
                if let Some(sample) = sample(prev, now, offset, survey.ptime_ms, rtcp) {
                    ws::add_media(&s.db, tenant, survey.id, at, &sample).await?;
                }
                prev = Some((now, offset));
            }
            None => {
                // The call ended; the call record finishes the survey. If it
                // does not arrive, finish without the error periods.
                let since = *gone_since.get_or_insert_with(Instant::now);
                if since.elapsed() > Duration::from_secs(90) {
                    let secs = recording_secs(s, &survey).await;
                    ws::finish(&s.db, tenant, survey.id, "UNKNOWN", secs).await?;
                    return Ok(());
                }
            }
        }
        if let Some(r) = radio_sample {
            ws::add_radio(&s.db, tenant, survey.id, at, &r).await?;
        }
    }
}

/// Reads codec and packet time of the call once.
async fn learn_media(
    db: &PgPool,
    esl: &EslClient,
    tenant: TenantId,
    survey: &mut Survey,
    call: Uuid,
) -> bool {
    let (codec_cmd, ptime_cmd) = (
        format!("uuid_getvar {call} rtp_use_codec_name"),
        format!("uuid_getvar {call} rtp_use_codec_ptime"),
    );
    let (codec, ptime) = tokio::join!(esl.api(&codec_cmd), esl.api(&ptime_cmd));
    let (Ok(codec), Ok(ptime)) = (codec, ptime) else {
        return false;
    };
    let Ok(ptime) = ptime.trim().parse::<i32>() else {
        return false;
    };
    let codec = fsxml::sanitize_value(codec.trim());
    if ws::set_media(db, tenant, survey.id, &codec, ptime)
        .await
        .is_ok()
    {
        survey.ptime_ms = ptime.clamp(10, 120);
        survey.codec = Some(codec);
    }
    true
}

async fn recording_secs(s: &AppState, survey: &Survey) -> Option<i32> {
    let file = survey.recording_file.as_deref()?;
    let path = s.media.recordings.join(file);
    tokio::task::spawn_blocking(move || crate::voicemail::ivr::wav_seconds(&path))
        .await
        .ok()
        .flatten()
        .map(|s| s as i32)
}

/// Follows DTMF and RTCP events of survey calls.
async fn events(s: AppState, live: Arc<Live>) {
    loop {
        if let Some(client) = s.telephony.esl.get().await {
            let mut rx = client.events();
            match client.subscribe(&["DTMF", "RECV_RTCP_MESSAGE"]).await {
                Ok(()) => loop {
                    let Ok(next) = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await
                    else {
                        if client.is_connected() {
                            continue;
                        }
                        break;
                    };
                    match next {
                        Ok(ev) => on_event(&s, &live, &ev).await,
                        Err(RecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "survey event watcher missed events");
                        }
                        Err(RecvError::Closed) => break,
                    }
                },
                Err(err) => tracing::warn!(error = %err, "cannot subscribe to survey events"),
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn on_event(s: &AppState, live: &Live, ev: &Event) {
    let Some(call) = ev
        .headers
        .get("Unique-ID")
        .and_then(|u| u.parse::<Uuid>().ok())
    else {
        return;
    };
    let Some((tenant, id)) = live
        .calls
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&call)
        .copied()
    else {
        return;
    };
    match ev.name() {
        Some("RECV_RTCP_MESSAGE") => {
            if let Some(r) = parse_rtcp(ev) {
                live.rtcp
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(call, r);
            }
        }
        Some("DTMF") => {
            let Some(digit) = ev.headers.get("DTMF-Digit").and_then(|d| d.chars().next()) else {
                return;
            };
            if digit == '#' {
                hang_up(&s.telephony.esl, call).await;
                return;
            }
            let Some(marker) = ws::dtmf_marker(digit) else {
                return;
            };
            let added = async {
                let survey = ws::get(&s.db, tenant, id).await?;
                if survey.state == state::RUNNING {
                    ws::add_marker(&s.db, tenant, &survey, marker, "dtmf", "", Utc::now()).await?;
                }
                CoreResult::Ok(())
            };
            if let Err(err) = added.await {
                tracing::warn!(error = %err, survey = %id, "cannot store survey marker");
            }
        }
        _ => {}
    }
}

/// Error periods of the uplink from the call record (`call-stats`).
pub fn error_periods(xml: &str) -> Vec<(DateTime<Utc>, DateTime<Utc>, i64)> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    let micros = |n: roxmltree::Node, tag: &str| {
        n.children()
            .find(|c| c.has_tag_name(tag))
            .and_then(|c| c.text())
            .and_then(|t| t.trim().parse::<i64>().ok())
    };
    doc.descendants()
        .filter(|n| n.has_tag_name("audio"))
        .filter(|n| n.parent().is_some_and(|p| p.has_tag_name("call-stats")))
        .flat_map(|audio| {
            audio
                .descendants()
                .filter(|n| n.has_tag_name("error-period"))
        })
        .filter_map(|p| {
            let start = Utc.timestamp_micros(micros(p, "start")?).single()?;
            let stop = micros(p, "stop")
                .filter(|s| *s > 0)
                .and_then(|s| Utc.timestamp_micros(s).single())
                .unwrap_or(start);
            Some((start, stop, micros(p, "flaws").unwrap_or(0)))
        })
        .collect()
}

/// Finishes the survey of a call record: error periods, recording length,
/// and the analysis job.
pub async fn ingest_cdr(s: &AppState, xml: &str) -> CoreResult<()> {
    if !xml.contains("talkops_survey_id") {
        return Ok(());
    }
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Ok(());
    };
    let var = |name: &str| {
        doc.descendants()
            .find(|n| n.has_tag_name("variables"))
            .and_then(|v| v.children().find(|n| n.has_tag_name(name)))
            .and_then(|n| n.text())
            .map(|t| {
                percent_encoding::percent_decode_str(t.trim())
                    .decode_utf8_lossy()
                    .into_owned()
            })
    };
    let Some(call) = var("uuid").and_then(|u| u.parse::<Uuid>().ok()) else {
        return Ok(());
    };
    let Some((tenant, survey)) = ws::by_call(&s.db, call).await? else {
        return Ok(());
    };
    match survey.state.as_str() {
        state::RUNNING => {
            for (start, stop, flaws) in error_periods(xml) {
                ws::add_error_period(&s.db, tenant, &survey, start, stop, flaws).await?;
            }
            let cause = var("hangup_cause").unwrap_or_default();
            let secs = recording_secs(s, &survey).await;
            ws::finish(
                &s.db,
                tenant,
                survey.id,
                &fsxml::sanitize_value(&cause),
                secs,
            )
            .await?;
        }
        state::CALLING | state::ARMED => {
            let cause = var("hangup_cause").unwrap_or_default();
            ws::fail(
                &s.db,
                tenant,
                survey.id,
                state::FAILED,
                &format!("not answered ({})", fsxml::sanitize_value(&cause)),
            )
            .await?;
        }
        _ => {}
    }
    Ok(())
}

/// Removes recording files of deleted surveys.
pub async fn remove_recordings(recordings: &std::path::Path, files: &[String]) {
    for file in files {
        if file.split('/').any(|p| p == ".." || p.is_empty()) {
            continue;
        }
        let _ = tokio::fs::remove_file(recordings.join(file)).await;
    }
}

/// Short view of the UniFi state of a MAC for the start dialog and the
/// survey snapshot.
pub async fn unifi_state(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    mac: Option<&str>,
) -> Value {
    let Some(conn) = unifi::get(db, tenant).await.ok().flatten() else {
        return json!({ "status": "not_configured" });
    };
    let Some(mac) = mac else {
        return json!({ "status": "no_mac", "version": conn.version });
    };
    let Some(mut radio) = radio_client(db, secrets, tenant, Some(mac)).await else {
        return json!({ "status": "error", "version": conn.version });
    };
    let sample = radio.poll(0).await;
    match (&sample.ap_mac, &sample.error) {
        (Some(_), _) => json!({
            "status": "connected",
            "version": conn.version,
            "ap_mac": sample.ap_mac,
            "ap_name": sample.ap_name,
            "radio": sample.radio,
            "channel": sample.channel,
            "rssi": sample.rssi,
        }),
        (None, Some(e)) if e == "disconnected" => {
            json!({ "status": "not_found", "version": conn.version })
        }
        (None, e) => json!({ "status": "error", "error": e, "version": conn.version }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_media_stats_and_samples() {
        let body = r#"{"command":"mediaStats","data":{"uuid":"x"},"status":"success","response":{"audio":{"in_media_packet_count":500,"in_flaw_total":3,"in_mos":4.41,"in_jitter_max_variance":2.5}}}"#;
        let c = parse_media_stats(body).unwrap();
        assert_eq!(c.received, 500);
        assert_eq!(c.flaws, 3);
        assert!(
            parse_media_stats(r#"{"status":"error","response":"Session does not exist"}"#)
                .is_none()
        );
        let later = Counters {
            received: 545,
            flaws: 8,
            ..c
        };
        assert!(sample(None, c, 1000, 20, None).is_none());
        let s = sample(Some((c, 10_000)), later, 11_000, 20, None).unwrap();
        assert_eq!(
            (s.received, s.expected, s.flaws, s.interval_ms),
            (45, 50, 5, 1000)
        );
        let s = sample(Some((c, 10_000)), later, 12_000, 30, None).unwrap();
        assert_eq!(s.expected, 67);
    }

    #[test]
    fn reads_rtcp_reports() {
        let mut headers = talkops_esl::Headers::default();
        headers.push("Event-Name".into(), "RECV_RTCP_MESSAGE".into());
        headers.push("Source0-Fraction".into(), "64".into());
        headers.push("Source0-Jitter".into(), "160".into());
        headers.push("RTP-Rate".into(), "16000".into());
        let ev = Event {
            headers,
            body: None,
        };
        let r = parse_rtcp(&ev).unwrap();
        assert_eq!(r.fraction_lost_pct, 25.0);
        assert_eq!(r.jitter_ms, 10.0);
    }

    #[test]
    fn reads_error_periods_from_the_call_record() {
        let xml = r#"<?xml version="1.0"?><cdr><call-stats><audio><inbound><mos>4.1</mos></inbound>
            <error-log><error-period><start>1791150000000000</start><stop>1791150003500000</stop>
            <flaws>12</flaws><consecutive-flaws>3</consecutive-flaws><duration-msec>3500</duration-msec>
            </error-period></error-log></audio></call-stats><variables><uuid>u</uuid></variables></cdr>"#;
        let p = error_periods(xml);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].1 - p[0].0).num_milliseconds(), 3500);
        assert_eq!(p[0].2, 12);
        assert!(error_periods("<cdr/>").is_empty());
    }

    #[test]
    fn signal_and_unsafe_values() {
        let dir = std::env::temp_dir().join(format!("talkops-signal-{}", Uuid::new_v4()));
        assert!(signal(&dir, "de").starts_with("tone_stream://L=30;"));
        let p = prompts::path(&dir, "wifi_survey_running", "de").unwrap();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"RIFF").unwrap();
        let s = signal(&dir, "de");
        assert!(s.starts_with("file_string://") && s.contains("!tone_stream://L=28;"));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(safe("abc123").is_ok());
        assert!(safe("a|b").is_err());
        assert!(safe("a b").is_err());
    }
}
