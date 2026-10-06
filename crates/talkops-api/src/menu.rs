//! IVR menu flow: play the greeting, read a choice, transfer the caller.
//!
//! Choices are handed back to the dialplan with
//! `transfer dest:<kind>:<id> XML talkops` (or `dial:<number>` for direct
//! dialing), where [`crate::fsxml::dialplan`] routes them like any other
//! destination.

use talkops_core::tenant::TenantId;
use talkops_core::trunks::NumberDestination;
use talkops_core::{ivr, numbering, settings};
use talkops_esl::EslError;
use uuid::Uuid;

use crate::fsxml::CONTEXT_TRANSFER;
use crate::fsxml::dialplan::transfer_target;
use crate::voicemail::ivr::{Call, Ivr, Seq};
use crate::voicemail::{FlowError, VmContext};

/// How an interactive call ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// TalkOps is done with the call; it may be hung up.
    Done,
    /// The call continues in the dialplan; it must not be hung up.
    Transferred,
}

/// Regex accepted by play_and_get_digits for this menu.
fn choice_regex(menu: &ivr::IvrMenu) -> String {
    let keys: String = menu.options.iter().map(|o| o.digit.as_str()).collect();
    match (keys.is_empty(), menu.direct_dial) {
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

/// Runs the menu `menu_id` for the caller.
pub async fn run<C: Call>(
    call: &mut C,
    ctx: &VmContext,
    tenant: TenantId,
    menu_id: Uuid,
) -> Result<Outcome, FlowError> {
    let menu = ivr::get(&ctx.db, tenant, menu_id).await?;
    let lang = match menu.language.clone() {
        Some(l) => l,
        None => settings::get(&ctx.db, tenant).await?.default_language,
    };
    call.execute("answer", "").await?;
    call.execute("playback", "silence_stream://500").await?;

    let mut ivr = Ivr::new(call, &ctx.media.sounds, &lang);
    let mut greeting = Seq::default();
    if menu.greeting_status == "ready" {
        ivr.file(
            &mut greeting,
            &ctx.media.sounds.join(ivr::greeting_file(tenant, menu.id)),
        );
    }
    let max = if menu.direct_dial { 8 } else { 1 };
    let choice = ivr
        .ask_with(
            &greeting,
            max,
            &choice_regex(&menu),
            menu.max_tries as u32,
            menu.timeout_secs as u32 * 1000,
        )
        .await?;

    if let Some(choice) = choice {
        if let Some(option) = menu.option(&choice) {
            if let Some(id) = option.id {
                tracing::info!(menu = %menu.name, choice, "IVR choice");
                return transfer(ivr.call, &transfer_target(option.kind, id)).await;
            }
        } else if menu.direct_dial
            && numbering::resolve(&ctx.db, tenant, &choice)
                .await?
                .is_some()
        {
            tracing::info!(menu = %menu.name, number = %choice, "IVR direct dial");
            return transfer(ivr.call, &format!("dial:{choice}")).await;
        }
    }
    match (menu.timeout_type, menu.timeout_id) {
        (kind, Some(id)) if kind != NumberDestination::None => {
            transfer(ivr.call, &transfer_target(kind, id)).await
        }
        _ => {
            let bye = ivr.keys(&["vm_goodbye"]);
            ivr.play(&bye).await?;
            Ok(Outcome::Done)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkops_core::ivr::{IvrMenu, MenuOption};

    fn menu(options: &[&str], direct: bool) -> IvrMenu {
        IvrMenu {
            id: Uuid::nil(),
            number: None,
            name: "Main".into(),
            language: None,
            greeting: "tts".into(),
            greeting_text: "Hallo".into(),
            greeting_status: "ready".into(),
            timeout_secs: 5,
            max_tries: 3,
            direct_dial: direct,
            options: sqlx::types::Json(
                options
                    .iter()
                    .map(|d| MenuOption {
                        digit: d.to_string(),
                        kind: NumberDestination::Extension,
                        id: Some(Uuid::nil()),
                    })
                    .collect(),
            ),
            timeout_type: NumberDestination::None,
            timeout_id: None,
        }
    }

    #[test]
    fn regexes() {
        assert_eq!(choice_regex(&menu(&["1", "2", "*"], false)), "^[12*]$");
        assert_eq!(choice_regex(&menu(&["1", "0"], true)), "^([10]|\\d{2,8})$");
        assert_eq!(choice_regex(&menu(&[], true)), "^\\d{2,8}$");
        assert_eq!(choice_regex(&menu(&[], false)), "^$");
    }
}
