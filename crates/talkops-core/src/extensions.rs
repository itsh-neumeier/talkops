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

const EXT_COLUMNS: &str = "id, number, display_name, user_id, outbound_number_id, hide_caller_id, ring_timeout_secs, enabled, dnd, forward_all, record_calls";

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
                                 record_calls)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING {EXT_COLUMNS}"
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
        .fetch_one(db)
        .await?)
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
             record_calls = $12, updated_at = now()
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
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Device {
    pub id: Uuid,
    pub extension_id: Uuid,
    pub name: String,
    pub kind: DeviceKind,
    pub sip_username: String,
    #[serde(skip)]
    pub sip_password_enc: String,
    /// Provisioned phone the device is an account of.
    pub phone_id: Option<Uuid>,
    /// Account slot on the phone (DECT: handset number).
    pub account_index: Option<i16>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DeviceInput {
    pub name: String,
    pub kind: DeviceKind,
    /// SIP username; generated (`<extension>-<n>`) when empty.
    #[serde(default)]
    pub sip_username: Option<String>,
    /// Place the device on a provisioned phone as account `account_index`
    /// (next free slot when omitted).
    #[serde(default)]
    pub phone_id: Option<Uuid>,
    #[serde(default)]
    pub account_index: Option<i16>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

const DEV_COLUMNS: &str = "id, extension_id, name, kind, sip_username, sip_password_enc, phone_id, account_index, enabled";

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
}

/// Looks up an enabled device of an enabled extension by SIP username (any tenant).
pub async fn device_auth<'e>(
    db: impl PgExecutor<'e>,
    sip_username: &str,
) -> CoreResult<Option<DeviceAuth>> {
    Ok(sqlx::query_as(
        "SELECT d.id AS device_id, d.tenant_id, d.sip_username, d.sip_password_enc,
                e.id AS extension_id, e.number AS extension_number, e.display_name
         FROM devices d JOIN extensions e ON e.id = d.extension_id
         WHERE lower(d.sip_username) = lower($1) AND d.enabled AND e.enabled",
    )
    .bind(sip_username)
    .fetch_optional(db)
    .await?)
}

/// SIP usernames of the enabled devices of an extension (the ring targets).
pub async fn ring_targets<'e>(db: impl PgExecutor<'e>, extension: Uuid) -> CoreResult<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT sip_username FROM devices WHERE extension_id = $1 AND enabled ORDER BY sip_username")
        .bind(extension)
        .fetch_all(db)
        .await?)
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
    let slot = phone_slot(pool, tenant, input.phone_id, input.account_index, None).await?;
    let username = match input
        .sip_username
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        Some(u) => u.to_owned(),
        None => {
            let existing: i64 =
                sqlx::query_scalar("SELECT count(*) FROM devices WHERE extension_id = $1")
                    .bind(extension.id)
                    .fetch_one(pool)
                    .await?;
            format!("{}-{}", extension.number, existing + 1)
        }
    };
    let password = crypto::random_password(20)?;
    let sql = format!(
        "INSERT INTO devices (tenant_id, extension_id, name, kind, sip_username, sip_password_enc,
                              phone_id, account_index, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {DEV_COLUMNS}"
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
        .fetch_one(pool)
        .await?;
    Ok((device, password))
}

pub async fn update_device(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &DeviceInput,
) -> CoreResult<Device> {
    let slot = phone_slot(pool, tenant, input.phone_id, input.account_index, Some(id)).await?;
    let sql = format!(
        "UPDATE devices SET name = $3, kind = $4, phone_id = $5, account_index = $6, enabled = $7,
             updated_at = now()
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
        .fetch_one(pool)
        .await?)
}

/// Generates a new SIP password; returns the plaintext.
pub async fn reset_device_password<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<String> {
    let password = crypto::random_password(20)?;
    let res = sqlx::query("UPDATE devices SET sip_password_enc = $3, updated_at = now() WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .bind(secrets.encrypt(&password)?)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(password)
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
}
