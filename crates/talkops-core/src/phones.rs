//! Provisioned phones (Yealink desk phones and DECT bases), firmware images
//! and shared phonebook contacts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::crypto::{self, SecretBox};
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Phone {
    pub id: Uuid,
    pub mac: String,
    pub model: String,
    pub name: String,
    /// Programmable keys, see `talkops_provisioning::yealink::LineKey`.
    #[schema(value_type = Vec<Object>)]
    pub line_keys: serde_json::Value,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_ip: Option<String>,
    pub last_firmware: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct PhoneInput {
    pub mac: String,
    pub model: String,
    pub name: String,
    #[serde(default = "empty_array")]
    #[schema(value_type = Vec<Object>)]
    pub line_keys: serde_json::Value,
}

fn empty_array() -> serde_json::Value {
    serde_json::json!([])
}

const PHONE_COLUMNS: &str = "id, mac, model, name, line_keys, last_seen_at, last_ip, last_firmware";

fn normalize_mac(mac: &str) -> CoreResult<String> {
    let hex: String = mac
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();
    let separators_ok = mac
        .trim()
        .chars()
        .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.'));
    if hex.len() != 12 || !separators_ok {
        return Err(CoreError::Validation(
            "MAC address must have 12 hex digits".into(),
        ));
    }
    Ok(hex)
}

fn validate(input: &PhoneInput) -> CoreResult<String> {
    if input.name.trim().is_empty() {
        return Err(CoreError::Validation("name is required".into()));
    }
    if !input.line_keys.is_array() {
        return Err(CoreError::Validation("line_keys must be an array".into()));
    }
    normalize_mac(&input.mac)
}

pub async fn list<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<Phone>> {
    let sql =
        format!("SELECT {PHONE_COLUMNS} FROM phones WHERE tenant_id = $1 ORDER BY lower(name)");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<Phone> {
    let sql = format!("SELECT {PHONE_COLUMNS} FROM phones WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

/// Looks up a phone by MAC across tenants (provisioning requests).
pub async fn find_by_mac(pool: &PgPool, mac: &str) -> CoreResult<Option<(TenantId, Phone)>> {
    #[derive(FromRow)]
    struct Row {
        tenant_id: TenantId,
        #[sqlx(flatten)]
        phone: Phone,
    }
    let sql = format!("SELECT tenant_id, {PHONE_COLUMNS} FROM phones WHERE mac = $1");
    let row: Option<Row> = sqlx::query_as(&sql).bind(mac).fetch_optional(pool).await?;
    Ok(row.map(|r| (r.tenant_id, r.phone)))
}

pub async fn create<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    input: &PhoneInput,
) -> CoreResult<Phone> {
    let mac = validate(input)?;
    let sql = format!(
        "INSERT INTO phones (tenant_id, mac, model, name, line_keys) VALUES ($1, $2, $3, $4, $5)
         RETURNING {PHONE_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(mac)
        .bind(input.model.trim())
        .bind(input.name.trim())
        .bind(&input.line_keys)
        .fetch_one(db)
        .await?)
}

pub async fn update<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    input: &PhoneInput,
) -> CoreResult<Phone> {
    let mac = validate(input)?;
    let sql = format!(
        "UPDATE phones SET mac = $3, model = $4, name = $5, line_keys = $6, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {PHONE_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(mac)
        .bind(input.model.trim())
        .bind(input.name.trim())
        .bind(&input.line_keys)
        .fetch_one(db)
        .await?)
}

pub async fn delete<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM phones WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Records that a phone contacted the provisioning server.
pub async fn record_seen(
    pool: &PgPool,
    id: Uuid,
    ip: Option<&str>,
    firmware: Option<&str>,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE phones SET last_seen_at = now(), last_ip = COALESCE($2, last_ip),
                           last_firmware = COALESCE($3, last_firmware)
         WHERE id = $1",
    )
    .bind(id)
    .bind(ip)
    .bind(firmware)
    .execute(pool)
    .await?;
    Ok(())
}

/// A SIP account placed on a phone, with everything needed for its config.
#[derive(Debug, Clone, FromRow)]
pub struct PhoneAccount {
    pub account_index: i16,
    pub device_id: Uuid,
    pub device_name: String,
    pub sip_username: String,
    pub sip_password_enc: String,
    pub extension_id: Uuid,
    pub extension_number: String,
    pub display_name: String,
}

/// Enabled devices of enabled extensions placed on the phone.
pub async fn accounts(pool: &PgPool, phone: Uuid) -> CoreResult<Vec<PhoneAccount>> {
    Ok(sqlx::query_as(
        "SELECT d.account_index, d.id AS device_id, d.name AS device_name, d.sip_username, d.sip_password_enc,
                e.id AS extension_id, e.number AS extension_number, e.display_name
         FROM devices d JOIN extensions e ON e.id = d.extension_id
         WHERE d.phone_id = $1 AND d.account_index IS NOT NULL AND d.enabled AND e.enabled
         ORDER BY d.account_index",
    )
    .bind(phone)
    .fetch_all(pool)
    .await?)
}

/// SIP usernames of all devices on the phone (resync via check-sync).
pub async fn device_usernames(pool: &PgPool, phone: Uuid) -> CoreResult<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT sip_username FROM devices WHERE phone_id = $1 ORDER BY account_index",
    )
    .bind(phone)
    .fetch_all(pool)
    .await?)
}

// --- provisioning credentials --------------------------------------------------

/// Credentials phones use to fetch configuration, and the phones' admin password.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProvisioningSecrets {
    pub username: String,
    pub password: String,
    pub phone_admin_password: String,
}

/// Returns the provisioning secrets, generating missing ones (or all of them
/// when `regenerate` is set).
pub async fn provisioning_secrets(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    regenerate: bool,
) -> CoreResult<ProvisioningSecrets> {
    let (username, password_enc, admin_enc): (String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT provisioning_username, provisioning_password_enc, phone_admin_password_enc
         FROM tenant_settings WHERE tenant_id = $1",
        )
        .bind(tenant)
        .fetch_one(pool)
        .await?;
    let decrypt = |enc: Option<String>| -> CoreResult<Option<String>> {
        enc.filter(|_| !regenerate)
            .map(|e| secrets.decrypt(&e))
            .transpose()
            .map_err(Into::into)
    };
    let (password, admin) = match (decrypt(password_enc)?, decrypt(admin_enc)?) {
        (Some(p), Some(a)) => (p, a),
        (p, a) => {
            let p = p.map_or_else(|| crypto::random_password(24), Ok)?;
            let a = a.map_or_else(|| crypto::random_password(16), Ok)?;
            sqlx::query(
                "UPDATE tenant_settings SET provisioning_password_enc = $2, phone_admin_password_enc = $3
                 WHERE tenant_id = $1",
            )
            .bind(tenant)
            .bind(secrets.encrypt(&p)?)
            .bind(secrets.encrypt(&a)?)
            .execute(pool)
            .await?;
            (p, a)
        }
    };
    Ok(ProvisioningSecrets {
        username,
        password,
        phone_admin_password: admin,
    })
}

// --- firmware ------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Firmware {
    pub id: Uuid,
    pub model: String,
    pub filename: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub active: bool,
    pub uploaded_at: DateTime<Utc>,
}

const FW_COLUMNS: &str = "id, model, filename, size_bytes, sha256, active, uploaded_at";

pub async fn list_firmware<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
) -> CoreResult<Vec<Firmware>> {
    let sql = format!(
        "SELECT {FW_COLUMNS} FROM firmware WHERE tenant_id = $1 ORDER BY model, uploaded_at DESC"
    );
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get_firmware<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<Firmware> {
    let sql = format!("SELECT {FW_COLUMNS} FROM firmware WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

pub async fn create_firmware<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    model: &str,
    filename: &str,
    size_bytes: i64,
    sha256: &str,
) -> CoreResult<Firmware> {
    let sql = format!(
        "INSERT INTO firmware (id, tenant_id, model, filename, size_bytes, sha256) VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING {FW_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(id)
        .bind(tenant)
        .bind(model)
        .bind(filename)
        .bind(size_bytes)
        .bind(sha256)
        .fetch_one(db)
        .await?)
}

/// Makes `id` the active image of its model (or deactivates it).
pub async fn set_firmware_active(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    active: bool,
) -> CoreResult<Firmware> {
    let mut tx = pool.begin().await?;
    let fw = get_firmware(&mut *tx, tenant, id).await?;
    if active {
        sqlx::query(
            "UPDATE firmware SET active = false WHERE tenant_id = $1 AND model = $2 AND active",
        )
        .bind(tenant)
        .bind(&fw.model)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE firmware SET active = $3 WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .bind(active)
        .execute(&mut *tx)
        .await?;
    let fw = get_firmware(&mut *tx, tenant, id).await?;
    tx.commit().await?;
    Ok(fw)
}

pub async fn active_firmware<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    model: &str,
) -> CoreResult<Option<Firmware>> {
    let sql =
        format!("SELECT {FW_COLUMNS} FROM firmware WHERE tenant_id = $1 AND model = $2 AND active");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(model)
        .fetch_optional(db)
        .await?)
}

pub async fn delete_firmware<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM firmware WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

// --- contacts -------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Contact {
    pub id: Uuid,
    pub name: String,
    pub company: String,
    pub phone_work: String,
    pub phone_mobile: String,
    pub phone_other: String,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ContactInput {
    pub name: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub phone_work: String,
    #[serde(default)]
    pub phone_mobile: String,
    #[serde(default)]
    pub phone_other: String,
}

const CONTACT_COLUMNS: &str = "id, name, company, phone_work, phone_mobile, phone_other";

fn clean_number(n: &str) -> CoreResult<String> {
    let cleaned: String = n
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '/' | '(' | ')'))
        .collect();
    let digits = cleaned.strip_prefix('+').unwrap_or(&cleaned);
    if !cleaned.is_empty() && (!digits.bytes().all(|b| b.is_ascii_digit()) || digits.len() > 20) {
        return Err(CoreError::Validation(format!("invalid phone number `{n}`")));
    }
    Ok(cleaned)
}

fn validate_contact(c: &ContactInput) -> CoreResult<[String; 3]> {
    if c.name.trim().is_empty() {
        return Err(CoreError::Validation("name is required".into()));
    }
    let numbers = [
        clean_number(&c.phone_work)?,
        clean_number(&c.phone_mobile)?,
        clean_number(&c.phone_other)?,
    ];
    if numbers.iter().all(String::is_empty) {
        return Err(CoreError::Validation(
            "at least one number is required".into(),
        ));
    }
    Ok(numbers)
}

pub async fn list_contacts<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
) -> CoreResult<Vec<Contact>> {
    let sql =
        format!("SELECT {CONTACT_COLUMNS} FROM contacts WHERE tenant_id = $1 ORDER BY lower(name)");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

/// Name of the phone book entry with this number (E.164), if any. Entries
/// are compared after normalizing with the tenant's dial plan.
pub async fn contact_name<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    e164: &str,
    dial_plan: &crate::dialing::DialPlanSettings,
) -> CoreResult<Option<String>> {
    let contacts = list_contacts(db, tenant).await?;
    Ok(contacts.into_iter().find_map(|c| {
        [&c.phone_work, &c.phone_mobile, &c.phone_other]
            .into_iter()
            .filter(|n| !n.is_empty())
            .any(|n| dial_plan.normalize_incoming(n).as_deref() == Some(e164))
            .then_some(c.name)
    }))
}

pub async fn create_contact<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    c: &ContactInput,
) -> CoreResult<Contact> {
    let [work, mobile, other] = validate_contact(c)?;
    let sql = format!(
        "INSERT INTO contacts (tenant_id, name, company, phone_work, phone_mobile, phone_other)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {CONTACT_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(c.name.trim())
        .bind(c.company.trim())
        .bind(work)
        .bind(mobile)
        .bind(other)
        .fetch_one(db)
        .await?)
}

pub async fn update_contact<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    c: &ContactInput,
) -> CoreResult<Contact> {
    let [work, mobile, other] = validate_contact(c)?;
    let sql = format!(
        "UPDATE contacts SET name = $3, company = $4, phone_work = $5, phone_mobile = $6, phone_other = $7,
             updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {CONTACT_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(c.name.trim())
        .bind(c.company.trim())
        .bind(work)
        .bind(mobile)
        .bind(other)
        .fetch_one(db)
        .await?)
}

pub async fn delete_contact<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM contacts WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}
