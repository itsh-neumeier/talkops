//! Smart Attendant call engine: walks the flow of
//! [`talkops_core::attendant`] step by step over the outbound Event Socket.
//!
//! Steps that leave TalkOps hand the call back to the dialplan with
//! `transfer <target> XML talkops`:
//! - forward: `dest:<kind>:<id>` (or `dial:<number>` for direct dialing),
//!   routed by [`crate::fsxml::dialplan`] like any other destination;
//! - ring phones: `attendant:<attendant>:<step>`; the dialplan rings the
//!   extensions and, if nobody answers, sends the call back here with
//!   `talkops_attendant_step` set to the step after it;
//! - park: `park+*5N`.

use talkops_core::attendant::{self, Node};
use talkops_core::tenant::TenantId;
use talkops_core::{audio, numbering, settings, time_conditions};
use talkops_esl::EslError;
use uuid::Uuid;

use crate::fsxml::CONTEXT_TRANSFER;
use crate::fsxml::dialplan::transfer_target;
use crate::voicemail::ivr::{Call, Ivr, Seq};
use crate::voicemail::{FlowError, VmContext, store_message};

/// How an interactive call ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// TalkOps is done with the call; it may be hung up.
    Done,
    /// The call continues in the dialplan; it must not be hung up.
    Transferred,
}

/// Steps per call; ends flows that loop without the caller doing anything.
const MAX_STEPS: usize = 50;

/// Transfer target for the "ring phones" step `step`.
pub fn ring_target(attendant: Uuid, step: &str) -> String {
    format!("attendant:{attendant}:{step}")
}

/// Regex accepted by play_and_get_digits for a menu.
fn choice_regex(digits: &[&str], direct_dial: bool) -> String {
    let keys: String = digits.concat();
    match (keys.is_empty(), direct_dial) {
        (true, true) => "^\\d{2,8}$".to_owned(),
        (false, true) => format!("^([{keys}]|\\d{{2,8}})$"),
        // Nothing to choose: wait for the timeout only.
        (true, false) => "^$".to_owned(),
        (false, false) => format!("^[{keys}]$"),
    }
}

async fn transfer<C: Call>(call: &mut C, target: &str) -> Result<Outcome, FlowError> {
    match call
        .execute("transfer", &format!("{target} XML {CONTEXT_TRANSFER}"))
        .await
    {
        Ok(_) => {
            call.release().await;
            Ok(Outcome::Transferred)
        }
        Err(EslError::Hangup) => Ok(Outcome::Transferred),
        Err(err) => Err(err.into()),
    }
}

/// Runs the attendant `id` for the caller, from the first step or `start`.
pub async fn run<C: Call>(
    call: &mut C,
    ctx: &VmContext,
    tenant: TenantId,
    id: Uuid,
    start: Option<&str>,
) -> Result<Outcome, FlowError> {
    let att = attendant::get(&ctx.db, tenant, id).await?;
    let tenant_settings = settings::get(&ctx.db, tenant).await?;
    let lang = att
        .language
        .clone()
        .unwrap_or(tenant_settings.default_language.clone());
    let index = att.flow.index();
    call.execute("answer", "").await?;
    if start.is_none() {
        call.execute("playback", "silence_stream://500").await?;
    }
    let clip = |clip_id: &Option<Uuid>| {
        clip_id
            .map(|c| ctx.media.sounds.join(audio::clip_file(tenant, c)))
            .filter(|p| p.is_file())
    };

    let mut ivr = Ivr::new(call, &ctx.media.sounds, &lang);
    let mut node: Option<&Node> = Some(
        start
            .and_then(|s| index.get(s).copied())
            .unwrap_or(&att.flow.0),
    );
    for _ in 0..MAX_STEPS {
        let Some(current) = node else {
            // The end of a branch.
            let bye = ivr.keys(&["vm_goodbye"]);
            ivr.play(&bye).await?;
            return Ok(Outcome::Done);
        };
        if ivr.call.is_hung_up() {
            return Ok(Outcome::Done);
        }
        node = match current {
            Node::Play { clip_id, next, .. } => {
                let mut seq = Seq::default();
                if let Some(path) = clip(clip_id) {
                    ivr.file(&mut seq, &path);
                }
                ivr.play(&seq).await?;
                next.as_deref()
            }
            Node::Menu {
                clip_id,
                timeout_secs,
                max_tries,
                direct_dial,
                options,
                timeout,
                ..
            } => {
                let mut prompt = Seq::default();
                if let Some(path) = clip(clip_id) {
                    ivr.file(&mut prompt, &path);
                }
                let digits: Vec<&str> = options.iter().map(|o| o.digit.as_str()).collect();
                let max = if *direct_dial { 8 } else { 1 };
                let choice = ivr
                    .ask_with(
                        &prompt,
                        max,
                        &choice_regex(&digits, *direct_dial),
                        *max_tries as u32,
                        *timeout_secs as u32 * 1000,
                    )
                    .await?;
                match choice {
                    Some(key) => match options.iter().find(|o| o.digit == key) {
                        Some(option) => {
                            tracing::info!(attendant = %att.name, key, "attendant choice");
                            option.next.as_deref()
                        }
                        None if *direct_dial
                            && numbering::resolve(&ctx.db, tenant, &key).await?.is_some() =>
                        {
                            tracing::info!(attendant = %att.name, number = %key, "direct dial");
                            return transfer(ivr.call, &format!("dial:{key}")).await;
                        }
                        None => timeout.as_deref(),
                    },
                    None => timeout.as_deref(),
                }
            }
            Node::Ring { id: step, .. } => {
                return transfer(ivr.call, &ring_target(att.id, step)).await;
            }
            Node::Schedule {
                time_condition_id,
                open,
                closed,
                ..
            } => {
                let is_open = match time_condition_id {
                    Some(tc) => match time_conditions::get(&ctx.db, tenant, *tc).await {
                        Ok(tc) => tc.state(chrono::Utc::now(), &tenant_settings.timezone).open,
                        // A deleted schedule counts as closed.
                        Err(_) => false,
                    },
                    None => false,
                };
                if is_open { open } else { closed }.as_deref()
            }
            Node::Voicemail {
                recipients,
                clip_id,
                max_message_secs,
                ..
            } => {
                ivr.set("playback_terminators", "#").await?;
                let mut greeting = Seq::default();
                match clip(clip_id) {
                    Some(path) => ivr.file(&mut greeting, &path),
                    None => ivr.prompt(&mut greeting, "vm_greeting_default"),
                }
                ivr.play(&greeting).await?;
                store_message(&mut ivr, ctx, tenant, recipients, *max_message_secs).await?;
                return Ok(Outcome::Done);
            }
            Node::Park { .. } => {
                return match ctx.telephony.reserve_park_slot().await {
                    Some(slot) => {
                        tracing::info!(attendant = %att.name, slot, "attendant parks call");
                        transfer(ivr.call, &format!("park+{slot}")).await
                    }
                    None => {
                        tracing::warn!(attendant = %att.name, "no free park slot");
                        let bye = ivr.keys(&["vm_goodbye"]);
                        ivr.play(&bye).await?;
                        Ok(Outcome::Done)
                    }
                };
            }
            Node::Transfer {
                destination_type,
                destination_id: Some(target),
                ..
            } => {
                return transfer(ivr.call, &transfer_target(*destination_type, *target)).await;
            }
            Node::Transfer { .. } => None,
            Node::Goto { target, .. } => index.get(target.as_str()).copied(),
            Node::Hangup { .. } => {
                let bye = ivr.keys(&["vm_goodbye"]);
                ivr.play(&bye).await?;
                return Ok(Outcome::Done);
            }
        };
    }
    tracing::warn!(attendant = %att.name, "attendant flow loops, hanging up");
    Ok(Outcome::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regexes() {
        assert_eq!(choice_regex(&["1", "2", "*"], false), "^[12*]$");
        assert_eq!(choice_regex(&["1", "0"], true), "^([10]|\\d{2,8})$");
        assert_eq!(choice_regex(&[], true), "^\\d{2,8}$");
        assert_eq!(choice_regex(&[], false), "^$");
        assert_eq!(
            ring_target(Uuid::nil(), "sales"),
            format!("attendant:{}:sales", Uuid::nil())
        );
    }
}
