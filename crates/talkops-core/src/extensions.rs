//! Extensions and their devices.
//!
//! An extension is a dialable internal number. Each device (desk phone, DECT
//! handset, softphone, …) has its own SIP credentials; calling the extension
//! rings all of its registered devices at once.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use crate::crypto::{self, SecretBox};
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Extension {
    pub id: Uuid,
    pub number: String,
    pub display_name: String,
    pub user_id: Option<Uuid>,
    pub outbound_number_id: Option<Uuid>,
    pub hide_caller_id: bool,
    pub ring_timeout_secs: i32,
    pub enabled: bool,
    /// Do not disturb: calls to the extension are rejected as busy.
    pub dnd: bool,
    /// Unconditional call forwarding target (extension or external number).
    pub forward_all: Option<String>,
    /// Call recording: `inherit` (tenant defaults), `always` or `never`.
    pub record_calls: String,
    /// Video calls (ADR 0025): internal calls with another extension with
    /// video, door stations, trunks with video. Otherwise audio only.
    pub video_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ExtensionInput {
    pub number: String,
    pub display_name: String,
    #[serde(default)]
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub outbound_number_id: Option<Uuid>,
    #[serde(default)]
    pub hide_caller_id: bool,
    #[serde(default = "default_ring_timeout")]
    pub ring_timeout_secs: i32,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub dnd: bool,
    #[serde(default)]
    pub forward_all: Option<String>,
    #[serde(default = "inherit")]
    pub record_calls: String,
    #[serde(default)]
    pub video_enabled: bool,
}

fn inherit() -> String {
    "inherit".into()
}

fn default_ring_timeout() -> i32 {
    30
}
fn yes() -> bool {
    true
}

const EXT_COLUMNS: &str = "id, number, display_name, user_id, outbound_number_id, hide_caller_id, ring_timeout_secs, enabled, dnd, forward_all, record_calls, video_enabled";

pub async fn list<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<Extension>> {
    let sql = format!("SELECT {EXT_COLUMNS} FROM extensions WHERE tenant_id = $1 ORDER BY number");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn list_for_user<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    user: Uuid,
) -> CoreResult<Vec<Extension>> {
    let sql = format!(
        "SELECT {EXT_COLUMNS} FROM extensions WHERE tenant_id = $1 AND user_id = $2 ORDER BY number"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(user)
        .fetch_all(db)
        .await?)
}

pub async fn get<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<Extension> {
    let sql = format!("SELECT {EXT_COLUMNS} FROM extensions WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

pub async fn find_by_number<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    number: &str,
) -> CoreResult<Option<Extension>> {
    let sql = format!("SELECT {EXT_COLUMNS} FROM extensions WHERE tenant_id = $1 AND number = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(number)
        .fetch_optional(db)
        .await?)
}

fn validate(input: &ExtensionInput, emergency: &[String]) -> CoreResult<()> {
    let n = &input.number;
    crate::numbering::validate_number(n, emergency)?;
    if !["inherit", "always", "never"].contains(&input.record_calls.as_str()) {
        return Err(CoreError::Validation("invalid recording setting".into()));
    }
    if input.display_name.trim().is_empty() {
        return Err(CoreError::Validation("display name is required".into()));
    }
    if let Some(f) = input.forward_all.as_deref().filter(|f| !f.is_empty()) {
        if !valid_forward_target(f) || f == n {
            return Err(CoreError::Validation(
                "forwarding target must be a number other than the extension itself".into(),
            ));
        }
    }
    Ok(())
}

/// Forwarding targets are extension or external numbers (digits, optional '+').
pub fn valid_forward_target(target: &str) -> bool {
    let digits = target.strip_prefix('+').unwrap_or(target);
    (2..=20).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
}

fn forward_value(input: &ExtensionInput) -> Option<String> {
    input
        .forward_all
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(str::to_owned)
}

pub async fn create(
    db: &sqlx::PgPool,
    tenant: TenantId,
    input: &ExtensionInput,
    emergency: &[String],
) -> CoreResult<Extension> {
    validate(input, emergency)?;
    crate::numbering::ensure_free(db, tenant, &input.number, None).await?;
    let sql = format!(
        "INSERT INTO extensions (tenant_id, number, display_name, user_id, outbound_number_id,
                                 hide_caller_id, ring_timeout_secs, enabled, dnd, forward_all,
                                 record_calls, video_enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING {EXT_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(&input.number)
        .bind(input.display_name.trim())
        .bind(input.user_id)
        .bind(input.outbound_number_id)
        .bind(input.hide_caller_id)
        .bind(input.ring_timeout_secs)
        .bind(input.enabled)
        .bind(input.dnd)
        .bind(forward_value(input))
        .bind(&input.record_calls)
        .bind(input.video_enabled)
        .fetch_one(db)
        .await?)
}

/// How long the extension rings before voicemail or the no-answer
/// forwarding takes over (5–300 s); changeable by the extension's user.
pub async fn set_ring_timeout(
    db: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    secs: i32,
) -> CoreResult<()> {
    if !(5..=300).contains(&secs) {
        return Err(CoreError::Validation(
            "ring time must be 5 to 300 seconds".into(),
        ));
    }
    let done = sqlx::query(
        "UPDATE extensions SET ring_timeout_secs = $3, updated_at = now()
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(secs)
    .execute(db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

pub async fn update(
    db: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &ExtensionInput,
    emergency: &[String],
) -> CoreResult<Extension> {
    validate(input, emergency)?;
    crate::numbering::ensure_free(db, tenant, &input.number, Some(id)).await?;
    let sql = format!(
        "UPDATE extensions SET number = $3, display_name = $4, user_id = $5, outbound_number_id = $6,
             hide_caller_id = $7, ring_timeout_secs = $8, enabled = $9, dnd = $10, forward_all = $11,
             record_calls = $12, video_enabled = $13, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {EXT_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(&input.number)
        .bind(input.display_name.trim())
        .bind(input.user_id)
        .bind(input.outbound_number_id)
        .bind(input.hide_caller_id)
        .bind(input.ring_timeout_secs)
        .bind(input.enabled)
        .bind(input.dnd)
        .bind(forward_value(input))
        .bind(&input.record_calls)
        .bind(input.video_enabled)
        .fetch_one(db)
        .await?)
}

/// Sets do-not-disturb (feature codes, phone DND key).
pub async fn set_dnd<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    dnd: bool,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE extensions SET dnd = $3, updated_at = now() WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(dnd)
    .execute(db)
    .await?;
    Ok(())
}

/// Sets or clears unconditional forwarding (feature codes).
pub async fn set_forward_all<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    target: Option<&str>,
) -> CoreResult<()> {
    if target.is_some_and(|t| !valid_forward_target(t)) {
        return Err(CoreError::Validation("invalid forwarding target".into()));
    }
    sqlx::query("UPDATE extensions SET forward_all = $3, updated_at = now() WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .bind(target)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM extensions WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

// --- devices -------------------------------------------------------------------

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "device_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Desk,
    Dect,
    Softphone,
    Mobile,
    Door,
    Other,
    /// The WebRTC softphone in the web interface.
    Browser,
    /// Wi-Fi handset (e.g. Yealink AX83H/AX86R).
    Wifi,
}

/// Audio codecs TalkOps uses with a device (ADR 0023). FreeSWITCH transcodes
/// between this codec and the codec of the other party.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "codec_profile", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum CodecProfile {
    /// The system codecs (Opus, G.722, G.711, video where allowed).
    Standard,
    /// G.722 wideband audio with G.711 as fallback, plus a jitter buffer.
    Hd,
    /// iLBC for networks with packet loss, G.711 as fallback, plus a jitter
    /// buffer.
    Robust,
}

impl CodecProfile {
    /// The profile in use: the configured one, else the default of the
    /// device kind (Wi-Fi handsets: `Hd`, all others: `Standard`).
    pub fn effective(kind: DeviceKind, configured: Option<CodecProfile>) -> CodecProfile {
        configured.unwrap_or(match kind {
            DeviceKind::Wifi => CodecProfile::Hd,
            _ => CodecProfile::Standard,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CodecProfile::Standard => "standard",
            CodecProfile::Hd => "hd",
            CodecProfile::Robust => "robust",
        }
    }

    pub fn parse(value: &str) -> Option<CodecProfile> {
        [
            CodecProfile::Standard,
            CodecProfile::Hd,
            CodecProfile::Robust,
        ]
        .into_iter()
        .find(|p| p.as_str() == value)
    }
}

/// Format of the SIP passwords TalkOps generates for a device. The format
/// applies when TalkOps creates or resets a password; it never changes a
/// stored password.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "sip_password_format", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SipPasswordFormat {
    /// 20 letters and digits.
    #[default]
    Standard,
    /// 15 letters and digits, for devices that accept at most 15 characters
    /// (e.g. Dahua VTO/VTH).
    Compact15,
}

impl SipPasswordFormat {
    /// Number of characters of a generated password.
    pub fn length(self) -> usize {
        match self {
            SipPasswordFormat::Standard => 20,
            SipPasswordFormat::Compact15 => 15,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SipPasswordFormat::Standard => "standard",
            SipPasswordFormat::Compact15 => "compact15",
        }
    }

    /// A new random password in this format.
    pub fn generate(self) -> CoreResult<String> {
        Ok(crypto::random_password(self.length())?)
    }
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Device {
    pub id: Uuid,
    pub extension_id: Uuid,
    /// Owner of a separate browser SIP identity; null for other devices.
    pub browser_user_id: Option<Uuid>,
    pub name: String,
    pub kind: DeviceKind,
    pub sip_username: String,
    #[serde(skip)]
    pub sip_password_enc: String,
    /// Provisioned phone the device is an account of.
    pub phone_id: Option<Uuid>,
    /// Account slot on the phone (DECT: handset number).
    pub account_index: Option<i16>,
    /// Label the provisioned phone shows for the account (line key, idle
    /// screen); empty = extension number.
    pub phone_label: String,
    /// Caller name the phone sends; empty = the extension's display name.
    pub phone_display_name: String,
    /// Configured codec profile; `None` = automatic from the device kind.
    pub codec_profile: Option<CodecProfile>,
    /// Format of the next generated SIP password.
    pub sip_password_format: SipPasswordFormat,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DeviceInput {
    pub name: String,
    pub kind: DeviceKind,
    /// SIP username: any free name of 3 to 64 letters, digits, `.`, `_` and
    /// `-`, independent of the extension number (`6578`, `8000vto`).
    /// Generated when empty: 6 random lower-case letters and digits
    /// (`28x1js`). On update, a different value renames the device; empty
    /// keeps the current name.
    #[serde(default)]
    pub sip_username: Option<String>,
    /// Place the device on a provisioned phone as account `account_index`
    /// (next free slot when omitted).
    #[serde(default)]
    pub phone_id: Option<Uuid>,
    #[serde(default)]
    pub account_index: Option<i16>,
    #[serde(default)]
    pub phone_label: String,
    #[serde(default)]
    pub phone_display_name: String,
    /// Codec profile; omitted or `null` = automatic from the device kind.
    #[serde(default)]
    pub codec_profile: Option<CodecProfile>,
    /// Format of generated SIP passwords. On create, omitted = `standard`.
    /// On update, omitted keeps the stored format. A changed format does not
    /// change the current password; it applies to the next reset.
    #[serde(default)]
    pub sip_password_format: Option<SipPasswordFormat>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

impl DeviceInput {
    /// Trimmed phone label and display name, checked against the column limits.
    fn phone_texts(&self) -> CoreResult<(&str, &str)> {
        phone_texts(&self.phone_label, &self.phone_display_name)
    }

    /// The requested SIP username, trimmed; `None` when empty.
    fn requested_username(&self) -> Option<&str> {
        self.sip_username
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
    }
}

/// Checks a chosen SIP username. The name does not depend on the extension
/// number: extension `8000` can use `6578`, `123` or `8000vto`. Some devices
/// (e.g. Dahua VTO/VTH) work best with digits only. The name must be unique
/// (see [`ensure_username_free`]). `web-` is reserved for browser devices.
pub fn validate_sip_username(username: &str) -> CoreResult<()> {
    let invalid = |msg: &str| Err(CoreError::Validation(format!("SIP username: {msg}")));
    if !(3..=64).contains(&username.len()) {
        return invalid("3 to 64 characters");
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return invalid("only letters, digits, '.', '_' and '-'");
    }
    if username.to_ascii_lowercase().starts_with("web-") {
        return invalid("'web-' is reserved for browser devices");
    }
    Ok(())
}

/// Fails with a conflict when another device (any tenant) already uses the
/// SIP username, without regard to case. The unique index stays the final
/// check for concurrent requests.
async fn ensure_username_free(
    pool: &sqlx::PgPool,
    username: &str,
    except: Option<Uuid>,
) -> CoreResult<()> {
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM devices WHERE lower(sip_username) = lower($1)
                        AND id IS DISTINCT FROM $2)",
    )
    .bind(username)
    .bind(except)
    .fetch_one(pool)
    .await?;
    if taken {
        return Err(CoreError::Conflict(format!(
            "SIP username {username} is already in use"
        )));
    }
    Ok(())
}

/// Length of a generated SIP username.
const GENERATED_USERNAME_LEN: usize = 6;

/// Generated SIP username: 6 random lower-case letters and digits with at
/// least one letter (`28x2js`), so that it never looks like an extension
/// number. It does not depend on the extension. Existing devices keep their
/// names.
async fn default_username(pool: &sqlx::PgPool) -> CoreResult<String> {
    // The large name space makes collisions rare; retry names already in use.
    for _ in 0..16 {
        let name = crypto::random_lowercase(GENERATED_USERNAME_LEN)?;
        if !name.chars().any(|c| c.is_ascii_lowercase()) {
            continue;
        }
        match ensure_username_free(pool, &name, None).await {
            Ok(()) => return Ok(name),
            Err(CoreError::Conflict(_)) => continue,
            Err(err) => return Err(err),
        }
    }
    Err(CoreError::Conflict(
        "no free SIP username found, try again".into(),
    ))
}

fn phone_texts<'a>(label: &'a str, display: &'a str) -> CoreResult<(&'a str, &'a str)> {
    let (label, display) = (label.trim(), display.trim());
    if label.chars().count() > 32 {
        return Err(CoreError::Validation("label: at most 32 characters".into()));
    }
    if display.chars().count() > 64 {
        return Err(CoreError::Validation(
            "display name: at most 64 characters".into(),
        ));
    }
    Ok((label, display))
}

/// Sets the label and display name a provisioned phone shows for one of its
/// accounts (empty = extension number / display name).
pub async fn set_phone_texts(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    phone_id: Uuid,
    device_id: Uuid,
    label: &str,
    display: &str,
) -> CoreResult<Device> {
    let (label, display) = phone_texts(label, display)?;
    let sql = format!(
        "UPDATE devices SET phone_label = $4, phone_display_name = $5, updated_at = now()
         WHERE tenant_id = $1 AND phone_id = $2 AND id = $3 RETURNING {DEV_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(phone_id)
        .bind(device_id)
        .bind(label)
        .bind(display)
        .fetch_one(pool)
        .await?)
}

const DEV_COLUMNS: &str = "id, extension_id, browser_user_id, name, kind, sip_username, sip_password_enc, phone_id, account_index, phone_label, phone_display_name, codec_profile, sip_password_format, enabled";

/// Resolves the account slot for a device on a phone: the requested one, or
/// the lowest free slot. Checks that the phone exists.
async fn phone_slot(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    phone_id: Option<Uuid>,
    requested: Option<i16>,
    device_id: Option<Uuid>,
) -> CoreResult<Option<i16>> {
    let Some(phone_id) = phone_id else {
        return Ok(None);
    };
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM phones WHERE tenant_id = $1 AND id = $2)")
            .bind(tenant)
            .bind(phone_id)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Err(CoreError::Validation("unknown phone".into()));
    }
    if let Some(slot) = requested {
        if !(1..=100).contains(&slot) {
            return Err(CoreError::Validation("account index must be 1-100".into()));
        }
        return Ok(Some(slot));
    }
    let used: Vec<i16> = sqlx::query_scalar(
        "SELECT account_index FROM devices
         WHERE phone_id = $1 AND account_index IS NOT NULL AND id IS DISTINCT FROM $2",
    )
    .bind(phone_id)
    .bind(device_id)
    .fetch_all(pool)
    .await?;
    Ok((1..=100).find(|i| !used.contains(i)))
}

pub async fn list_devices<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    extension: Uuid,
) -> CoreResult<Vec<Device>> {
    let sql = format!(
        "SELECT {DEV_COLUMNS} FROM devices WHERE tenant_id = $1 AND extension_id = $2 ORDER BY name"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(extension)
        .fetch_all(db)
        .await?)
}

pub async fn list_all_devices<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
) -> CoreResult<Vec<Device>> {
    let sql =
        format!("SELECT {DEV_COLUMNS} FROM devices WHERE tenant_id = $1 ORDER BY sip_username");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get_device<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<Device> {
    let sql = format!("SELECT {DEV_COLUMNS} FROM devices WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

/// Everything FreeSWITCH needs to authenticate a device and route its calls.
#[derive(Debug, Clone, FromRow)]
pub struct DeviceAuth {
    pub device_id: Uuid,
    pub tenant_id: TenantId,
    pub sip_username: String,
    pub sip_password_enc: String,
    pub extension_id: Uuid,
    pub extension_number: String,
    pub display_name: String,
    pub kind: DeviceKind,
    pub codec_profile: Option<CodecProfile>,
}

impl DeviceAuth {
    /// The codec profile in use for this device.
    pub fn effective_codec_profile(&self) -> CodecProfile {
        CodecProfile::effective(self.kind, self.codec_profile)
    }
}

/// Looks up an enabled device of an enabled extension by SIP username (any tenant).
pub async fn device_auth<'e>(
    db: impl PgExecutor<'e>,
    sip_username: &str,
) -> CoreResult<Option<DeviceAuth>> {
    Ok(sqlx::query_as(
        "SELECT d.id AS device_id, d.tenant_id, d.sip_username, d.sip_password_enc,
                e.id AS extension_id, e.number AS extension_number, e.display_name,
                d.kind, d.codec_profile
         FROM devices d JOIN extensions e ON e.id = d.extension_id
         WHERE lower(d.sip_username) = lower($1) AND d.enabled AND e.enabled
           AND (d.browser_user_id IS NULL OR EXISTS (
               SELECT 1 FROM users u WHERE u.id = d.browser_user_id
                 AND u.tenant_id = d.tenant_id AND u.enabled
                 AND (u.role = 'admin' OR e.user_id = u.id)))",
    )
    .bind(sip_username)
    .fetch_optional(db)
    .await?)
}

/// An enabled device of an extension that a call rings.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct RingTarget {
    pub sip_username: String,
    pub kind: DeviceKind,
    pub codec_profile: Option<CodecProfile>,
}

impl RingTarget {
    /// The codec profile in use for the device.
    pub fn effective_codec_profile(&self) -> CodecProfile {
        CodecProfile::effective(self.kind, self.codec_profile)
    }
}

/// The enabled devices of an extension (the ring targets).
pub async fn ring_targets<'e>(
    db: impl PgExecutor<'e>,
    extension: Uuid,
) -> CoreResult<Vec<RingTarget>> {
    Ok(sqlx::query_as(
        "SELECT d.sip_username, d.kind, d.codec_profile
         FROM devices d JOIN extensions e ON e.id = d.extension_id
         WHERE d.extension_id = $1 AND d.enabled
           AND (d.browser_user_id IS NULL OR EXISTS (
               SELECT 1 FROM users u WHERE u.id = d.browser_user_id
                 AND u.tenant_id = d.tenant_id AND u.enabled
                 AND (u.role = 'admin' OR e.user_id = u.id))) ORDER BY d.sip_username",
    )
    .bind(extension)
    .fetch_all(db)
    .await?)
}

/// Gets a user's browser device without sharing another user's SIP credentials.
/// Lock the extension so concurrent requests cannot create duplicate accounts.
pub async fn browser_device(
    pool: &sqlx::PgPool,
    secrets: &SecretBox,
    extension: &Extension,
    actor: &crate::audit::Actor,
    display_name: &str,
) -> CoreResult<(Device, String)> {
    let user = actor.user_id.ok_or(CoreError::NotFound)?;
    let mut tx = pool.begin().await?;
    // Recheck authorization under the lock before credentials are returned.
    let authorized: Option<Uuid> = sqlx::query_scalar(
        "SELECT e.id FROM extensions e JOIN users u ON u.id = $3 AND u.tenant_id = e.tenant_id
         WHERE e.id = $2 AND e.tenant_id = $1 AND e.enabled AND u.enabled
           AND (u.role = 'admin' OR e.user_id = u.id) FOR UPDATE OF e",
    )
    .bind(actor.tenant)
    .bind(extension.id)
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?;
    if authorized.is_none() {
        return Err(CoreError::NotFound);
    }
    let sql = format!(
        "SELECT {DEV_COLUMNS} FROM devices WHERE tenant_id = $1
                       AND extension_id = $2 AND browser_user_id = $3 AND kind = 'browser'"
    );
    let existing: Option<Device> = sqlx::query_as(&sql)
        .bind(actor.tenant)
        .bind(extension.id)
        .bind(user)
        .fetch_optional(&mut *tx)
        .await?;
    let (device, password, action) = if let Some(device) = existing {
        let password = secrets.decrypt(&device.sip_password_enc)?;
        (device, password, "view_credentials")
    } else {
        let password = SipPasswordFormat::Standard.generate()?;
        let sql = format!(
            "INSERT INTO devices
            (tenant_id, extension_id, browser_user_id, name, kind, sip_username, sip_password_enc)
            VALUES ($1, $2, $3, $4, 'browser', $5, $6) RETURNING {DEV_COLUMNS}"
        );
        let device: Device = sqlx::query_as(&sql)
            .bind(actor.tenant)
            .bind(extension.id)
            .bind(user)
            .bind(format!("Browser ({display_name})"))
            .bind(format!("web-{}", Uuid::new_v4()))
            .bind(secrets.encrypt(&password)?)
            .fetch_one(&mut *tx)
            .await?;
        (device, password, "create")
    };
    if device.enabled {
        crate::audit::record(
            &mut *tx,
            actor,
            action,
            "device",
            Some(device.id.to_string()),
            serde_json::json!({"extension": extension.number, "kind": "browser"}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok((device, password))
}

/// Creates a device with a generated SIP password. Returns the device and the
/// plaintext password (shown once to the admin).
pub async fn create_device(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    extension: &Extension,
    input: &DeviceInput,
) -> CoreResult<(Device, String)> {
    let (label, display) = input.phone_texts()?;
    let slot = phone_slot(pool, tenant, input.phone_id, input.account_index, None).await?;
    let username = match input.requested_username() {
        Some(u) => {
            validate_sip_username(u)?;
            ensure_username_free(pool, u, None).await?;
            u.to_owned()
        }
        None => default_username(pool).await?,
    };
    let format = input.sip_password_format.unwrap_or_default();
    check_password_format(input.kind, format)?;
    let password = format.generate()?;
    let sql = format!(
        "INSERT INTO devices (tenant_id, extension_id, name, kind, sip_username, sip_password_enc,
                              phone_id, account_index, enabled, phone_label, phone_display_name,
                              codec_profile, sip_password_format)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING {DEV_COLUMNS}"
    );
    let device = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(extension.id)
        .bind(input.name.trim())
        .bind(input.kind)
        .bind(&username)
        .bind(secrets.encrypt(&password)?)
        .bind(input.phone_id)
        .bind(slot)
        .bind(input.enabled)
        .bind(label)
        .bind(display)
        .bind(input.codec_profile)
        .bind(format)
        .fetch_one(pool)
        .await?;
    Ok((device, password))
}

/// Browser devices get their password from TalkOps only; they keep the
/// standard format.
fn check_password_format(kind: DeviceKind, format: SipPasswordFormat) -> CoreResult<()> {
    if kind == DeviceKind::Browser && format != SipPasswordFormat::Standard {
        return Err(CoreError::Validation(
            "SIP password format: browser devices use the standard format".into(),
        ));
    }
    Ok(())
}

pub async fn update_device(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &DeviceInput,
) -> CoreResult<Device> {
    let (label, display) = input.phone_texts()?;
    let current = get_device(pool, tenant, id).await?;
    let rename = input
        .requested_username()
        .filter(|u| *u != current.sip_username);
    if let Some(username) = rename {
        // Browser identities belong to a user and are generated (`web-…`).
        if current.kind == DeviceKind::Browser || current.browser_user_id.is_some() {
            return Err(CoreError::Validation(
                "SIP username: browser devices cannot be renamed".into(),
            ));
        }
        validate_sip_username(username)?;
        ensure_username_free(pool, username, Some(id)).await?;
    }
    if let Some(format) = input.sip_password_format {
        check_password_format(current.kind, format)?;
    }
    // A kind change must also keep the resulting device's format valid.
    check_password_format(
        input.kind,
        input
            .sip_password_format
            .unwrap_or(current.sip_password_format),
    )?;
    let slot = phone_slot(pool, tenant, input.phone_id, input.account_index, Some(id)).await?;
    // The format applies to the next generated password; the current
    // password stays valid.
    let sql = format!(
        "UPDATE devices SET name = $3, kind = $4, phone_id = $5, account_index = $6, enabled = $7,
             phone_label = $8, phone_display_name = $9, codec_profile = $10,
             sip_username = COALESCE($11, sip_username),
             sip_password_format = COALESCE($12, sip_password_format), updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {DEV_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(input.name.trim())
        .bind(input.kind)
        .bind(input.phone_id)
        .bind(slot)
        .bind(input.enabled)
        .bind(label)
        .bind(display)
        .bind(input.codec_profile)
        .bind(rename)
        .bind(input.sip_password_format)
        .fetch_one(pool)
        .await?)
}

/// Generates a new SIP password in the device's format; `format` changes the
/// stored format first. Returns the plaintext and the format used.
pub async fn reset_device_password(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
    format: Option<SipPasswordFormat>,
) -> CoreResult<(String, SipPasswordFormat)> {
    let mut tx = pool.begin().await?;
    let current: Option<(DeviceKind, SipPasswordFormat)> = sqlx::query_as(
        "SELECT kind, sip_password_format FROM devices WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (kind, stored) = current.ok_or(CoreError::NotFound)?;
    let format = format.unwrap_or(stored);
    check_password_format(kind, format)?;
    let password = format.generate()?;
    sqlx::query(
        "UPDATE devices SET sip_password_enc = $3, sip_password_format = $4, updated_at = now()
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(secrets.encrypt(&password)?)
    .bind(format)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((password, format))
}

pub async fn delete_device<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM devices WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_profile_defaults_by_kind() {
        use CodecProfile::*;
        assert_eq!(CodecProfile::effective(DeviceKind::Wifi, None), Hd);
        assert_eq!(CodecProfile::effective(DeviceKind::Desk, None), Standard);
        assert_eq!(
            CodecProfile::effective(DeviceKind::Wifi, Some(Robust)),
            Robust
        );
        assert_eq!(
            CodecProfile::effective(DeviceKind::Wifi, Some(Standard)),
            Standard
        );
        for p in [Standard, Hd, Robust] {
            assert_eq!(CodecProfile::parse(p.as_str()), Some(p));
        }
        assert_eq!(CodecProfile::parse("opus"), None);
    }

    #[tokio::test]
    async fn generated_username_keeps_database_errors() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/talkops")
            .unwrap();
        pool.close().await;
        assert!(matches!(
            default_username(&pool).await,
            Err(CoreError::Db(sqlx::Error::PoolClosed))
        ));
    }

    #[test]
    fn sip_username_validation() {
        for ok in [
            "8000",
            "6578",
            "123",
            "8000vto",
            "8001-1",
            "8000VTO.2",
            "20_a",
            "28x1js",
        ] {
            assert!(validate_sip_username(ok).is_ok(), "{ok}");
        }
        for bad in [
            "80",       // too short
            "8000 vto", // space
            "8000vtö",  // non-ASCII
            "web-8000", // reserved for browser devices
            "WEB-x1",   // reserved, any case
        ] {
            assert!(
                matches!(validate_sip_username(bad), Err(CoreError::Validation(_))),
                "{bad}"
            );
        }
        assert!(validate_sip_username(&"a".repeat(64)).is_ok());
        assert!(validate_sip_username(&"a".repeat(65)).is_err());
    }

    #[test]
    fn extension_validation() {
        let emergency = vec!["110".to_string(), "112".to_string()];
        let mk = |n: &str| ExtensionInput {
            number: n.into(),
            display_name: "A".into(),
            user_id: None,
            outbound_number_id: None,
            hide_caller_id: false,
            ring_timeout_secs: 30,
            enabled: true,
            dnd: false,
            forward_all: None,
            record_calls: "inherit".into(),
            video_enabled: false,
        };
        assert!(validate(&mk("20"), &emergency).is_ok());
        assert!(
            validate(
                &ExtensionInput {
                    forward_all: Some("21".into()),
                    ..mk("20")
                },
                &emergency
            )
            .is_ok()
        );
        assert!(
            validate(
                &ExtensionInput {
                    forward_all: Some("20".into()),
                    ..mk("20")
                },
                &emergency
            )
            .is_err()
        );
        assert!(
            validate(
                &ExtensionInput {
                    forward_all: Some("abc".into()),
                    ..mk("20")
                },
                &emergency
            )
            .is_err()
        );
        assert!(validate(&mk("2001"), &emergency).is_ok());
        assert!(validate(&mk("0123"), &emergency).is_err());
        assert!(validate(&mk("11"), &emergency).is_err());
        assert!(validate(&mk("1120"), &emergency).is_err());
        assert!(validate(&mk("2"), &emergency).is_err());
        assert!(validate(&mk("2a"), &emergency).is_err());
    }

    #[test]
    fn sip_password_formats() {
        // The alphabet of crypto::random_password: no 0, O, 1, l, I.
        let allowed = |c: char| c.is_ascii_alphanumeric() && !"0O1lI".contains(c);
        for (format, len) in [
            (SipPasswordFormat::Standard, 20),
            (SipPasswordFormat::Compact15, 15),
        ] {
            assert_eq!(format.length(), len);
            for _ in 0..50 {
                let p = format.generate().unwrap();
                assert_eq!(p.len(), len, "{format:?}");
                assert!(p.chars().all(allowed), "{p}");
            }
        }
        assert_eq!(SipPasswordFormat::default(), SipPasswordFormat::Standard);
        assert_eq!(
            serde_json::to_value(SipPasswordFormat::Compact15).unwrap(),
            "compact15"
        );
        assert_eq!(
            serde_json::from_value::<SipPasswordFormat>("compact15".into()).unwrap(),
            SipPasswordFormat::Compact15
        );
        assert!(serde_json::from_value::<SipPasswordFormat>("short".into()).is_err());
        assert_eq!(SipPasswordFormat::Compact15.as_str(), "compact15");
    }
}
