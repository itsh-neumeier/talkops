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
}

fn default_ring_timeout() -> i32 {
    30
}
fn yes() -> bool {
    true
}

const EXT_COLUMNS: &str = "id, number, display_name, user_id, outbound_number_id, hide_caller_id, ring_timeout_secs, enabled";

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
    if !(2..=8).contains(&n.len()) || !n.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CoreError::Validation(
            "extension number must have 2-8 digits".into(),
        ));
    }
    // Extensions must not shadow dialing prefixes or emergency numbers.
    if n.starts_with('0') {
        return Err(CoreError::Validation(
            "extension numbers must not start with 0 (trunk prefix)".into(),
        ));
    }
    if emergency
        .iter()
        .any(|e| n.starts_with(e.as_str()) || e.starts_with(n.as_str()))
        || n.starts_with("11")
    {
        return Err(CoreError::Validation(
            "extension number collides with emergency or service numbers".into(),
        ));
    }
    if input.display_name.trim().is_empty() {
        return Err(CoreError::Validation("display name is required".into()));
    }
    Ok(())
}

pub async fn create<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    input: &ExtensionInput,
    emergency: &[String],
) -> CoreResult<Extension> {
    validate(input, emergency)?;
    let sql = format!(
        "INSERT INTO extensions (tenant_id, number, display_name, user_id, outbound_number_id,
                                 hide_caller_id, ring_timeout_secs, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {EXT_COLUMNS}"
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
        .fetch_one(db)
        .await?)
}

pub async fn update<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    input: &ExtensionInput,
    emergency: &[String],
) -> CoreResult<Extension> {
    validate(input, emergency)?;
    let sql = format!(
        "UPDATE extensions SET number = $3, display_name = $4, user_id = $5, outbound_number_id = $6,
             hide_caller_id = $7, ring_timeout_secs = $8, enabled = $9, updated_at = now()
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
        .fetch_one(db)
        .await?)
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
    pub mac: Option<String>,
    pub model: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DeviceInput {
    pub name: String,
    pub kind: DeviceKind,
    /// SIP username; generated (`<extension>-<n>`) when empty.
    #[serde(default)]
    pub sip_username: Option<String>,
    #[serde(default)]
    pub mac: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

const DEV_COLUMNS: &str =
    "id, extension_id, name, kind, sip_username, sip_password_enc, mac, model, enabled";

fn normalize_mac(mac: Option<&str>) -> CoreResult<Option<String>> {
    let Some(mac) = mac.map(str::trim).filter(|m| !m.is_empty()) else {
        return Ok(None);
    };
    let hex: String = mac
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();
    let separators_ok = mac
        .chars()
        .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.'));
    if hex.len() != 12 || !separators_ok {
        return Err(CoreError::Validation(
            "MAC address must have 12 hex digits".into(),
        ));
    }
    Ok(Some(hex))
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
    let mac = normalize_mac(input.mac.as_deref())?;
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
        "INSERT INTO devices (tenant_id, extension_id, name, kind, sip_username, sip_password_enc, mac, model, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NULLIF($8, ''), $9) RETURNING {DEV_COLUMNS}"
    );
    let device = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(extension.id)
        .bind(input.name.trim())
        .bind(input.kind)
        .bind(&username)
        .bind(secrets.encrypt(&password)?)
        .bind(mac)
        .bind(input.model.as_deref().map(str::trim))
        .bind(input.enabled)
        .fetch_one(pool)
        .await?;
    Ok((device, password))
}

pub async fn update_device<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    input: &DeviceInput,
) -> CoreResult<Device> {
    let mac = normalize_mac(input.mac.as_deref())?;
    let sql = format!(
        "UPDATE devices SET name = $3, kind = $4, mac = $5, model = NULLIF($6, ''), enabled = $7, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {DEV_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(input.name.trim())
        .bind(input.kind)
        .bind(mac)
        .bind(input.model.as_deref().map(str::trim))
        .bind(input.enabled)
        .fetch_one(db)
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
    fn mac_normalization() {
        assert_eq!(
            normalize_mac(Some("80:5E:C0:12:34:56")).unwrap().as_deref(),
            Some("805ec0123456")
        );
        assert_eq!(
            normalize_mac(Some("805e.c012.3456")).unwrap().as_deref(),
            Some("805ec0123456")
        );
        assert_eq!(normalize_mac(Some("")).unwrap(), None);
        assert!(normalize_mac(Some("80:5E:C0")).is_err());
        assert!(normalize_mac(Some("80:5E:C0:12:34:5Z")).is_err());
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
        };
        assert!(validate(&mk("20"), &emergency).is_ok());
        assert!(validate(&mk("2001"), &emergency).is_ok());
        assert!(validate(&mk("0123"), &emergency).is_err());
        assert!(validate(&mk("11"), &emergency).is_err());
        assert!(validate(&mk("1120"), &emergency).is_err());
        assert!(validate(&mk("2"), &emergency).is_err());
        assert!(validate(&mk("2a"), &emergency).is_err());
    }
}
