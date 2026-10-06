//! Voicemail call flows, run over the outbound Event Socket:
//!
//! - `vm_deposit`: greeting, beep, record, store, notify (MWI, mail).
//! - `vm_check` (`*97`): the caller's own box, no PIN.
//! - `vm_login` (`*98`): any box after extension number and PIN.
//!
//! [`handle`] also dispatches IVR menus (`ivr`, see [`crate::menu`]).
//!
//! Prompts come from the media worker (`talkops_core::prompts`); missing
//! prompt files are skipped, so a box keeps working without TTS.

pub mod ivr;

use std::path::PathBuf;
use std::sync::Arc;

use sqlx::PgPool;
use talkops_core::error::CoreResult;
use talkops_core::tenant::TenantId;
use talkops_core::voicemail::{self, NewMessage, VoicemailBox};
use talkops_core::{extensions, settings};
use talkops_esl::EslError;
use talkops_esl::outbound::OutboundSession;
use uuid::Uuid;

use crate::fsxml::sanitize_value;
use crate::menu::Outcome;
use crate::telephony::Telephony;
use crate::{AppState, MediaPaths};
use ivr::{Call, Ivr, Seq};

/// Messages shorter than this are discarded (hang-ups at the greeting).
const MIN_MESSAGE_SECS: u32 = 1;

/// What the flows need from the server.
#[derive(Clone)]
pub struct VmContext {
    pub db: PgPool,
    pub secrets: talkops_core::crypto::SecretBox,
    pub media: Arc<MediaPaths>,
    pub telephony: Telephony,
}

impl From<&AppState> for VmContext {
    fn from(state: &AppState) -> Self {
        Self {
            db: state.db.clone(),
            secrets: state.secrets.clone(),
            media: state.media.clone(),
            telephony: state.telephony.clone(),
        }
    }
}

impl VmContext {
    fn path(&self, relative: &str) -> PathBuf {
        self.media.voicemail.join(relative)
    }
}

/// Entry point for calls handed over by FreeSWITCH.
pub async fn handle(mut session: OutboundSession, ctx: VmContext) {
    let app = session.var("talkops_app").unwrap_or_default().to_owned();
    let uuid = session.uuid().to_owned();
    tracing::info!(%uuid, app, "interactive call");
    let result = match app.as_str() {
        "vm_deposit" | "vm_check" | "vm_login" => {
            run(&mut session, &ctx, &app).await.map(|()| Outcome::Done)
        }
        "ivr" => run_menu(&mut session, &ctx).await,
        "door_open" => open_door(&mut session, &ctx).await.map(|()| Outcome::Done),
        other => {
            tracing::warn!(%uuid, app = other, "unknown interactive application");
            Ok(Outcome::Done)
        }
    };
    match result {
        Ok(Outcome::Transferred) => return,
        Ok(Outcome::Done) | Err(FlowError::Esl(EslError::Hangup)) => {}
        Err(err) => tracing::error!(%uuid, app, error = %err, "interactive call failed"),
    }
    session.hangup("NORMAL_CLEARING").await;
}

#[derive(Debug, thiserror::Error)]
pub enum FlowError {
    #[error(transparent)]
    Esl(#[from] EslError),
    #[error(transparent)]
    Core(#[from] talkops_core::error::CoreError),
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for FlowError {
    fn from(err: std::io::Error) -> Self {
        FlowError::Esl(EslError::Io(err))
    }
}

type FlowResult<T> = Result<T, FlowError>;

async fn run_menu<C: Call>(call: &mut C, ctx: &VmContext) -> FlowResult<Outcome> {
    let tenant = call
        .var("talkops_tenant_id")
        .and_then(|v| v.parse().ok())
        .map(TenantId)
        .ok_or_else(|| FlowError::Other("call without tenant".into()))?;
    let menu = call
        .var("talkops_ivr_id")
        .and_then(|v| v.parse::<Uuid>().ok())
        .ok_or_else(|| FlowError::Other("call without menu".into()))?;
    crate::menu::run(call, ctx, tenant, menu).await
}

/// `*85`/`*86`: opens a door and says whether it worked.
async fn open_door<C: Call>(call: &mut C, ctx: &VmContext) -> FlowResult<()> {
    let var = |c: &C, n: &str| c.var(n).and_then(|v| v.parse::<Uuid>().ok());
    let (Some(tenant), Some(station), Some(caller)) = (
        var(call, "talkops_tenant_id").map(TenantId),
        var(call, "talkops_open_door_id"),
        var(call, "talkops_extension_id"),
    ) else {
        return Err(FlowError::Other("door open without tenant/station".into()));
    };
    let door: i16 = call
        .var("talkops_door")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let station = talkops_core::doors::get(&ctx.db, tenant, station).await?;
    let lang = settings::get(&ctx.db, tenant).await?.default_language;
    let door_ctx = crate::doors::DoorCtx {
        db: ctx.db.clone(),
        secrets: ctx.secrets.clone(),
        media: ctx.media.clone(),
    };
    let ok = door_ctx
        .open(
            tenant,
            &station,
            door,
            serde_json::json!({ "extension_id": caller }),
        )
        .await
        .is_ok();
    call.execute("answer", "").await?;
    call.execute("playback", "silence_stream://300").await?;
    let mut ivr = Ivr::new(call, &ctx.media.sounds, &lang);
    let seq = ivr.keys(&[if ok { "door_opened" } else { "door_failed" }]);
    ivr.play(&seq).await?;
    Ok(())
}

/// Runs one voicemail application on `call`.
pub async fn run<C: Call>(call: &mut C, ctx: &VmContext, app: &str) -> FlowResult<()> {
    let tenant = call
        .var("talkops_tenant_id")
        .and_then(|v| v.parse().ok())
        .map(TenantId)
        .ok_or_else(|| FlowError::Other("call without tenant".into()))?;
    let default_lang = settings::get(&ctx.db, tenant).await?.default_language;
    let ext = call
        .var("talkops_vm_extension_id")
        .and_then(|v| v.parse::<Uuid>().ok());
    match (app, ext) {
        ("vm_deposit", Some(ext)) => deposit(call, ctx, tenant, ext, &default_lang).await,
        ("vm_check", Some(ext)) => {
            let vbox = voicemail::get_box(&ctx.db, tenant, ext).await?;
            let lang = vbox.language.clone().unwrap_or(default_lang);
            call.execute("answer", "").await?;
            call.execute("playback", "silence_stream://500").await?;
            mailbox(call, ctx, tenant, ext, &vbox, &lang).await
        }
        ("vm_login", _) => login(call, ctx, tenant, &default_lang).await,
        _ => Err(FlowError::Other(format!("{app} without mailbox"))),
    }
}

/// Leaves a message in the box of `ext`.
async fn deposit<C: Call>(
    call: &mut C,
    ctx: &VmContext,
    tenant: TenantId,
    ext: Uuid,
    default_lang: &str,
) -> FlowResult<()> {
    let vbox = voicemail::get_box(&ctx.db, tenant, ext).await?;
    if !vbox.enabled {
        return Ok(());
    }
    let lang = vbox.language.clone().unwrap_or(default_lang.to_owned());
    let mut ivr = Ivr::new(call, &ctx.media.sounds, &lang);
    ivr.call.execute("answer", "").await?;
    ivr.call.execute("playback", "silence_stream://500").await?;
    // `#` skips the greeting.
    ivr.set("playback_terminators", "#").await?;
    let mut greeting = Seq::default();
    let custom = ctx.path(&voicemail::greeting_file(tenant, ext));
    if vbox.uses_custom_greeting() && custom.is_file() {
        ivr.file(&mut greeting, &custom);
    } else {
        ivr.prompt(&mut greeting, "vm_greeting_default");
    }
    ivr.play(&greeting).await?;
    let mut beep = Seq::default();
    ivr.beep(&mut beep);
    ivr.play(&beep).await?;

    let id = Uuid::new_v4();
    let path = ctx.path(&voicemail::message_file(tenant, ext, id));
    let secs = ivr.record(&path, vbox.max_message_secs).await?;
    if secs < MIN_MESSAGE_SECS {
        let _ = tokio::fs::remove_file(&path).await;
        return Ok(());
    }
    let caller_number = ivr
        .call
        .var("talkops_caller_number")
        .map(|n| sanitize_value(&n))
        .unwrap_or_default();
    let caller_name = ivr
        .call
        .var("talkops_caller_name")
        .map(|n| sanitize_value(&n))
        .filter(|n| *n != caller_number)
        .unwrap_or_default();
    voicemail::create_message(
        &ctx.db,
        tenant,
        &NewMessage {
            id,
            extension_id: ext,
            caller_number,
            caller_name,
            duration_secs: secs as i32,
            call_uuid: ivr.call.var("Unique-ID").or_else(|| ivr.call.var("uuid")),
        },
    )
    .await?;
    tracing::info!(extension = %ext, secs, "voicemail stored");
    update_mwi(ctx, ext).await;
    if !ivr.call.is_hung_up() {
        let bye = ivr.keys(&["vm_saved"]);
        ivr.play(&bye).await?;
    }
    Ok(())
}

/// `*98`: extension number and PIN, three attempts.
async fn login<C: Call>(
    call: &mut C,
    ctx: &VmContext,
    tenant: TenantId,
    lang: &str,
) -> FlowResult<()> {
    call.execute("answer", "").await?;
    call.execute("playback", "silence_stream://500").await?;
    for _ in 0..3 {
        let mut ivr = Ivr::new(call, &ctx.media.sounds, lang);
        let ask_box = ivr.keys(&["vm_enter_box"]);
        let Some(number) = ivr.ask(&ask_box, 10, "^\\d+$").await? else {
            continue;
        };
        let ask_pin = ivr.keys(&["vm_enter_pin"]);
        let pin = ivr.ask(&ask_pin, 10, "^\\d+$").await?.unwrap_or_default();
        if let Some(ext) = extensions::find_by_number(&ctx.db, tenant, &number).await? {
            let vbox = voicemail::get_box(&ctx.db, tenant, ext.id).await?;
            if vbox.enabled && vbox.verify_pin(&pin) {
                let box_lang = vbox.language.clone().unwrap_or(lang.to_owned());
                return mailbox(call, ctx, tenant, ext.id, &vbox, &box_lang).await;
            }
        }
        tracing::info!(box_number = %sanitize_value(&number), "voicemail login failed");
        let failed = ivr.keys(&["vm_login_failed"]);
        ivr.play(&failed).await?;
    }
    let mut ivr = Ivr::new(call, &ctx.media.sounds, lang);
    let bye = ivr.keys(&["vm_goodbye"]);
    ivr.play(&bye).await?;
    Ok(())
}

/// Main menu of a box: announce counts, listen (1), record greeting (5).
async fn mailbox<C: Call>(
    call: &mut C,
    ctx: &VmContext,
    tenant: TenantId,
    ext: Uuid,
    vbox: &VoicemailBox,
    lang: &str,
) -> FlowResult<()> {
    let mut ivr = Ivr::new(call, &ctx.media.sounds, lang);
    let mut announce = true;
    loop {
        let mut menu = Seq::default();
        if announce {
            let (new, saved) = voicemail::counts(&ctx.db, ext).await?;
            counts_prompt(&ivr, &mut menu, new, saved);
            announce = false;
        }
        ivr.prompt(&mut menu, "vm_main_menu");
        match ivr.ask(&menu, 1, "^[15*]$").await?.as_deref() {
            Some("1") => {
                listen(&mut ivr, ctx, tenant, ext).await?;
                announce = true;
            }
            Some("5") => record_greeting(&mut ivr, ctx, tenant, ext, vbox).await?,
            _ => break,
        }
    }
    let bye = ivr.keys(&["vm_goodbye"]);
    ivr.play(&bye).await?;
    Ok(())
}

/// "You have 2 new messages and one saved message."
fn counts_prompt<C: Call>(ivr: &Ivr<'_, C>, seq: &mut Seq, new: u32, saved: u32) {
    if new == 0 && saved == 0 {
        ivr.prompt(seq, "vm_no_new_messages");
        return;
    }
    ivr.prompt(seq, "vm_you_have");
    if new > 0 {
        if new == 1 {
            ivr.prompt(seq, "vm_one_new_message");
        } else {
            ivr.number(seq, new);
            ivr.prompt(seq, "vm_new_messages");
        }
    }
    if saved > 0 {
        if new > 0 {
            ivr.prompt(seq, "vm_and");
        }
        if saved == 1 {
            ivr.prompt(seq, "vm_one_saved_message");
        } else {
            ivr.number(seq, saved);
            ivr.prompt(seq, "vm_saved_messages");
        }
    }
}

/// Plays all messages, new ones first: 1 repeat, 7 delete, 9 save, # next.
async fn listen<C: Call>(
    ivr: &mut Ivr<'_, C>,
    ctx: &VmContext,
    tenant: TenantId,
    ext: Uuid,
) -> FlowResult<()> {
    let messages = voicemail::list_messages(&ctx.db, tenant, ext).await?;
    for (i, msg) in messages.iter().enumerate() {
        loop {
            let mut seq = Seq::default();
            ivr.prompt(&mut seq, "vm_message");
            ivr.number(&mut seq, i as u32 + 1);
            let digits: String = msg
                .caller_number
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            if !digits.is_empty() {
                ivr.prompt(&mut seq, "vm_from");
                ivr.digits(&mut seq, &msg.caller_number);
            }
            ivr.file(&mut seq, &ctx.path(&msg.file));
            ivr.play(&seq).await?;
            let menu = ivr.keys(&["vm_message_menu"]);
            match ivr.ask(&menu, 1, "^[179#]$").await?.as_deref() {
                Some("1") => continue,
                Some("7") => {
                    delete_message(ctx, tenant, msg.id).await?;
                    let done = ivr.keys(&["vm_deleted"]);
                    ivr.play(&done).await?;
                }
                choice => {
                    voicemail::set_message_status(&ctx.db, tenant, msg.id, true).await?;
                    if choice == Some("9") {
                        let done = ivr.keys(&["vm_message_saved"]);
                        ivr.play(&done).await?;
                    }
                }
            }
            update_mwi(ctx, ext).await;
            break;
        }
    }
    let done = ivr.keys(&["vm_no_more_messages"]);
    ivr.play(&done).await?;
    Ok(())
}

/// Records a new greeting on the phone; it replaces any previous one.
async fn record_greeting<C: Call>(
    ivr: &mut Ivr<'_, C>,
    ctx: &VmContext,
    tenant: TenantId,
    ext: Uuid,
    vbox: &VoicemailBox,
) -> FlowResult<()> {
    let mut seq = ivr.keys(&["vm_record_greeting"]);
    ivr.beep(&mut seq);
    ivr.play(&seq).await?;
    let path = ctx.path(&voicemail::greeting_file(tenant, ext));
    let tmp = ivr::temp_path(&path);
    let secs = ivr.record(&tmp, vbox.max_message_secs.min(120)).await?;
    if secs < MIN_MESSAGE_SECS {
        let _ = tokio::fs::remove_file(&tmp).await;
        let invalid = ivr.keys(&["vm_invalid"]);
        ivr.play(&invalid).await?;
        return Ok(());
    }
    tokio::fs::rename(&tmp, &path).await?;
    voicemail::set_recorded_greeting(&ctx.db, tenant, ext).await?;
    let saved = ivr.keys(&["vm_greeting_saved"]);
    ivr.play(&saved).await?;
    Ok(())
}

/// Deletes a message and its recording.
pub async fn delete_message(ctx: &VmContext, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let msg = voicemail::delete_message(&ctx.db, tenant, id).await?;
    if let Err(err) = tokio::fs::remove_file(ctx.path(&msg.file)).await {
        tracing::warn!(error = %err, file = %msg.file, "could not remove voicemail file");
    }
    Ok(())
}

/// Sends the current message counts to all devices of the extension.
pub async fn update_mwi(ctx: &VmContext, ext: Uuid) {
    let result = async {
        let (new, saved) = voicemail::counts(&ctx.db, ext).await?;
        for user in voicemail::mwi_targets(&ctx.db, ext).await? {
            ctx.telephony.send_mwi(&user, new, saved).await;
        }
        CoreResult::Ok(())
    }
    .await;
    if let Err(err) = result {
        tracing::warn!(error = %err, "MWI update failed");
    }
}

#[cfg(test)]
mod tests;
