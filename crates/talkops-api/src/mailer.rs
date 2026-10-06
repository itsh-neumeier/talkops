//! Outgoing mail: voicemail notifications (job `mail.voicemail`) and the
//! SMTP test from the settings page. Runs in the TalkOps server because it
//! holds the key for the SMTP password.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::jobs::{self, Job};
use talkops_core::mail::{self, SmtpConfig};
use talkops_core::settings;
use talkops_core::voicemail::{self, MailInfo};

use crate::MediaPaths;

/// Mail attachments above this size are left out (the message stays in TalkOps).
const MAX_ATTACHMENT: u64 = 10 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("invalid address: {0}")]
    Address(String),
    #[error("SMTP: {0}")]
    Smtp(String),
    #[error("{0}")]
    Other(String),
}

impl From<talkops_core::error::CoreError> for MailError {
    fn from(err: talkops_core::error::CoreError) -> Self {
        MailError::Other(err.to_string())
    }
}

/// SMTP transport for the configured security mode.
pub fn transport(cfg: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>, MailError> {
    let smtp = |e: lettre::transport::smtp::Error| MailError::Smtp(e.to_string());
    let builder = match cfg.security.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host).map_err(smtp)?,
        "starttls" => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host).map_err(smtp)?
        }
        _ => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host),
    };
    let mut builder = builder
        .port(cfg.port)
        .timeout(Some(Duration::from_secs(20)));
    if !cfg.username.is_empty() {
        builder = builder.credentials(Credentials::new(
            cfg.username.clone(),
            cfg.password.clone().unwrap_or_default(),
        ));
    }
    Ok(builder.build())
}

fn mailbox(addr: &str) -> Result<Mailbox, MailError> {
    addr.parse()
        .map_err(|_| MailError::Address(addr.to_owned()))
}

/// Sends a test mail to `to`.
pub async fn send_test(cfg: &SmtpConfig, to: &str) -> Result<(), MailError> {
    let msg = Message::builder()
        .from(mailbox(&cfg.from)?)
        .to(mailbox(to)?)
        .subject("TalkOps: test mail")
        .header(ContentType::TEXT_PLAIN)
        .body("This is a test mail from your TalkOps phone system.\n".to_owned())
        .map_err(|e| MailError::Other(e.to_string()))?;
    transport(cfg)?
        .send(msg)
        .await
        .map_err(|e| MailError::Smtp(e.to_string()))?;
    Ok(())
}

/// Texts of the notification mail.
pub struct VoicemailText {
    pub subject: String,
    pub body: String,
}

/// Renders the notification in the box language (`de`/`en`).
pub fn voicemail_text(info: &MailInfo, caller: &str, at: &str, lang: &str) -> VoicemailText {
    let duration = format!("{}:{:02}", info.duration_secs / 60, info.duration_secs % 60);
    let caller_line = if info.caller_name.is_empty() {
        caller.to_owned()
    } else {
        format!("{} ({caller})", info.caller_name)
    };
    if lang == "en" {
        VoicemailText {
            subject: format!("New voicemail from {caller_line}"),
            body: format!(
                "Hello {name},\n\nthere is a new voicemail for extension {number}.\n\n\
                 From:     {caller_line}\nReceived: {at}\nLength:   {duration}\n\n\
                 Listen to it in the TalkOps web interface or by dialing *97.\n\n-- \nTalkOps\n",
                name = info.extension_name,
                number = info.extension_number,
            ),
        }
    } else {
        VoicemailText {
            subject: format!("Neue Sprachnachricht von {caller_line}"),
            body: format!(
                "Hallo {name},\n\nfür die Nebenstelle {number} ist eine neue Sprachnachricht eingegangen.\n\n\
                 Von:     {caller_line}\nZeit:    {at}\nLänge:   {duration}\n\n\
                 Abhören in der TalkOps-Weboberfläche oder per Anruf auf *97.\n\n-- \nTalkOps\n",
                name = info.extension_name,
                number = info.extension_number,
            ),
        }
    }
}

/// Formats a timestamp in the tenant's time zone.
pub fn local_time(at: DateTime<Utc>, zone: &str) -> String {
    let tz: Tz = zone.parse().unwrap_or(chrono_tz::Europe::Berlin);
    at.with_timezone(&tz).format("%d.%m.%Y %H:%M").to_string()
}

/// Builds the notification mail, with the recording attached if wanted.
pub fn build_voicemail_mail(
    from: &str,
    to: &str,
    text: VoicemailText,
    audio: Option<Vec<u8>>,
) -> Result<Message, MailError> {
    let builder = Message::builder()
        .from(mailbox(from)?)
        .to(mailbox(to)?)
        .subject(text.subject);
    let body = SinglePart::plain(text.body);
    let msg = match audio {
        Some(audio) => builder.multipart(
            MultiPart::mixed().singlepart(body).singlepart(
                Attachment::new("voicemail.wav".to_owned())
                    .body(audio, "audio/wav".parse().expect("valid content type")),
            ),
        ),
        None => builder.singlepart(body),
    };
    msg.map_err(|e| MailError::Other(e.to_string()))
}

/// Prepares the notification for a message; `None` if nothing is to be sent
/// (notification off, no address, message deleted).
pub async fn prepare(
    db: &PgPool,
    media: &MediaPaths,
    tenant: talkops_core::tenant::TenantId,
    message: uuid::Uuid,
    from: &str,
) -> Result<Option<Message>, MailError> {
    let Some(info) = voicemail::mail_info(db, message).await? else {
        return Ok(None);
    };
    let Some(to) = info.email.clone().filter(|e| !e.is_empty()) else {
        tracing::info!(extension = %info.extension_number, "voicemail mail skipped: user has no e-mail address");
        return Ok(None);
    };
    let s = settings::get(db, tenant).await?;
    let lang = info.language.clone().unwrap_or(s.default_language.clone());
    let caller = if info.caller_number.starts_with('+') {
        s.dial_plan().for_display(&info.caller_number)
    } else if info.caller_number.is_empty() {
        if lang == "en" { "unknown" } else { "unbekannt" }.to_owned()
    } else {
        info.caller_number.clone()
    };
    let text = voicemail_text(
        &info,
        &caller,
        &local_time(info.created_at, &s.timezone),
        &lang,
    );
    let path = media.voicemail.join(&info.file);
    let audio = match tokio::fs::metadata(&path).await {
        Ok(m) if info.attach_audio && m.len() <= MAX_ATTACHMENT => Some(
            tokio::fs::read(&path)
                .await
                .map_err(|e| MailError::Other(e.to_string()))?,
        ),
        _ => None,
    };
    build_voicemail_mail(from, &to, text, audio).map(Some)
}

async fn process(
    db: &PgPool,
    secrets: &SecretBox,
    media: &MediaPaths,
    job: &Job,
) -> Result<(), MailError> {
    let message = job.payload["message_id"]
        .as_str()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| MailError::Other("invalid payload".into()))?;
    let Some(cfg) = mail::config(db, job.tenant_id, secrets).await? else {
        tracing::warn!("voicemail mail skipped: SMTP is not configured");
        return Ok(());
    };
    if let Some(msg) = prepare(db, media, job.tenant_id, message, &cfg.from).await? {
        transport(&cfg)?
            .send(msg)
            .await
            .map_err(|e| MailError::Smtp(e.to_string()))?;
        tracing::info!(%message, "voicemail mail sent");
    }
    Ok(())
}

/// Runs mail jobs in the background (polling; mail is not time-critical).
pub fn spawn(db: PgPool, secrets: SecretBox, media: Arc<MediaPaths>) {
    let worker = format!("talkops-mail-{}", std::process::id());
    tokio::spawn(async move {
        loop {
            loop {
                let job = match jobs::claim(&db, &worker, &[voicemail::JOB_MAIL]).await {
                    Ok(Some(job)) => job,
                    Ok(None) => break,
                    Err(err) => {
                        tracing::debug!(error = %err, "mail job claim failed");
                        break;
                    }
                };
                let result = match process(&db, &secrets, &media, &job).await {
                    Ok(()) => jobs::complete(&db, job.id).await.map(|_| ()),
                    Err(err) => {
                        tracing::warn!(job = %job.id, error = %err, "voicemail mail failed");
                        jobs::fail(&db, job.id, &err.to_string()).await.map(|_| ())
                    }
                };
                if let Err(err) = result {
                    tracing::warn!(error = %err, "mail job bookkeeping failed");
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn info() -> MailInfo {
        MailInfo {
            message_id: uuid::Uuid::nil(),
            file: "x.wav".into(),
            caller_number: "+4930123456".into(),
            caller_name: "Anna".into(),
            duration_secs: 75,
            created_at: Utc.with_ymd_and_hms(2026, 7, 1, 10, 5, 0).unwrap(),
            extension_number: "20".into(),
            extension_name: "Office".into(),
            email: Some("anna@example.com".into()),
            attach_audio: true,
            language: None,
        }
    }

    #[test]
    fn renders_notification() {
        let at = local_time(info().created_at, "Europe/Berlin");
        assert_eq!(at, "01.07.2026 12:05");
        let text = voicemail_text(&info(), "030 123456", &at, "de");
        assert_eq!(text.subject, "Neue Sprachnachricht von Anna (030 123456)");
        assert!(text.body.contains("Länge:   1:15"));
        let text = voicemail_text(&info(), "030 123456", &at, "en");
        assert!(text.body.contains("extension 20"));

        let msg = build_voicemail_mail(
            "TalkOps <pbx@example.com>",
            "anna@example.com",
            text,
            Some(b"RIFF....".to_vec()),
        )
        .unwrap();
        let raw = String::from_utf8(msg.formatted()).unwrap();
        assert!(raw.contains("filename=\"voicemail.wav\""), "{raw}");
        assert!(raw.contains("To: anna@example.com"));
        assert!(
            build_voicemail_mail(
                "nobody",
                "a@b.c",
                voicemail_text(&info(), "x", "y", "de"),
                None
            )
            .is_err()
        );
    }
}
