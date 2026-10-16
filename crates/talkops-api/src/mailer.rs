//! Outgoing mail: voicemail notifications (job `mail.voicemail`) and the
//! SMTP test from the settings page. Runs in the TalkOps server because it
//! holds the key for the SMTP password. Mails are HTML + plain text in the
//! layout of ADR 0034.

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
use talkops_core::mail_render::{self, Design, MailKind, RenderedMail};
use talkops_core::mail_templates::{self, Logo};
use talkops_core::settings;
use talkops_core::tenant::TenantId;
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

/// Sender mailbox: the SMTP sender address with the branding display name
/// (if set).
pub fn sender(from: &str, display_name: &str) -> Result<Mailbox, MailError> {
    let mut mb = mailbox(from)?;
    if !display_name.trim().is_empty() {
        mb.name = Some(display_name.trim().to_owned());
    }
    Ok(mb)
}

/// A file attached to a mail.
pub struct MailAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

/// Builds a multipart mail: plain text and HTML (with the logo as inline
/// image, Content-ID [mail_render::LOGO_CID]), plus attachments.
pub fn build_message(
    from: Mailbox,
    to: &str,
    mail: RenderedMail,
    logo: Option<&Logo>,
    attachments: Vec<MailAttachment>,
) -> Result<Message, MailError> {
    let other = |e: lettre::error::Error| MailError::Other(e.to_string());
    let content_type = |ct: &str| {
        ContentType::parse(ct).map_err(|_| MailError::Other(format!("invalid content type {ct}")))
    };
    let html = SinglePart::html(mail.html);
    let alternative = MultiPart::alternative().singlepart(SinglePart::plain(mail.text));
    let alternative = match logo {
        Some(logo) => alternative.multipart(
            MultiPart::related().singlepart(html).singlepart(
                Attachment::new_inline(mail_render::LOGO_CID.to_owned())
                    .body(logo.bytes.clone(), content_type(&logo.content_type)?),
            ),
        ),
        None => alternative.singlepart(html),
    };
    let builder = Message::builder()
        .from(from)
        .to(mailbox(to)?)
        .subject(mail.subject);
    if attachments.is_empty() {
        return builder.multipart(alternative).map_err(other);
    }
    let mut mixed = MultiPart::mixed().multipart(alternative);
    for a in attachments {
        mixed = mixed
            .singlepart(Attachment::new(a.filename).body(a.data, content_type(&a.content_type)?));
    }
    builder.multipart(mixed).map_err(other)
}

/// Renders mail type `kind` for a tenant: own text or default in `lang`,
/// with the tenant's branding.
pub async fn render_for(
    db: &PgPool,
    tenant: TenantId,
    kind: &MailKind,
    lang: &str,
    values: &mail_render::Values,
) -> Result<(Design, Option<Logo>, RenderedMail), MailError> {
    let (design, logo) = mail_templates::design(db, tenant).await?;
    let text = mail_templates::template(db, tenant, kind, lang).await?;
    let mail = mail_render::render(&design, kind, lang, &text.subject, &text.body, values)
        .map_err(|e| MailError::Other(e.to_string()))?;
    Ok((design, logo, mail))
}

/// Sends a mail with the given SMTP settings.
pub async fn send(cfg: &SmtpConfig, msg: Message) -> Result<(), MailError> {
    transport(cfg)?
        .send(msg)
        .await
        .map_err(|e| MailError::Smtp(e.to_string()))?;
    Ok(())
}

/// Common placeholder values: recipient name and the current time.
pub async fn base_values(
    db: &PgPool,
    tenant: TenantId,
    recipient: &str,
) -> Result<mail_render::Values, MailError> {
    let s = settings::get(db, tenant).await?;
    let mut values = mail_render::Values::new();
    values.insert("recipient.name".into(), recipient.to_owned());
    values.insert("now".into(), local_time(Utc::now(), &s.timezone));
    Ok(values)
}

/// Sends the test mail (mail type `test`) to `to`.
pub async fn send_test(
    db: &PgPool,
    tenant: TenantId,
    cfg: &SmtpConfig,
    to: &str,
    lang: &str,
) -> Result<(), MailError> {
    let values = base_values(db, tenant, "").await?;
    let (design, logo, mail) = render_for(db, tenant, &mail_render::TEST, lang, &values).await?;
    let msg = build_message(
        sender(&cfg.from, &design.sender_name)?,
        to,
        mail,
        logo.as_ref(),
        Vec::new(),
    )?;
    send(cfg, msg).await
}

/// Placeholder values of a voicemail notification.
pub fn voicemail_values(info: &MailInfo, caller: &str, at: &str) -> mail_render::Values {
    let duration = format!("{}:{:02}", info.duration_secs / 60, info.duration_secs % 60);
    let display = if info.caller_name.is_empty() {
        caller.to_owned()
    } else {
        format!("{} ({caller})", info.caller_name)
    };
    let transcript = info
        .transcript
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_owned();
    [
        ("recipient.name", info.extension_name.clone()),
        ("extension.number", info.extension_number.clone()),
        ("extension.name", info.extension_name.clone()),
        ("caller.name", info.caller_name.clone()),
        ("caller.number", caller.to_owned()),
        ("caller.display", display),
        ("call.time", at.to_owned()),
        ("call.duration", duration),
        ("transcript", transcript),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

/// Formats a timestamp in the tenant's time zone.
pub fn local_time(at: DateTime<Utc>, zone: &str) -> String {
    let tz: Tz = zone.parse().unwrap_or(chrono_tz::Europe::Berlin);
    at.with_timezone(&tz).format("%d.%m.%Y %H:%M").to_string()
}

/// Prepares the notification for a message; `None` if nothing is to be sent
/// (notification off, no address, message deleted).
pub async fn prepare(
    db: &PgPool,
    media: &MediaPaths,
    tenant: TenantId,
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
    let mut values = voicemail_values(&info, &caller, &local_time(info.created_at, &s.timezone));
    values.insert("now".into(), local_time(Utc::now(), &s.timezone));
    let (design, logo, mail) =
        render_for(db, tenant, &mail_render::VOICEMAIL, &lang, &values).await?;
    let path = media.voicemail.join(&info.file);
    let mut attachments = Vec::new();
    if let Ok(m) = tokio::fs::metadata(&path).await
        && info.attach_audio
        && m.len() <= MAX_ATTACHMENT
    {
        attachments.push(MailAttachment {
            filename: "voicemail.wav".into(),
            content_type: "audio/wav".into(),
            data: tokio::fs::read(&path)
                .await
                .map_err(|e| MailError::Other(e.to_string()))?,
        });
    }
    build_message(
        sender(from, &design.sender_name)?,
        &to,
        mail,
        logo.as_ref(),
        attachments,
    )
    .map(Some)
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
        send(&cfg, msg).await?;
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
    use talkops_core::mail_render::{InstanceInfo, LogoSize};

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
            transcript: Some(" Bitte ruf zurück. ".into()),
        }
    }

    fn design() -> Design {
        Design {
            sender_name: "Phone system".into(),
            accent_color: "#0d9488".into(),
            logo: Some(LogoSize {
                width: 10,
                height: 10,
            }),
            logo_src: format!("cid:{}", mail_render::LOGO_CID),
            instance: InstanceInfo {
                name: "Home".into(),
                url: String::new(),
                hostname: "pbx".into(),
                version: "1.0.0".into(),
            },
            ..Design::default()
        }
    }

    #[test]
    fn renders_notification() {
        let at = local_time(info().created_at, "Europe/Berlin");
        assert_eq!(at, "01.07.2026 12:05");
        let values = voicemail_values(&info(), "030 123456", &at);
        let kind = &mail_render::VOICEMAIL;
        let mail = mail_render::render(
            &design(),
            kind,
            "de",
            kind.subject[0],
            kind.body[0],
            &values,
        )
        .unwrap();
        assert_eq!(
            mail.subject,
            "[Home] Neue Sprachnachricht von Anna (030 123456)"
        );
        assert!(mail.text.contains("- Länge: 1:15"), "{}", mail.text);
        assert!(
            mail.text
                .contains("Transkript (automatisch):\nBitte ruf zurück."),
            "{}",
            mail.text
        );

        let logo = Logo {
            bytes: b"GIF89a\x01\x00\x01\x00".to_vec(),
            content_type: "image/gif".into(),
            size: LogoSize {
                width: 1,
                height: 1,
            },
        };
        let msg = build_message(
            sender("pbx@example.com", "Phone system").unwrap(),
            "anna@example.com",
            mail.clone(),
            Some(&logo),
            vec![MailAttachment {
                filename: "voicemail.wav".into(),
                content_type: "audio/wav".into(),
                data: b"RIFF....".to_vec(),
            }],
        )
        .unwrap();
        let raw = String::from_utf8(msg.formatted()).unwrap();
        assert!(raw.contains("filename=\"voicemail.wav\""), "{raw}");
        assert!(raw.contains("To: anna@example.com"));
        assert!(
            raw.contains("From: \"Phone system\" <pbx@example.com>"),
            "{raw}"
        );
        assert!(raw.contains("multipart/mixed"));
        assert!(raw.contains("multipart/alternative"));
        assert!(raw.contains("multipart/related"));
        assert!(raw.contains("Content-ID: <logo@talkops>"), "{raw}");
        assert!(raw.contains("text/plain"));
        assert!(raw.contains("text/html"));

        let msg = build_message(
            sender("pbx@example.com", "").unwrap(),
            "a@b.c",
            mail,
            None,
            Vec::new(),
        )
        .unwrap();
        let raw = String::from_utf8(msg.formatted()).unwrap();
        assert!(!raw.contains("multipart/mixed"));
        assert!(!raw.contains("Content-ID"));
        assert!(sender("nobody", "").is_err());
    }
}
