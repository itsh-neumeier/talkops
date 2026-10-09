//! SIP trunks, their accounts (registrations) and phone numbers.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::presets::{PresetCatalog, TrunkPreset};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Trunk {
    pub id: Uuid,
    pub name: String,
    pub preset: String,
    #[schema(value_type = Object)]
    pub overrides: serde_json::Value,
    pub enabled: bool,
    /// Offer video to the provider (only if it supports it).
    pub video_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct TrunkInput {
    pub name: String,
    pub preset: String,
    #[serde(default = "empty_object")]
    #[schema(value_type = Object)]
    pub overrides: serde_json::Value,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub video_enabled: bool,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
fn yes() -> bool {
    true
}

const TRUNK_COLUMNS: &str = "id, name, preset, overrides, enabled, video_enabled";

/// Validates the preset reference and overrides; returns the preset.
pub fn validate_trunk<'p>(
    catalog: &'p PresetCatalog,
    input: &TrunkInput,
) -> CoreResult<&'p TrunkPreset> {
    if input.name.trim().is_empty() {
        return Err(CoreError::Validation("name is required".into()));
    }
    let preset = catalog
        .get(&input.preset)
        .ok_or_else(|| CoreError::Validation(format!("unknown preset `{}`", input.preset)))?;
    let sip = preset
        .effective_sip(&input.overrides)
        .map_err(CoreError::Validation)?;
    if sip.registrar.as_deref().is_none_or(|r| r.trim().is_empty()) {
        return Err(CoreError::Validation(
            "this provider needs a registrar/SIP server (overrides.registrar)".into(),
        ));
    }
    Ok(preset)
}

pub async fn list<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<Trunk>> {
    let sql =
        format!("SELECT {TRUNK_COLUMNS} FROM trunks WHERE tenant_id = $1 ORDER BY lower(name)");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<Trunk> {
    let sql = format!("SELECT {TRUNK_COLUMNS} FROM trunks WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

pub async fn create<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    catalog: &PresetCatalog,
    input: &TrunkInput,
) -> CoreResult<Trunk> {
    validate_trunk(catalog, input)?;
    let sql = format!(
        "INSERT INTO trunks (tenant_id, name, preset, overrides, enabled, video_enabled)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING {TRUNK_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(input.name.trim())
        .bind(&input.preset)
        .bind(&input.overrides)
        .bind(input.enabled)
        .bind(input.video_enabled)
        .fetch_one(db)
        .await?)
}

pub async fn update<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    catalog: &PresetCatalog,
    id: Uuid,
    input: &TrunkInput,
) -> CoreResult<Trunk> {
    validate_trunk(catalog, input)?;
    let sql = format!(
        "UPDATE trunks SET name = $3, preset = $4, overrides = $5, enabled = $6,
             video_enabled = $7, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {TRUNK_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(input.name.trim())
        .bind(&input.preset)
        .bind(&input.overrides)
        .bind(input.enabled)
        .bind(input.video_enabled)
        .fetch_one(db)
        .await?)
}

pub async fn delete<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM trunks WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

// --- accounts ---------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct TrunkAccount {
    pub id: Uuid,
    pub trunk_id: Uuid,
    pub username: String,
    pub auth_username: String,
    #[serde(skip)]
    pub password_enc: String,
    pub enabled: bool,
    /// Inbound calls to this account that match none of its numbers
    /// (accounts without numbers, e.g. SIP addresses).
    pub destination_type: NumberDestination,
    pub destination_id: Option<Uuid>,
}

impl TrunkAccount {
    /// FreeSWITCH gateway name of this account.
    pub fn gateway_name(&self) -> String {
        gateway_name(self.id)
    }
}

/// Gateway names are derived from account ids so they never need escaping.
pub fn gateway_name(account_id: Uuid) -> String {
    format!("gw-{}", account_id.simple())
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct AccountInput {
    pub username: String,
    #[serde(default)]
    pub auth_username: String,
    /// Required on create; on update an empty/absent password keeps the old one.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Destination of calls to the account itself (see [TrunkAccount]).
    #[serde(default = "no_destination")]
    pub destination_type: NumberDestination,
    #[serde(default)]
    pub destination_id: Option<Uuid>,
}

fn no_destination() -> NumberDestination {
    NumberDestination::None
}

const ACC_COLUMNS: &str = "id, trunk_id, username, auth_username, password_enc, enabled, destination_type, destination_id";

pub async fn list_accounts<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    trunk: Uuid,
) -> CoreResult<Vec<TrunkAccount>> {
    let sql = format!(
        "SELECT {ACC_COLUMNS} FROM trunk_accounts WHERE tenant_id = $1 AND trunk_id = $2 ORDER BY username"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(trunk)
        .fetch_all(db)
        .await?)
}

pub async fn get_account<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<TrunkAccount> {
    let sql = format!("SELECT {ACC_COLUMNS} FROM trunk_accounts WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

pub async fn create_account<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    secrets: &SecretBox,
    trunk: Uuid,
    input: &AccountInput,
) -> CoreResult<TrunkAccount> {
    let password = input
        .password
        .as_deref()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| CoreError::Validation("password is required".into()))?;
    if input.username.trim().is_empty() {
        return Err(CoreError::Validation("username is required".into()));
    }
    let sql = format!(
        "INSERT INTO trunk_accounts (tenant_id, trunk_id, username, auth_username, password_enc, enabled,
                                     destination_type, destination_id)
         SELECT $1, t.id, $3, $4, $5, $6, $7, $8 FROM trunks t WHERE t.tenant_id = $1 AND t.id = $2
         RETURNING {ACC_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(trunk)
        .bind(input.username.trim())
        .bind(input.auth_username.trim())
        .bind(secrets.encrypt(password)?)
        .bind(input.enabled)
        .bind(input.destination_type)
        .bind(
            input
                .destination_id
                .filter(|_| input.destination_type != NumberDestination::None),
        )
        .fetch_one(db)
        .await?)
}

pub async fn update_account<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
    input: &AccountInput,
) -> CoreResult<TrunkAccount> {
    let password_enc = match input.password.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => Some(secrets.encrypt(p)?),
        None => None,
    };
    let sql = format!(
        "UPDATE trunk_accounts SET username = $3, auth_username = $4,
             password_enc = COALESCE($5, password_enc), enabled = $6,
             destination_type = $7, destination_id = $8, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {ACC_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(input.username.trim())
        .bind(input.auth_username.trim())
        .bind(password_enc)
        .bind(input.enabled)
        .bind(input.destination_type)
        .bind(
            input
                .destination_id
                .filter(|_| input.destination_type != NumberDestination::None),
        )
        .fetch_one(db)
        .await?)
}

pub async fn delete_account<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM trunk_accounts WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// An enabled account of an enabled trunk, with its tenant (inbound
/// routing knows only the account id from the gateway variables).
pub async fn active_account(
    pool: &PgPool,
    account: Uuid,
) -> CoreResult<Option<(TenantId, TrunkAccount)>> {
    let sql = format!(
        "SELECT a.tenant_id, {cols} FROM trunk_accounts a JOIN trunks t ON t.id = a.trunk_id
         WHERE a.id = $1 AND a.enabled AND t.enabled",
        cols = ACC_COLUMNS
            .split(", ")
            .map(|c| format!("a.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let Some(row) = sqlx::query(&sql).bind(account).fetch_optional(pool).await? else {
        return Ok(None);
    };
    let tenant: TenantId = sqlx::Row::try_get(&row, "tenant_id")?;
    Ok(Some((tenant, TrunkAccount::from_row(&row)?)))
}

/// Enabled accounts of enabled trunks across all tenants: the gateways
/// FreeSWITCH should have.
#[derive(Debug, Clone, FromRow)]
pub struct GatewayRow {
    pub account_id: Uuid,
    pub tenant_id: TenantId,
    pub trunk_id: Uuid,
    pub preset: String,
    pub overrides: serde_json::Value,
    pub username: String,
    pub auth_username: String,
    pub password_enc: String,
    /// First number of the account (for per-number registrations).
    pub first_number: Option<String>,
}

pub async fn active_gateways(pool: &PgPool) -> CoreResult<Vec<GatewayRow>> {
    Ok(sqlx::query_as(
        "SELECT a.id AS account_id, a.tenant_id, t.id AS trunk_id, t.preset, t.overrides,
                a.username, a.auth_username, a.password_enc,
                (SELECT n.e164 FROM numbers n WHERE n.account_id = a.id ORDER BY n.e164 LIMIT 1) AS first_number
         FROM trunk_accounts a JOIN trunks t ON t.id = a.trunk_id
         WHERE a.enabled AND t.enabled
         ORDER BY a.id",
    )
    .fetch_all(pool)
    .await?)
}

// --- numbers ------------------------------------------------------------------------

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "number_destination", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum NumberDestination {
    None,
    Extension,
    RingGroup,
    /// Straight to the voicemail of an extension.
    Voicemail,
    TimeCondition,
    Ivr,
    Queue,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct PhoneNumber {
    pub id: Uuid,
    pub trunk_id: Uuid,
    pub account_id: Option<Uuid>,
    pub e164: String,
    pub label: String,
    pub destination_type: NumberDestination,
    pub destination_id: Option<Uuid>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct NumberInput {
    pub trunk_id: Uuid,
    #[serde(default)]
    pub account_id: Option<Uuid>,
    /// E.164 with '+'.
    pub e164: String,
    #[serde(default)]
    pub label: String,
    pub destination_type: NumberDestination,
    #[serde(default)]
    pub destination_id: Option<Uuid>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

const NUM_COLUMNS: &str =
    "id, trunk_id, account_id, e164, label, destination_type, destination_id, enabled";

fn validate_number(input: &NumberInput) -> CoreResult<()> {
    let digits = input.e164.strip_prefix('+').unwrap_or("");
    if !(5..=15).contains(&digits.len())
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || digits.starts_with('0')
    {
        return Err(CoreError::Validation(
            "number must be in E.164 format, e.g. +49891234567".into(),
        ));
    }
    if input.destination_type != NumberDestination::None && input.destination_id.is_none() {
        return Err(CoreError::Validation("destination is required".into()));
    }
    Ok(())
}

pub async fn list_numbers<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
) -> CoreResult<Vec<PhoneNumber>> {
    let sql = format!("SELECT {NUM_COLUMNS} FROM numbers WHERE tenant_id = $1 ORDER BY e164");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get_number<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<PhoneNumber> {
    let sql = format!("SELECT {NUM_COLUMNS} FROM numbers WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

/// Finds an enabled number by E.164 (any tenant: inbound calls arrive before
/// the tenant is known).
pub async fn find_number(pool: &PgPool, e164: &str) -> CoreResult<Option<(TenantId, PhoneNumber)>> {
    let sql =
        format!("SELECT tenant_id, {NUM_COLUMNS} FROM numbers WHERE e164 = $1 AND enabled LIMIT 1");
    let row: Option<NumberWithTenant> =
        sqlx::query_as(&sql).bind(e164).fetch_optional(pool).await?;
    Ok(row.map(|r| (r.tenant_id, r.number)))
}

/// Numbers registered via a given account (per-number trunks).
pub async fn numbers_of_account(
    pool: &PgPool,
    account: Uuid,
) -> CoreResult<Vec<(TenantId, PhoneNumber)>> {
    let sql = format!(
        "SELECT tenant_id, {NUM_COLUMNS} FROM numbers WHERE account_id = $1 AND enabled ORDER BY e164"
    );
    let rows: Vec<NumberWithTenant> = sqlx::query_as(&sql).bind(account).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| (r.tenant_id, r.number)).collect())
}

#[derive(FromRow)]
struct NumberWithTenant {
    tenant_id: TenantId,
    #[sqlx(flatten)]
    number: PhoneNumber,
}

async fn check_refs(pool: &PgPool, tenant: TenantId, input: &NumberInput) -> CoreResult<()> {
    get(pool, tenant, input.trunk_id)
        .await
        .map_err(|_| CoreError::Validation("unknown trunk".into()))?;
    if let Some(acc) = input.account_id {
        let account = get_account(pool, tenant, acc)
            .await
            .map_err(|_| CoreError::Validation("unknown account".into()))?;
        if account.trunk_id != input.trunk_id {
            return Err(CoreError::Validation(
                "account belongs to another trunk".into(),
            ));
        }
    }
    crate::numbering::check_destination(pool, tenant, input.destination_type, input.destination_id)
        .await?;
    Ok(())
}

pub async fn create_number(
    pool: &PgPool,
    tenant: TenantId,
    input: &NumberInput,
) -> CoreResult<PhoneNumber> {
    validate_number(input)?;
    check_refs(pool, tenant, input).await?;
    let sql = format!(
        "INSERT INTO numbers (tenant_id, trunk_id, account_id, e164, label, destination_type, destination_id, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {NUM_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(input.trunk_id)
        .bind(input.account_id)
        .bind(&input.e164)
        .bind(input.label.trim())
        .bind(input.destination_type)
        .bind(
            input
                .destination_id
                .filter(|_| input.destination_type != NumberDestination::None),
        )
        .bind(input.enabled)
        .fetch_one(pool)
        .await?)
}

pub async fn update_number(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &NumberInput,
) -> CoreResult<PhoneNumber> {
    validate_number(input)?;
    check_refs(pool, tenant, input).await?;
    let sql = format!(
        "UPDATE numbers SET trunk_id = $3, account_id = $4, e164 = $5, label = $6, destination_type = $7,
             destination_id = $8, enabled = $9, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {NUM_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(input.trunk_id)
        .bind(input.account_id)
        .bind(&input.e164)
        .bind(input.label.trim())
        .bind(input.destination_type)
        .bind(
            input
                .destination_id
                .filter(|_| input.destination_type != NumberDestination::None),
        )
        .bind(input.enabled)
        .fetch_one(pool)
        .await?)
}

pub async fn delete_number<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM numbers WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// The trunk route for an outbound call from a given caller number.
#[derive(Debug, Clone, FromRow)]
pub struct OutboundRoute {
    pub number_id: Uuid,
    pub e164: String,
    pub trunk_id: Uuid,
    pub preset: String,
    pub overrides: serde_json::Value,
    pub account_id: Uuid,
    pub account_username: String,
    pub video_enabled: bool,
}

/// Resolves the outbound route for a caller number: its trunk and the account
/// to send through (the number's own account, else the trunk's first account).
pub async fn outbound_route(
    pool: &PgPool,
    tenant: TenantId,
    number_id: Uuid,
) -> CoreResult<Option<OutboundRoute>> {
    Ok(sqlx::query_as(
        "SELECT n.id AS number_id, n.e164, t.id AS trunk_id, t.preset, t.overrides,
                a.id AS account_id, a.username AS account_username, t.video_enabled
         FROM numbers n
         JOIN trunks t ON t.id = n.trunk_id AND t.enabled
         JOIN LATERAL (
             SELECT a.id, a.username FROM trunk_accounts a
             WHERE a.trunk_id = t.id AND a.enabled AND (n.account_id IS NULL OR a.id = n.account_id)
             ORDER BY (a.id = n.account_id) DESC NULLS LAST, a.created_at
             LIMIT 1
         ) a ON true
         WHERE n.tenant_id = $1 AND n.id = $2 AND n.enabled",
    )
    .bind(tenant)
    .bind(number_id)
    .fetch_optional(pool)
    .await?)
}
