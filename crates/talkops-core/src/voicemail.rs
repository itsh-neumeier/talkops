//! Voicemail boxes (one per extension), messages and greetings.
//!
//! Audio lives in the shared voicemail volume; the database stores paths
//! relative to it: `<tenant>/<extension>/<message>.wav` and
//! `<tenant>/<extension>/greeting.wav`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::crypto;
use crate::error::{CoreError, CoreResult};
use crate::jobs::{self, NewJob};
use crate::tenant::TenantId;

/// Job kind: render a TTS greeting (media worker).
pub const JOB_TTS_GREETING: &str = "tts.greeting";
/// Job kind: e-mail a new message to the box owner (TalkOps server).
pub const JOB_MAIL: &str = "mail.voicemail";

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct VoicemailBox {
    pub extension_id: Uuid,
    pub enabled: bool,
    #[serde(skip)]
    pub pin_hash: Option<String>,
    pub email_notify: bool,
    pub attach_audio: bool,
    /// `de`, `en` or `null` (tenant default).
    pub language: Option<String>,
    /// `default`, `tts` or `recorded`.
    pub greeting: String,
    pub greeting_text: String,
    /// `none`, `pending`, `ready` or `failed`.
    pub greeting_status: String,
    pub max_message_secs: i32,
}

impl VoicemailBox {
    fn disabled(extension_id: Uuid) -> Self {
        Self {
            extension_id,
            enabled: false,
            pin_hash: None,
            email_notify: false,
            attach_audio: true,
            language: None,
            greeting: "default".into(),
            greeting_text: String::new(),
            greeting_status: "none".into(),
            max_message_secs: 180,
        }
    }

    pub fn has_pin(&self) -> bool {
        self.pin_hash.is_some()
    }

    /// Checks a PIN entered on the phone.
    pub fn verify_pin(&self, pin: &str) -> bool {
        self.pin_hash
            .as_deref()
            .is_some_and(|hash| crypto::verify_password(pin, hash))
    }

    /// True when the custom greeting file should be played.
    pub fn uses_custom_greeting(&self) -> bool {
        self.greeting != "default" && self.greeting_status == "ready"
    }
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct VoicemailBoxInput {
    pub enabled: bool,
    /// New PIN (4–10 digits); `""` removes it, `null` keeps it.
    #[serde(default)]
    pub pin: Option<String>,
    #[serde(default)]
    pub email_notify: bool,
    #[serde(default = "yes")]
    pub attach_audio: bool,
    #[serde(default)]
    pub language: Option<String>,
    /// `default`, `tts` or `recorded` (the latter is set by recording on the phone).
    #[serde(default = "default_greeting")]
    pub greeting: String,
    #[serde(default)]
    pub greeting_text: String,
    #[serde(default = "default_max_secs")]
    pub max_message_secs: i32,
}

fn yes() -> bool {
    true
}
fn default_greeting() -> String {
    "default".into()
}
fn default_max_secs() -> i32 {
    180
}

const BOX_COLUMNS: &str = "extension_id, enabled, pin_hash, email_notify, attach_audio, language, \
                           greeting, greeting_text, greeting_status, max_message_secs";

/// Relative path of a box's custom greeting.
pub fn greeting_file(tenant: TenantId, extension: Uuid) -> String {
    format!("{tenant}/{extension}/greeting.wav")
}

/// Relative path of a message recording.
pub fn message_file(tenant: TenantId, extension: Uuid, message: Uuid) -> String {
    format!("{tenant}/{extension}/{message}.wav")
}

/// The box of an extension (a disabled default if none was configured).
pub async fn get_box<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    extension: Uuid,
) -> CoreResult<VoicemailBox> {
    let sql = format!(
        "SELECT {BOX_COLUMNS} FROM voicemail_boxes WHERE tenant_id = $1 AND extension_id = $2"
    );
    let found: Option<VoicemailBox> = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(extension)
        .fetch_optional(db)
        .await?;
    Ok(found.unwrap_or_else(|| VoicemailBox::disabled(extension)))
}

fn validate(input: &VoicemailBoxInput) -> CoreResult<()> {
    if let Some(pin) = input.pin.as_deref().filter(|p| !p.is_empty()) {
        if !(4..=10).contains(&pin.len()) || !pin.bytes().all(|b| b.is_ascii_digit()) {
            return Err(CoreError::Validation("PIN must have 4 to 10 digits".into()));
        }
    }
    if let Some(lang) = input.language.as_deref() {
        if !crate::prompts::LANGUAGES.contains(&lang) {
            return Err(CoreError::Validation(format!(
                "unsupported language `{lang}`"
            )));
        }
    }
    if !["default", "tts", "recorded"].contains(&input.greeting.as_str()) {
        return Err(CoreError::Validation("invalid greeting type".into()));
    }
    if input.greeting == "tts" && input.greeting_text.trim().is_empty() {
        return Err(CoreError::Validation("greeting text is required".into()));
    }
    if input.greeting_text.chars().count() > 1000 {
        return Err(CoreError::Validation("greeting text is too long".into()));
    }
    if !(10..=600).contains(&input.max_message_secs) {
        return Err(CoreError::Validation(
            "maximum message length must be 10 to 600 seconds".into(),
        ));
    }
    Ok(())
}

/// Saves the box settings. A new or changed TTS greeting is queued for
/// rendering by the media worker.
pub async fn update_box(
    pool: &PgPool,
    tenant: TenantId,
    extension: Uuid,
    input: &VoicemailBoxInput,
    default_language: &str,
) -> CoreResult<VoicemailBox> {
    validate(input)?;
    let mut tx = pool.begin().await?;
    let old = get_box(&mut *tx, tenant, extension).await?;
    let pin_hash = match input.pin.as_deref() {
        None => old.pin_hash.clone(),
        Some("") => None,
        Some(pin) => Some(crypto::hash_password(pin)?),
    };
    let text = input.greeting_text.trim();
    let language = input.language.clone();
    let rerender = input.greeting == "tts"
        && (old.greeting != "tts"
            || old.greeting_text != text
            || old.language != language
            || old.greeting_status == "failed");
    let status = match input.greeting.as_str() {
        "default" => "none",
        _ if rerender => "pending",
        // Switching back to a recording requires one to exist.
        "recorded" if old.greeting != "recorded" => {
            return Err(CoreError::Validation(
                "record a greeting on the phone (*97, option 5) first".into(),
            ));
        }
        _ => old.greeting_status.as_str(),
    };
    let sql = format!(
        "INSERT INTO voicemail_boxes (extension_id, tenant_id, enabled, pin_hash, email_notify,
                                      attach_audio, language, greeting, greeting_text,
                                      greeting_status, max_message_secs)
         SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
         WHERE EXISTS (SELECT 1 FROM extensions WHERE id = $1 AND tenant_id = $2)
         ON CONFLICT (extension_id) DO UPDATE SET
             enabled = $3, pin_hash = $4, email_notify = $5, attach_audio = $6, language = $7,
             greeting = $8, greeting_text = $9, greeting_status = $10, max_message_secs = $11,
             updated_at = now()
         RETURNING {BOX_COLUMNS}"
    );
    let saved: VoicemailBox = sqlx::query_as(&sql)
        .bind(extension)
        .bind(tenant)
        .bind(input.enabled)
        .bind(pin_hash)
        .bind(input.email_notify)
        .bind(input.attach_audio)
        .bind(&language)
        .bind(&input.greeting)
        .bind(text)
        .bind(status)
        .bind(input.max_message_secs)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?;
    if rerender {
        let lang = language.as_deref().unwrap_or(default_language);
        let mut job = NewJob::new(
            JOB_TTS_GREETING,
            serde_json::json!({
                "extension_id": extension,
                "text": text,
                "language": crate::prompts::language(lang),
                "file": greeting_file(tenant, extension),
            }),
        );
        job.tenant_id = tenant;
        job.priority = 10;
        jobs::enqueue(&mut *tx, job).await?;
    }
    tx.commit().await?;
    Ok(saved)
}

/// Records the result of rendering a TTS greeting (media worker). Ignored if
/// the text changed in the meantime.
pub async fn set_greeting_status(
    pool: &PgPool,
    extension: Uuid,
    text: &str,
    ready: bool,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE voicemail_boxes SET greeting_status = $3, updated_at = now()
         WHERE extension_id = $1 AND greeting = 'tts' AND greeting_text = $2",
    )
    .bind(extension)
    .bind(text)
    .bind(if ready { "ready" } else { "failed" })
    .execute(pool)
    .await?;
    Ok(())
}

/// A greeting was recorded on the phone and is now the active one.
pub async fn set_recorded_greeting(
    pool: &PgPool,
    tenant: TenantId,
    extension: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE voicemail_boxes SET greeting = 'recorded', greeting_status = 'ready', updated_at = now()
         WHERE tenant_id = $1 AND extension_id = $2",
    )
    .bind(tenant)
    .bind(extension)
    .execute(pool)
    .await?;
    Ok(())
}

// --- messages -------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Message {
    pub id: Uuid,
    pub extension_id: Uuid,
    pub caller_number: String,
    pub caller_name: String,
    pub duration_secs: i32,
    #[serde(skip)]
    pub file: String,
    /// `new` or `saved`.
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub heard_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub id: Uuid,
    pub extension_id: Uuid,
    pub caller_number: String,
    pub caller_name: String,
    pub duration_secs: i32,
    pub call_uuid: Option<String>,
}

const MSG_COLUMNS: &str = "id, extension_id, caller_number, caller_name, duration_secs, file, status, created_at, heard_at";

/// Stores a new message; queues the e-mail notification if enabled.
pub async fn create_message(
    pool: &PgPool,
    tenant: TenantId,
    msg: &NewMessage,
) -> CoreResult<Message> {
    let mut tx = pool.begin().await?;
    let sql = format!(
        "INSERT INTO voicemail_messages (id, tenant_id, extension_id, caller_number, caller_name,
                                         duration_secs, file, call_uuid)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {MSG_COLUMNS}"
    );
    let saved: Message = sqlx::query_as(&sql)
        .bind(msg.id)
        .bind(tenant)
        .bind(msg.extension_id)
        .bind(&msg.caller_number)
        .bind(&msg.caller_name)
        .bind(msg.duration_secs)
        .bind(message_file(tenant, msg.extension_id, msg.id))
        .bind(&msg.call_uuid)
        .fetch_one(&mut *tx)
        .await?;
    if get_box(&mut *tx, tenant, msg.extension_id)
        .await?
        .email_notify
    {
        let mut job = NewJob::new(JOB_MAIL, serde_json::json!({ "message_id": saved.id }));
        job.tenant_id = tenant;
        jobs::enqueue(&mut *tx, job).await?;
    }
    tx.commit().await?;
    Ok(saved)
}

/// Messages of a box: new ones first, oldest first within each group.
pub async fn list_messages<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    extension: Uuid,
) -> CoreResult<Vec<Message>> {
    let sql = format!(
        "SELECT {MSG_COLUMNS} FROM voicemail_messages WHERE tenant_id = $1 AND extension_id = $2
         ORDER BY status = 'new' DESC, created_at"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(extension)
        .fetch_all(db)
        .await?)
}

pub async fn get_message<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<Message> {
    let sql =
        format!("SELECT {MSG_COLUMNS} FROM voicemail_messages WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

/// Marks a message as heard (`saved`) or unheard (`new`).
pub async fn set_message_status<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    heard: bool,
) -> CoreResult<Message> {
    let sql = format!(
        "UPDATE voicemail_messages
         SET status = $3, heard_at = CASE WHEN $3 = 'saved' THEN COALESCE(heard_at, now()) END
         WHERE tenant_id = $1 AND id = $2 RETURNING {MSG_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(if heard { "saved" } else { "new" })
        .fetch_one(db)
        .await?)
}

/// Deletes a message and returns it (the caller removes the file).
pub async fn delete_message<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<Message> {
    let sql = format!(
        "DELETE FROM voicemail_messages WHERE tenant_id = $1 AND id = $2 RETURNING {MSG_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

/// (new, saved) message counts of a box, for MWI and announcements.
pub async fn counts<'e>(db: impl PgExecutor<'e>, extension: Uuid) -> CoreResult<(u32, u32)> {
    let (new, saved): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'new'), count(*) FILTER (WHERE status = 'saved')
         FROM voicemail_messages WHERE extension_id = $1",
    )
    .bind(extension)
    .fetch_one(db)
    .await?;
    Ok((new as u32, saved as u32))
}

/// Everything the notification mail needs about a message.
#[derive(Debug, Clone, FromRow)]
pub struct MailInfo {
    pub message_id: Uuid,
    pub file: String,
    pub caller_number: String,
    pub caller_name: String,
    pub duration_secs: i32,
    pub created_at: DateTime<Utc>,
    pub extension_number: String,
    pub extension_name: String,
    pub email: Option<String>,
    pub attach_audio: bool,
    pub language: Option<String>,
}

pub async fn mail_info(pool: &PgPool, message: Uuid) -> CoreResult<Option<MailInfo>> {
    Ok(sqlx::query_as(
        "SELECT m.id AS message_id, m.file, m.caller_number, m.caller_name, m.duration_secs,
                m.created_at, e.number AS extension_number, e.display_name AS extension_name,
                u.email, b.attach_audio, b.language
         FROM voicemail_messages m
         JOIN extensions e ON e.id = m.extension_id
         JOIN voicemail_boxes b ON b.extension_id = m.extension_id
         LEFT JOIN users u ON u.id = e.user_id
         WHERE m.id = $1 AND b.email_notify",
    )
    .bind(message)
    .fetch_optional(pool)
    .await?)
}

/// SIP usernames of an extension's devices (MWI targets).
pub async fn mwi_targets<'e>(db: impl PgExecutor<'e>, extension: Uuid) -> CoreResult<Vec<String>> {
    Ok(
        sqlx::query_scalar("SELECT sip_username FROM devices WHERE extension_id = $1")
            .bind(extension)
            .fetch_all(db)
            .await?,
    )
}
