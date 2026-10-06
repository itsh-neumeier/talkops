//! Per-call dialplan: FreeSWITCH asks for every new call, TalkOps decides
//! where it goes and answers with a single generated extension.
//!
//! - `internal` context: calls from authenticated devices (extensions,
//!   emergency/service numbers, external numbers via the caller's trunk).
//! - `public` context: calls arriving from trunks, routed by phone number.

use std::collections::HashMap;

use sqlx::PgPool;
use talkops_core::cdr::Direction;
use talkops_core::dialing::{DialPlanSettings, Dialed, NumberFormat};
use talkops_core::error::CoreResult;
use talkops_core::extensions::{self, Extension};
use talkops_core::presets::{CallerIdHeader, Dtmf, PresetCatalog, Srtp};
use talkops_core::ring_groups::{self, RingGroup};
use talkops_core::settings::{self, TenantSettings};
use talkops_core::tenant::TenantId;
use talkops_core::trunks::{self, NumberDestination, OutboundRoute, gateway_name};
use talkops_core::voicemail;
use talkops_core::{ivr, numbering, prompts, recordings};
use talkops_core::{queues, time_conditions};
use uuid::Uuid;

use super::{
    CONTEXT_INTERNAL, CONTEXT_PUBLIC, CONTEXT_TRANSFER, SIP_DOMAIN, XmlWriter, sanitize_value,
};

/// The relevant parts of a dialplan lookup request.
#[derive(Debug, Clone, Default)]
pub struct CallRequest {
    pub context: String,
    pub destination: String,
    pub caller_number: String,
    pub caller_name: String,
    vars: HashMap<String, String>,
}

impl CallRequest {
    pub fn from_params(params: &HashMap<String, String>) -> Self {
        let get = |k: &str| params.get(k).cloned().unwrap_or_default();
        let vars = params
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix("variable_")
                    .map(|k| (k.to_owned(), v.clone()))
            })
            .collect();
        Self {
            context: get("Caller-Context"),
            destination: get("Caller-Destination-Number"),
            caller_number: get("Caller-Caller-ID-Number"),
            caller_name: get("Caller-Caller-ID-Name"),
            vars,
        }
    }

    pub fn with_var(mut self, name: &str, value: &str) -> Self {
        self.vars.insert(name.to_owned(), value.to_owned());
        self
    }

    fn var(&self, name: &str) -> Option<&str> {
        self.vars
            .get(name)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    fn uuid_var(&self, name: &str) -> Option<Uuid> {
        self.var(name).and_then(|v| v.parse().ok())
    }
}

/// One dialplan application call.
pub type Action = (&'static str, String);

fn set(name: &str, value: impl Into<String>) -> Action {
    ("set", format!("{name}={}", value.into()))
}

fn reject(code: &str) -> Vec<Action> {
    vec![("respond", code.to_owned())]
}

/// What routing decisions need.
#[derive(Clone, Copy)]
pub struct Routing<'a> {
    pub pool: &'a PgPool,
    pub catalog: &'a PresetCatalog,
    /// Outbound Event Socket address for interactive calls (voicemail).
    pub socket: &'a str,
    /// Shared recordings volume.
    pub recordings: &'a std::path::Path,
    /// Shared sounds volume (system prompts).
    pub sounds: &'a std::path::Path,
}

/// Decides how to handle a call. Never fails open: errors become rejections.
pub async fn plan(r: &Routing<'_>, req: &CallRequest) -> Vec<Action> {
    let result = match req.context.as_str() {
        CONTEXT_INTERNAL => plan_internal(r, req).await,
        CONTEXT_PUBLIC => plan_public(r, req).await,
        CONTEXT_TRANSFER => plan_transfer(r, req).await,
        other => {
            tracing::warn!(context = other, "dialplan request for unknown context");
            Ok(reject("403 Forbidden"))
        }
    };
    let result = match result {
        Ok(actions) if req.context != CONTEXT_TRANSFER => with_recording(r, actions).await,
        other => other,
    };
    result.unwrap_or_else(|err| {
        tracing::error!(error = %err, "dialplan routing failed");
        reject("500 Server Internal Error")
    })
}

/// Value of a variable the plan sets (`set`/`export name=value`).
fn planned_var<'a>(actions: &'a [Action], name: &str) -> Option<&'a str> {
    actions.iter().rev().find_map(|(app, data)| {
        (*app == "set" || *app == "export")
            .then(|| data.strip_prefix(name)?.strip_prefix('='))
            .flatten()
    })
}

/// Adds call recording (and the announcement) in front of the first
/// `bridge`/`callcenter` if the tenant or an involved extension wants it.
async fn with_recording(r: &Routing<'_>, mut actions: Vec<Action>) -> CoreResult<Vec<Action>> {
    let Some(pos) = actions
        .iter()
        .position(|(app, _)| *app == "bridge" || *app == "callcenter")
    else {
        return Ok(actions);
    };
    let (Some(tenant), Some(direction)) = (
        planned_var(&actions, "talkops_tenant_id")
            .and_then(|v| v.parse().ok())
            .map(TenantId),
        planned_var(&actions, "talkops_direction"),
    ) else {
        return Ok(actions);
    };
    let direction = match direction {
        "inbound" => Direction::Inbound,
        "outbound" => Direction::Outbound,
        _ => Direction::Internal,
    };
    let mut involved = Vec::new();
    for var in ["talkops_extension_id", "talkops_dest_extension_id"] {
        if let Some(id) = planned_var(&actions, var).and_then(|v| v.parse().ok()) {
            if let Ok(ext) = extensions::get(r.pool, tenant, id).await {
                involved.push(ext);
            }
        }
    }
    let settings = settings::get(r.pool, tenant).await?;
    if !recordings::should_record(&settings, direction, &involved.iter().collect::<Vec<_>>()) {
        return Ok(actions);
    }
    let file = recordings::recording_file(tenant, chrono::Utc::now(), "${uuid}");
    if let Some(dir) = r.recordings.join(&file).parent() {
        if let Err(err) = std::fs::create_dir_all(dir) {
            tracing::error!(error = %err, "cannot create recording directory; not recording");
            return Ok(actions);
        }
    }
    let mut rec = vec![
        set("talkops_recording", file.clone()),
        set("RECORD_STEREO", "true"),
        set("RECORD_ANSWER_REQ", "true"),
        (
            "record_session",
            r.recordings.join(&file).display().to_string(),
        ),
    ];
    let announcement = settings
        .recording_announcement
        .then(|| prompts::path(r.sounds, "rec_announcement", &settings.default_language))
        .flatten()
        .filter(|p| p.is_file())
        .map(|p| p.display().to_string());
    if let Some(prompt) = announcement {
        if actions[pos].0 == "callcenter" {
            // The queue answered already: tell the caller before waiting.
            rec.push(("playback", prompt));
        } else {
            // Both parties hear it once the call is answered.
            for leg in ["aleg", "bleg"] {
                rec.push(set(&format!("bridge_pre_execute_{leg}_app"), "playback"));
                rec.push(set(
                    &format!("bridge_pre_execute_{leg}_data"),
                    prompt.clone(),
                ));
            }
        }
    }
    let path = r.recordings.join(&file).display().to_string();
    let after = pos + rec.len() + 1;
    actions.splice(pos..pos, rec);
    // Unanswered: whatever follows (voicemail, fallback) is not part of it.
    if after < actions.len() && actions[after - 1].0 == "bridge" {
        actions.insert(after, ("stop_record_session", path));
    }
    Ok(actions)
}

async fn plan_internal(r: &Routing<'_>, req: &CallRequest) -> CoreResult<Vec<Action>> {
    let (pool, catalog) = (r.pool, r.catalog);
    let (Some(tenant), Some(ext_id)) = (
        req.uuid_var("talkops_tenant_id"),
        req.uuid_var("talkops_extension_id"),
    ) else {
        tracing::warn!("internal call without authenticated device");
        return Ok(reject("403 Forbidden"));
    };
    let tenant = TenantId(tenant);
    let Ok(caller) = extensions::get(pool, tenant, ext_id).await else {
        return Ok(reject("403 Forbidden"));
    };
    let settings = settings::get(pool, tenant).await?;
    // BLF keys monitoring a park slot dial `park+*51`.
    let raw_dest = req
        .destination
        .strip_prefix("park+")
        .unwrap_or(&req.destination);
    let dest: String = raw_dest
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+' || *c == '*' || *c == '#')
        .collect();

    let mut actions = vec![
        ("export", format!("talkops_tenant_id={tenant}")),
        // Blind transfers (SIP REFER) are routed by TalkOps, see plan_transfer.
        (
            "export",
            format!("force_transfer_context={CONTEXT_TRANSFER}"),
        ),
        set("talkops_extension_id", caller.id.to_string()),
        set("talkops_caller_number", caller.number.clone()),
        set("talkops_caller_name", sanitize_value(&caller.display_name)),
    ];

    if let Some(feature) = feature_code(r, tenant, &caller, &dest).await? {
        actions.extend(feature);
        return Ok(actions);
    }

    if let Some((kind, id)) = numbering::resolve(pool, tenant, &dest).await? {
        actions.push(set("talkops_direction", "internal"));
        let name = sanitize_value(&caller.display_name);
        actions.extend(route_to(r, tenant, kind, Some(id), &name, 0).await?);
        return Ok(actions);
    }

    let dial_plan = settings.dial_plan();
    let (route_number, dialed_e164, raw_dial, emergency) = match dial_plan.classify(&dest) {
        Dialed::Emergency(n) => (settings.default_number_id, None, n, true),
        Dialed::Service(n) => (
            caller.outbound_number_id.or(settings.default_number_id),
            None,
            n,
            false,
        ),
        Dialed::External(e164) => (
            caller.outbound_number_id.or(settings.default_number_id),
            Some(e164.clone()),
            e164,
            false,
        ),
        Dialed::Invalid => return Ok(reject("404 Not Found")),
    };
    let Some(route_number) = route_number else {
        tracing::warn!(extension = %caller.number, "no outbound number configured");
        return Ok(reject("503 Service Unavailable"));
    };
    let Some(route) = trunks::outbound_route(pool, tenant, route_number).await? else {
        tracing::warn!(extension = %caller.number, "outbound trunk unavailable");
        return Ok(reject("503 Service Unavailable"));
    };

    // Emergency calls present the caller's own number if it is on the
    // emergency trunk, so the control centre can call back the right line.
    let mut caller_e164 = route.e164.clone();
    if emergency {
        if let Some(own) = caller
            .outbound_number_id
            .filter(|id| *id != route.number_id)
        {
            if let Some(own_route) = trunks::outbound_route(pool, tenant, own).await? {
                if own_route.trunk_id == route.trunk_id {
                    caller_e164 = own_route.e164;
                }
            }
        }
    }

    actions.push(set("talkops_direction", "outbound"));
    actions.extend(outbound_actions(
        catalog,
        &dial_plan,
        &route,
        &caller_e164,
        dialed_e164.as_deref(),
        &raw_dial,
        caller.hide_caller_id && !emergency,
    )?);
    Ok(actions)
}

/// Confirmation tone for feature codes (rising two-tone beep).
const CONFIRM_TONE: &str = "tone_stream://%(200,100,800);%(300,0,1200)";

/// Parking lot used for all slots.
const PARK_LOT: &str = "talkops";

/// `*51` … `*59`: call park slots.
fn park_slot(dest: &str) -> Option<&str> {
    let n = dest.strip_prefix("*5")?;
    (n.len() == 1 && ('1'..='9').contains(&n.chars().next()?)).then_some(dest)
}

/// Parks the call in `slot`, or picks up the call parked there.
fn park(slot: &str) -> Vec<Action> {
    vec![
        // Parked calls ring back nobody but give up after 10 minutes.
        set("valet_parking_timeout", "600"),
        ("valet_park", format!("{PARK_LOT} {slot}")),
        ("hangup", String::new()),
    ]
}

/// Falling two-tone beep: a time condition is now forced closed.
const CLOSED_TONE: &str = "tone_stream://%(300,100,1200);%(400,0,600)";

fn confirm() -> Vec<Action> {
    vec![
        ("answer", String::new()),
        ("sleep", "300".to_owned()),
        ("playback", CONFIRM_TONE.to_owned()),
        ("hangup", String::new()),
    ]
}

/// Name of the pickup group every extension's calls are registered in, so
/// `**<ext>` and BLF keys can grab them.
fn pickup_group(ext: &Extension) -> String {
    format!("ext-{}", ext.id.simple())
}

/// Feature codes dialed from a phone. Returns `None` for ordinary numbers.
///
/// - `*78` / `*79`: do not disturb on / off
/// - `*72<number>` / `*73`: unconditional call forwarding on / off
/// - `**<ext>`: pick up a call ringing at `<ext>`
/// - `*97` / `*98`: own voicemail / any voicemail with PIN
/// - `*30<number>`: toggle a time condition between schedule and closed
/// - `*51` … `*59`: park in / pick up from a park slot
async fn feature_code(
    r: &Routing<'_>,
    tenant: TenantId,
    caller: &Extension,
    dest: &str,
) -> CoreResult<Option<Vec<Action>>> {
    let pool = r.pool;
    // Toggles are not calls: no `talkops_direction`, so no CDR is written.
    let mut a = Vec::new();
    if dest == "*97" {
        // Own mailbox, no PIN needed from the extension's own devices.
        if !voicemail::get_box(pool, tenant, caller.id).await?.enabled {
            return Ok(Some(reject("404 Not Found")));
        }
        a.extend(vm_socket(r, "vm_check", caller.id));
        return Ok(Some(a));
    }
    if let Some(slot) = park_slot(dest) {
        a.extend(park(slot));
        return Ok(Some(a));
    }
    if let Some(number) = dest.strip_prefix("*30") {
        // Toggle a time condition between its schedule and "closed".
        let Some((NumberDestination::TimeCondition, id)) =
            numbering::resolve(pool, tenant, number).await?
        else {
            return Ok(Some(reject("404 Not Found")));
        };
        let tc = time_conditions::get(pool, tenant, id).await?;
        let next = if tc.r#override == "auto" {
            "closed"
        } else {
            "auto"
        };
        time_conditions::set_override(pool, tenant, id, next).await?;
        tracing::info!(time_condition = %tc.name, state = next, "override toggled by feature code");
        if next == "closed" {
            a.extend([
                ("answer", String::new()),
                ("sleep", "300".to_owned()),
                ("playback", CLOSED_TONE.to_owned()),
                ("hangup", String::new()),
            ]);
        } else {
            a.extend(confirm());
        }
        return Ok(Some(a));
    }
    if dest == "*98" {
        a.push(set("talkops_app", "vm_login"));
        a.push(set("verbose_events", "true"));
        a.push(("socket", format!("{} async full", r.socket)));
        return Ok(Some(a));
    }
    if dest == "*78" || dest == "*79" {
        extensions::set_dnd(pool, tenant, caller.id, dest == "*78").await?;
        a.extend(confirm());
        return Ok(Some(a));
    }
    if dest == "*73" {
        extensions::set_forward_all(pool, tenant, caller.id, None).await?;
        a.extend(confirm());
        return Ok(Some(a));
    }
    if let Some(target) = dest.strip_prefix("*72") {
        if !extensions::valid_forward_target(target) || target == caller.number {
            return Ok(Some(reject("484 Address Incomplete")));
        }
        extensions::set_forward_all(pool, tenant, caller.id, Some(target)).await?;
        a.extend(confirm());
        return Ok(Some(a));
    }
    if let Some(number) = dest.strip_prefix("**") {
        let Some(target) = extensions::find_by_number(pool, tenant, number).await? else {
            return Ok(Some(reject("404 Not Found")));
        };
        a.push(set("talkops_direction", "internal"));
        a.push(set("talkops_destination", target.number.clone()));
        a.push(("pickup", pickup_group(&target)));
        a.push(("hangup", String::new()));
        return Ok(Some(a));
    }
    Ok(None)
}

/// Rings all devices of `target`, honouring DND and unconditional
/// forwarding (one hop only, so forwarding loops are impossible).
async fn ring_extension(
    r: &Routing<'_>,
    tenant: TenantId,
    target: &Extension,
) -> CoreResult<Vec<Action>> {
    let (pool, catalog) = (r.pool, r.catalog);
    if !target.enabled {
        return Ok(reject("480 Temporarily Unavailable"));
    }
    if target.dnd {
        return Ok(match voicemail_of(r, tenant, target).await? {
            Some(vm) => vm,
            None => reject("486 Busy Here"),
        });
    }
    if let Some(forward) = target.forward_all.as_deref() {
        let mut a = vec![set("talkops_forwarded_from", target.number.clone())];
        if let Some(next) = extensions::find_by_number(pool, tenant, forward).await? {
            if next.id != target.id {
                if !next.enabled {
                    return Ok(reject("480 Temporarily Unavailable"));
                }
                if next.dnd {
                    return Ok(match voicemail_of(r, tenant, &next).await? {
                        Some(vm) => vm,
                        None => reject("486 Busy Here"),
                    });
                }
                a.extend(bridge_devices(r, tenant, &next).await?);
                return Ok(a);
            }
        } else {
            match forward_external(pool, catalog, tenant, target, forward).await? {
                Some(out) => {
                    a.extend(out);
                    return Ok(a);
                }
                None => return Ok(reject("480 Temporarily Unavailable")),
            }
        }
    }
    bridge_devices(r, tenant, target).await
}

/// Hands the call to the TalkOps voicemail application.
fn vm_socket(r: &Routing<'_>, app: &str, extension: Uuid) -> Vec<Action> {
    vec![
        set("talkops_app", app),
        set("talkops_vm_extension_id", extension.to_string()),
        // Channel variables in CHANNEL_EXECUTE_COMPLETE (DTMF results).
        set("verbose_events", "true"),
        ("socket", format!("{} async full", r.socket)),
    ]
}

/// Voicemail deposit for `target`, if its box is enabled.
async fn voicemail_of(
    r: &Routing<'_>,
    tenant: TenantId,
    target: &Extension,
) -> CoreResult<Option<Vec<Action>>> {
    if !voicemail::get_box(r.pool, tenant, target.id).await?.enabled {
        return Ok(None);
    }
    let mut a = vec![
        set("talkops_destination", target.number.clone()),
        set("talkops_dest_extension_id", target.id.to_string()),
    ];
    a.extend(vm_socket(r, "vm_deposit", target.id));
    Ok(Some(a))
}

/// Rings all devices; unanswered, busy or unreachable calls go to voicemail
/// if the box is enabled.
async fn bridge_devices(
    r: &Routing<'_>,
    tenant: TenantId,
    target: &Extension,
) -> CoreResult<Vec<Action>> {
    let devices = extensions::ring_targets(r.pool, target.id).await?;
    let vm = voicemail_of(r, tenant, target).await?;
    if devices.is_empty() {
        return Ok(vm.unwrap_or_else(|| reject("480 Temporarily Unavailable")));
    }
    let dial = devices
        .iter()
        .map(|d| format!("user/{}@{SIP_DOMAIN}", sanitize_value(d)))
        .chain(std::iter::once(format!("pickup/{}", pickup_group(target))))
        .collect::<Vec<_>>()
        .join(",");
    Ok([
        set("talkops_destination", target.number.clone()),
        set("talkops_dest_extension_id", target.id.to_string()),
        set("call_timeout", target.ring_timeout_secs.to_string()),
        set("hangup_after_bridge", "true"),
        set("continue_on_fail", "true"),
        ("bridge", dial),
    ]
    .into_iter()
    .chain(vm.unwrap_or_else(|| vec![("hangup", String::new())]))
    .collect())
}

/// Forwards a call to an external number through the forwarding
/// extension's outbound number (or the tenant default).
async fn forward_external(
    pool: &PgPool,
    catalog: &PresetCatalog,
    tenant: TenantId,
    ext: &Extension,
    number: &str,
) -> CoreResult<Option<Vec<Action>>> {
    let settings = settings::get(pool, tenant).await?;
    let dial_plan = settings.dial_plan();
    let (dialed_e164, raw) = match dial_plan.classify(number) {
        Dialed::External(e164) => (Some(e164.clone()), e164),
        Dialed::Service(n) => (None, n),
        Dialed::Emergency(_) | Dialed::Invalid => return Ok(None),
    };
    let Some(number_id) = ext.outbound_number_id.or(settings.default_number_id) else {
        return Ok(None);
    };
    let Some(route) = trunks::outbound_route(pool, tenant, number_id).await? else {
        return Ok(None);
    };
    // The call keeps its original direction for the call log.
    outbound_actions(
        catalog,
        &dial_plan,
        &route,
        &route.e164,
        dialed_e164.as_deref(),
        &raw,
        false,
    )
    .map(Some)
}

fn codec_string(codecs: &[String]) -> String {
    codecs
        .iter()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| {
            c.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn outbound_actions(
    catalog: &PresetCatalog,
    dial_plan: &DialPlanSettings,
    route: &OutboundRoute,
    caller_e164: &str,
    dialed_e164: Option<&str>,
    raw_dial: &str,
    hide_caller_id: bool,
) -> CoreResult<Vec<Action>> {
    let preset = catalog.get(&route.preset).ok_or_else(|| {
        talkops_core::error::CoreError::Validation(format!("unknown preset {}", route.preset))
    })?;
    let sip = preset
        .effective_sip(&route.overrides)
        .map_err(talkops_core::error::CoreError::Validation)?;
    let dialed = match dialed_e164 {
        Some(e164) => dial_plan.format(e164, sip.number_format),
        None => raw_dial.to_owned(), // emergency/service numbers go out as dialed
    };
    let caller_formatted = dial_plan.format(caller_e164, sip.caller_id_format);
    let domain = sip
        .from_domain
        .clone()
        .or(sip.realm.clone())
        .or(sip.registrar.clone())
        .unwrap_or_default();

    let mut a = vec![
        set("talkops_trunk_id", route.trunk_id.to_string()),
        set("talkops_number_id", route.number_id.to_string()),
        set(
            "talkops_destination",
            dialed_e164.unwrap_or(raw_dial).to_owned(),
        ),
        set("effective_caller_id_number", caller_formatted.clone()),
        set("effective_caller_id_name", caller_formatted.clone()),
        set("absolute_codec_string", codec_string(&sip.codecs)),
        set(
            "dtmf_type",
            match sip.dtmf {
                Dtmf::Rfc2833 => "rfc2833",
                Dtmf::Info => "info",
                Dtmf::Inband => "none",
            },
        ),
    ];
    match sip.caller_id_header {
        CallerIdHeader::Pai => a.push(set("sip_cid_type", "pid")),
        CallerIdHeader::Ppi => {
            a.push(set("sip_cid_type", "none"));
            a.push(set(
                "sip_h_P-Preferred-Identity",
                format!(
                    "<sip:{}@{}>",
                    caller_formatted,
                    domain.split(':').next().unwrap_or_default()
                ),
            ));
        }
        CallerIdHeader::From => a.push(set("sip_cid_type", "none")),
    }
    match sip.srtp {
        Srtp::Off => {}
        Srtp::Optional => a.push(set("rtp_secure_media", "optional")),
        Srtp::Required => a.push(set("rtp_secure_media", "mandatory")),
    }
    if hide_caller_id {
        a.push(set("sip_cid_type", "pid"));
        a.push(("privacy", "full".to_owned()));
    }
    a.push(set("hangup_after_bridge", "true"));
    a.push(set("continue_on_fail", "true"));
    a.push((
        "bridge",
        format!(
            "sofia/gateway/{}/{}",
            gateway_name(route.account_id),
            sanitize_value(&dialed)
        ),
    ));
    a.push(("hangup", String::new()));
    Ok(a)
}

async fn plan_public(r: &Routing<'_>, req: &CallRequest) -> CoreResult<Vec<Action>> {
    let pool = r.pool;
    let gw_tenant = req
        .uuid_var("talkops_tenant_id")
        .map(TenantId)
        .unwrap_or(TenantId::DEFAULT);
    let dial_plan = settings::get(pool, gw_tenant)
        .await
        .map(|s| s.dial_plan())
        .unwrap_or_default();

    // Which of our numbers was called? Try the usual places in order.
    let mut found = None;
    for candidate in [
        Some(req.destination.as_str()),
        req.var("sip_to_user"),
        req.var("sip_req_user"),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(e164) = dial_plan.normalize_incoming(candidate) {
            if let Some(hit) = trunks::find_number(pool, &e164).await? {
                found = Some(hit);
                break;
            }
        }
    }
    // Per-number registrations: the account identifies the number.
    if found.is_none() {
        if let Some(account) = req.uuid_var("talkops_account_id") {
            let numbers = trunks::numbers_of_account(pool, account).await?;
            if numbers.len() == 1 {
                found = numbers.into_iter().next();
            }
        }
    }
    let Some((tenant, number)) = found else {
        tracing::info!(destination = %sanitize_value(&req.destination), "inbound call for unknown number");
        return Ok(reject("404 Not Found"));
    };
    let tenant_settings: TenantSettings = settings::get(pool, tenant).await?;
    let dial_plan = tenant_settings.dial_plan();

    let caller_e164 = dial_plan.normalize_incoming(&req.caller_number);
    let display = caller_e164
        .as_deref()
        .map(|e| dial_plan.for_display(e))
        .unwrap_or_else(|| "anonymous".to_owned());
    let name = Some(sanitize_value(&req.caller_name))
        .filter(|n| !n.is_empty() && *n != req.caller_number)
        .unwrap_or_else(|| display.clone());

    let mut actions = vec![
        ("export", format!("talkops_tenant_id={tenant}")),
        (
            "export",
            format!("force_transfer_context={CONTEXT_TRANSFER}"),
        ),
        set("talkops_direction", "inbound"),
        set("talkops_trunk_id", number.trunk_id.to_string()),
        set("talkops_number_id", number.id.to_string()),
        set(
            "talkops_caller_number",
            caller_e164.clone().unwrap_or_else(|| "anonymous".into()),
        ),
        set("talkops_caller_name", name.clone()),
        set("effective_caller_id_number", display),
        set("effective_caller_id_name", name.clone()),
    ];
    if number.destination_type == NumberDestination::Extension {
        if let Some(ext) = number.destination_id {
            actions.push(set("talkops_extension_id", ext.to_string()));
        }
    }
    let routed = route_to(
        r,
        tenant,
        number.destination_type,
        number.destination_id,
        &name,
        0,
    )
    .await?;
    // A bare rejection needs no call variables.
    if routed.first().is_some_and(|(app, _)| *app == "respond") {
        return Ok(routed);
    }
    actions.extend(routed);
    Ok(actions)
}

/// Target of a transfer from a TalkOps application:
/// `dest:<kind>:<uuid>` or `dial:<internal number>`.
pub fn transfer_target(kind: NumberDestination, id: Uuid) -> String {
    let kind = serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("dest:{kind}:{id}")
}

async fn plan_transfer(r: &Routing<'_>, req: &CallRequest) -> CoreResult<Vec<Action>> {
    let Some(tenant) = req.uuid_var("talkops_tenant_id").map(TenantId) else {
        return Ok(reject("403 Forbidden"));
    };
    let name = req
        .var("talkops_caller_name")
        .unwrap_or_default()
        .to_owned();
    let target = if let Some(rest) = req.destination.strip_prefix("dest:") {
        let (kind, id) = rest.split_once(':').unwrap_or_default();
        let kind: Option<NumberDestination> = serde_json::from_value(serde_json::json!(kind)).ok();
        kind.zip(id.parse::<Uuid>().ok())
    } else if let Some(number) = req.destination.strip_prefix("dial:") {
        numbering::resolve(r.pool, tenant, number).await?
    } else if let Some(slot) = park_slot(
        req.destination
            .strip_prefix("park+")
            .unwrap_or(&req.destination),
    ) {
        return Ok(park(slot));
    } else {
        // Blind transfer by a phone: an internal or an external number.
        let dest: String = req
            .destination
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect();
        match numbering::resolve(r.pool, tenant, &dest).await? {
            Some(target) => Some(target),
            None => return transfer_external(r, tenant, &dest).await,
        }
    };
    let Some((kind, id)) = target else {
        tracing::warn!(destination = %sanitize_value(&req.destination), "invalid transfer target");
        return Ok(reject("404 Not Found"));
    };
    let mut a = vec![set("talkops_tenant_id", tenant.to_string())];
    if kind == NumberDestination::Extension {
        a.push(set("talkops_dest_extension_id", id.to_string()));
    }
    a.extend(route_to(r, tenant, kind, Some(id), &sanitize_value(&name), 1).await?);
    Ok(a)
}

/// Blind transfer to an external number, via the tenant's default number.
async fn transfer_external(
    r: &Routing<'_>,
    tenant: TenantId,
    dest: &str,
) -> CoreResult<Vec<Action>> {
    let settings = settings::get(r.pool, tenant).await?;
    let dial_plan = settings.dial_plan();
    let (dialed_e164, raw) = match dial_plan.classify(dest) {
        Dialed::External(e164) => (Some(e164.clone()), e164),
        Dialed::Service(n) | Dialed::Emergency(n) => (None, n),
        Dialed::Invalid => return Ok(reject("404 Not Found")),
    };
    let Some(number_id) = settings.default_number_id else {
        return Ok(reject("503 Service Unavailable"));
    };
    let Some(route) = trunks::outbound_route(r.pool, tenant, number_id).await? else {
        return Ok(reject("503 Service Unavailable"));
    };
    outbound_actions(
        r.catalog,
        &dial_plan,
        &route,
        &route.e164,
        dialed_e164.as_deref(),
        &raw,
        false,
    )
}

/// Destinations may point to each other (group → fallback → menu …); this
/// bounds the chain so misconfigurations cannot loop.
const MAX_DEPTH: u8 = 5;

type Planned<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = CoreResult<Vec<Action>>> + Send + 'a>>;

/// Actions sending the call to a destination. `caller_name` is the name
/// shown to the called phones (ring groups may prefix it).
pub fn route_to<'a>(
    r: &'a Routing<'a>,
    tenant: TenantId,
    kind: NumberDestination,
    id: Option<Uuid>,
    caller_name: &'a str,
    depth: u8,
) -> Planned<'a> {
    Box::pin(async move {
        let unavailable = || Ok(reject("480 Temporarily Unavailable"));
        if depth > MAX_DEPTH {
            tracing::warn!("destination chain too long, rejecting call");
            return unavailable();
        }
        let Some(id) = id else {
            return unavailable();
        };
        match kind {
            NumberDestination::Extension => match extensions::get(r.pool, tenant, id).await {
                Ok(ext) => ring_extension(r, tenant, &ext).await,
                Err(_) => unavailable(),
            },
            NumberDestination::Voicemail => match extensions::get(r.pool, tenant, id).await {
                Ok(ext) => Ok(voicemail_of(r, tenant, &ext)
                    .await?
                    .unwrap_or_else(|| reject("480 Temporarily Unavailable"))),
                Err(_) => unavailable(),
            },
            NumberDestination::RingGroup => match ring_groups::get(r.pool, tenant, id).await {
                Ok(group) => ring_group(r, tenant, &group, caller_name, depth).await,
                Err(_) => unavailable(),
            },
            NumberDestination::Ivr => {
                if ivr::get(r.pool, tenant, id).await.is_err() {
                    return unavailable();
                }
                Ok(vec![
                    set("talkops_app", "ivr"),
                    set("talkops_ivr_id", id.to_string()),
                    set("verbose_events", "true"),
                    ("socket", format!("{} async full", r.socket)),
                ])
            }
            NumberDestination::Queue => match queues::get(r.pool, tenant, id).await {
                Ok(q) if q.enabled => {
                    let mut a = vec![
                        set(
                            "talkops_destination",
                            q.number.clone().unwrap_or_else(|| sanitize_value(&q.name)),
                        ),
                        set("talkops_queue_id", q.id.to_string()),
                        set("hangup_after_bridge", "true"),
                        ("answer", String::new()),
                        ("callcenter", q.cc_name()),
                    ];
                    // Back here only if the caller left the queue unanswered.
                    let after = route_to(
                        r,
                        tenant,
                        q.timeout_type,
                        q.timeout_id,
                        caller_name,
                        depth + 1,
                    )
                    .await?;
                    if after.first().is_some_and(|(app, _)| *app == "respond") {
                        a.push(("hangup", String::new()));
                    } else {
                        a.extend(after);
                    }
                    Ok(a)
                }
                Ok(q) => {
                    route_to(
                        r,
                        tenant,
                        q.timeout_type,
                        q.timeout_id,
                        caller_name,
                        depth + 1,
                    )
                    .await
                }
                Err(_) => unavailable(),
            },
            NumberDestination::TimeCondition => {
                let Ok(tc) = time_conditions::get(r.pool, tenant, id).await else {
                    return unavailable();
                };
                let zone = settings::get(r.pool, tenant).await?.timezone;
                let state = tc.state(chrono::Utc::now(), &zone);
                let (kind, target) = tc.destination(state.open);
                let mut a = vec![set(
                    "talkops_time_condition",
                    if state.open { "open" } else { "closed" },
                )];
                a.extend(route_to(r, tenant, kind, target, caller_name, depth + 1).await?);
                Ok(a)
            }
            _ => unavailable(),
        }
    })
}

/// Rings the members of a group; unanswered calls go to its fallback.
async fn ring_group(
    r: &Routing<'_>,
    tenant: TenantId,
    group: &RingGroup,
    caller_name: &str,
    depth: u8,
) -> CoreResult<Vec<Action>> {
    let fallback = || {
        route_to(
            r,
            tenant,
            group.fallback_type,
            group.fallback_id,
            caller_name,
            depth + 1,
        )
    };
    if !group.enabled {
        return fallback().await;
    }
    let timeout = group.ring_timeout_secs;
    let mut legs: Vec<Vec<String>> = Vec::new();
    for member in &group.members {
        let Ok(ext) = extensions::get(r.pool, tenant, *member).await else {
            continue;
        };
        if !ext.enabled || ext.dnd {
            continue;
        }
        let mut endpoints: Vec<String> = extensions::ring_targets(r.pool, ext.id)
            .await?
            .iter()
            .map(|d| format!("user/{}@{SIP_DOMAIN}", sanitize_value(d)))
            .collect();
        if endpoints.is_empty() {
            continue;
        }
        endpoints.push(format!("pickup/{}", pickup_group(&ext)));
        legs.push(endpoints);
    }
    if legs.is_empty() {
        return fallback().await;
    }
    let dial = if group.strategy == "sequential" {
        legs.iter()
            .map(|eps| {
                eps.iter()
                    .map(|e| format!("[leg_timeout={timeout}]{e}"))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join("|")
    } else {
        legs.concat().join(",")
    };
    let mut a = vec![
        set(
            "talkops_destination",
            group
                .number
                .clone()
                .unwrap_or_else(|| sanitize_value(&group.name)),
        ),
        set("talkops_ring_group_id", group.id.to_string()),
    ];
    if !group.caller_id_prefix.is_empty() {
        a.push(set(
            "effective_caller_id_name",
            format!(
                "{}{}{caller_name}",
                sanitize_value(&group.caller_id_prefix),
                // Keep the separator space sanitizing trims away.
                if group.caller_id_prefix.ends_with(' ') {
                    " "
                } else {
                    ""
                }
            ),
        ));
    }
    if group.strategy != "sequential" {
        a.push(set("call_timeout", timeout.to_string()));
    }
    a.push(set("hangup_after_bridge", "true"));
    a.push(set("continue_on_fail", "true"));
    a.push(("bridge", dial));
    let after = fallback().await?;
    if after.first().is_some_and(|(app, _)| *app == "respond") {
        a.push(("hangup", String::new()));
    } else {
        a.extend(after);
    }
    Ok(a)
}

/// Renders the dialplan response for `context`.
pub fn render(context: &str, actions: &[Action]) -> String {
    let mut w = XmlWriter::document();
    w.open(
        "section",
        &[("name", "dialplan"), ("description", "TalkOps")],
    );
    w.open("context", &[("name", context)]);
    w.open("extension", &[("name", "talkops")]);
    w.open("condition", &[]);
    for (app, data) in actions {
        if data.is_empty() {
            w.empty("action", &[("application", app)]);
        } else {
            w.empty("action", &[("application", app), ("data", data)]);
        }
    }
    w.close("condition");
    w.close("extension");
    w.close("context");
    w.close("section");
    w.finish()
}

/// Helper for tests and the API: format a number the way a trunk expects it.
pub fn format_for(dial_plan: &DialPlanSettings, e164: &str, format: NumberFormat) -> String {
    dial_plan.format(e164, format)
}
