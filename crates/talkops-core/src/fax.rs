//! Durable fax documents, access policy and mail admission.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{
    crypto::SecretBox,
    error::{CoreError, CoreResult},
    roles::{self, Permission},
    tenant::TenantId,
    trunks,
    users::Role,
};

pub const PREPARE: &str = "fax.prepare";
pub const RECEIVE: &str = "fax.receive";
pub const MAX_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_FILES: usize = 10;
pub const MAX_PAGES: i32 = 50;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub global_number_id: Option<Uuid>,
    pub always_global: bool,
    pub header: String,
    pub footer: String,
    pub retention_days: i32,
    pub imap_enabled: bool,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_username: String,
    pub imap_folder: String,
    pub faxin_address: String,
    /// The server accepts only authenticated, sender-bound messages for this mailbox.
    pub trusted_mailbox: bool,
    /// Trunk UUID -> audio, preferred, required.
    pub trunk_modes: std::collections::BTreeMap<Uuid, String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            global_number_id: None,
            always_global: false,
            header: "{sender} | {date} | {page}/{pages}".into(),
            footer: String::new(),
            retention_days: 30,
            imap_enabled: false,
            imap_host: String::new(),
            imap_port: 993,
            imap_username: String::new(),
            imap_folder: "INBOX".into(),
            faxin_address: String::new(),
            trusted_mailbox: false,
            trunk_modes: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Template {
    pub header: String,
    pub footer: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct Endpoint {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
    pub send_users: Vec<Uuid>,
    pub receive_users: Vec<Uuid>,
    pub reception_email: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct Whitelist {
    pub email: String,
    pub user_id: Uuid,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InputFile {
    pub file: String,
    pub name: String,
}
#[derive(Debug, Clone)]
pub struct Receipt {
    pub key: String,
    pub fingerprint: String,
}
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Fax {
    pub id: Uuid,
    #[schema(value_type = Uuid)]
    pub tenant_id: TenantId,
    pub owner_id: Uuid,
    pub endpoint_id: Option<Uuid>,
    pub direction: String,
    pub state: String,
    pub number_id: Option<Uuid>,
    pub trunk_id: Option<Uuid>,
    pub source: String,
    pub destination: String,
    pub sender_email: String,
    pub header: String,
    pub footer: String,
    #[schema(value_type = Object)]
    pub inputs: serde_json::Value,
    pub pages: i32,
    pub result: String,
    pub attempts: i32,
    pub call_uuid: Option<Uuid>,
    pub next_attempt_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
pub fn invalid(message: &str) -> CoreError {
    CoreError::Validation(message.into())
}
pub fn number(value: &str) -> bool {
    let bytes = value.as_bytes();
    (3..=16).contains(&bytes.len())
        && bytes[0] == b'+'
        && (b'1'..=b'9').contains(&bytes[1])
        && bytes[2..].iter().all(u8::is_ascii_digit)
}
/// Full-address matching, case-insensitive ASCII mailbox identities. No patterns or display names.
pub fn email(value: &str) -> CoreResult<String> {
    let value = value.trim();
    let Some((local, domain)) = value.split_once('@') else {
        return Err(invalid("invalid sender email"));
    };
    if value.len() > 254
        || local.is_empty()
        || domain.is_empty()
        || !value.is_ascii()
        || !local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        || !domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
        || local.contains('*')
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || domain.contains("..")
    {
        return Err(invalid("invalid sender email"));
    }
    Ok(value.to_ascii_lowercase())
}
pub fn template(text: &str) -> CoreResult<()> {
    if text.chars().count() > 180 || text.chars().any(char::is_control) {
        return Err(invalid("fax text must fit one line (180 characters)"));
    }
    let mut stripped = text.to_owned();
    for key in ["sender", "recipient", "date", "page", "pages"] {
        stripped = stripped.replace(&format!("{{{key}}}"), "");
    }
    if stripped.contains(['{', '}']) {
        return Err(invalid("unknown fax placeholder"));
    }
    Ok(())
}
pub async fn settings(db: &PgPool, tenant: TenantId) -> CoreResult<Settings> {
    let value: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT config FROM fax_settings WHERE tenant_id=$1")
            .bind(tenant)
            .fetch_optional(db)
            .await?;
    serde_json::from_value(value.unwrap_or_else(|| serde_json::json!({})))
        .map_err(|_| invalid("invalid fax settings"))
}
pub async fn password(
    db: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
) -> CoreResult<Option<String>> {
    let enc: Option<String> =
        sqlx::query_scalar("SELECT imap_password_enc FROM fax_settings WHERE tenant_id=$1")
            .bind(tenant)
            .fetch_optional(db)
            .await?
            .flatten();
    enc.map(|s| secrets.decrypt(&s).map_err(CoreError::from))
        .transpose()
}
pub async fn save_settings(
    db: &PgPool,
    tenant: TenantId,
    cfg: &Settings,
    password: Option<&str>,
    secrets: &SecretBox,
) -> CoreResult<()> {
    template(&cfg.header)?;
    template(&cfg.footer)?;
    if !(1..=3650).contains(&cfg.retention_days)
        || cfg.imap_port == 0
        || cfg.imap_host.contains(['\r', '\n', '/', '\\'])
        || cfg.imap_host.len() > 253
        || cfg.imap_folder.len() > 100
        || cfg.imap_folder.chars().any(char::is_control)
    {
        return Err(invalid("invalid fax settings"));
    }
    if cfg.imap_enabled
        && (!cfg.enabled
            || !cfg.trusted_mailbox
            || cfg.imap_host.is_empty()
            || cfg.imap_username.is_empty())
    {
        return Err(invalid(
            "mail input needs fax, a trusted mailbox and IMAPS credentials",
        ));
    }
    if cfg.imap_enabled {
        email(&cfg.faxin_address)?;
        let has_password = match password {
            Some(p) => !p.is_empty(),
            None => self::password(db, tenant, secrets).await?.is_some(),
        };
        if !has_password || crate::mail::config(db, tenant, secrets).await?.is_none() {
            return Err(invalid(
                "mail-to-fax needs an IMAPS password and outgoing SMTP for rejection notices",
            ));
        }
    }
    if cfg
        .trunk_modes
        .values()
        .any(|v| !["audio", "preferred", "required"].contains(&v.as_str()))
    {
        return Err(invalid("invalid fax transport mode"));
    }
    if let Some(id) = cfg.global_number_id {
        trunks::get_number(db, tenant, id).await?;
    }
    let enc = password
        .filter(|p| !p.is_empty())
        .map(|p| secrets.encrypt(p))
        .transpose()?;
    sqlx::query("INSERT INTO fax_settings(tenant_id,config,imap_password_enc) VALUES($1,$2,$3) ON CONFLICT(tenant_id) DO UPDATE SET config=$2, imap_password_enc=CASE WHEN $4 THEN $3 ELSE fax_settings.imap_password_enc END")
        .bind(tenant).bind(serde_json::to_value(cfg).map_err(|_| invalid("invalid settings"))?).bind(enc).bind(password.is_some()).execute(db).await?;
    Ok(())
}
pub async fn endpoints(db: &PgPool, tenant: TenantId) -> CoreResult<Vec<Endpoint>> {
    Ok(sqlx::query_as("SELECT id,name,owner_id,send_users,receive_users,reception_email,enabled FROM fax_endpoints WHERE tenant_id=$1 ORDER BY name").bind(tenant).fetch_all(db).await?)
}
pub async fn save_endpoint(db: &PgPool, tenant: TenantId, e: &Endpoint) -> CoreResult<()> {
    if e.name.trim().is_empty()
        || e.name.len() > 100
        || e.send_users.len() + e.receive_users.len() > 100
    {
        return Err(invalid("invalid fax endpoint"));
    }
    if !e.reception_email.is_empty() {
        email(&e.reception_email)?;
    }
    for user in e
        .send_users
        .iter()
        .chain(e.receive_users.iter())
        .chain(std::iter::once(&e.owner_id))
    {
        crate::users::get(db, tenant, *user).await?;
    }
    sqlx::query("INSERT INTO fax_endpoints(id,tenant_id,name,owner_id,send_users,receive_users,reception_email,enabled) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(id) DO UPDATE SET name=$3,owner_id=$4,send_users=$5,receive_users=$6,reception_email=$7,enabled=$8 WHERE fax_endpoints.tenant_id=$2")
        .bind(e.id).bind(tenant).bind(e.name.trim()).bind(e.owner_id).bind(&e.send_users).bind(&e.receive_users).bind(&e.reception_email).bind(e.enabled).execute(db).await?;
    Ok(())
}
pub async fn whitelist(db: &PgPool, tenant: TenantId) -> CoreResult<Vec<Whitelist>> {
    Ok(sqlx::query_as(
        "SELECT email,user_id,enabled FROM fax_whitelist WHERE tenant_id=$1 ORDER BY email",
    )
    .bind(tenant)
    .fetch_all(db)
    .await?)
}
pub async fn save_whitelist(
    db: &PgPool,
    tenant: TenantId,
    entries: &[Whitelist],
) -> CoreResult<()> {
    if entries.len() > 500 {
        return Err(invalid("too many whitelist entries"));
    }
    for e in entries {
        email(&e.email)?;
        crate::users::get(db, tenant, e.user_id).await?;
    }
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM fax_whitelist WHERE tenant_id=$1")
        .bind(tenant)
        .execute(&mut *tx)
        .await?;
    for e in entries {
        sqlx::query(
            "INSERT INTO fax_whitelist(tenant_id,email,user_id,enabled) VALUES($1,$2,$3,$4)",
        )
        .bind(tenant)
        .bind(email(&e.email)?)
        .bind(e.user_id)
        .bind(e.enabled)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
pub async fn user_can(
    db: &PgPool,
    tenant: TenantId,
    user: Uuid,
    perm: Permission,
) -> CoreResult<bool> {
    let u = crate::users::get(db, tenant, user).await?;
    Ok(u.enabled
        && (u.role == Role::Admin
            || roles::of_user(db, tenant, u.role, u.role_id)
                .await?
                .has(perm)))
}
pub async fn mail_user(db: &PgPool, tenant: TenantId, address: &str) -> CoreResult<Uuid> {
    let address = email(address)?;
    let user: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM fax_whitelist WHERE tenant_id=$1 AND email=$2 AND enabled",
    )
    .bind(tenant)
    .bind(address)
    .fetch_optional(db)
    .await?;
    let user = user.ok_or_else(|| invalid("Sender is not permitted to submit faxes."))?;
    if !user_can(db, tenant, user, Permission::FaxSend).await?
        || !user_can(db, tenant, user, Permission::FaxMailSubmit).await?
    {
        return Err(invalid("Sender is not permitted to submit faxes."));
    }
    Ok(user)
}
pub async fn get(db: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Fax> {
    Ok(
        sqlx::query_as("SELECT * FROM faxes WHERE tenant_id=$1 AND id=$2")
            .bind(tenant)
            .bind(id)
            .fetch_one(db)
            .await?,
    )
}
pub async fn list(db: &PgPool, tenant: TenantId, user: Uuid, all: bool) -> CoreResult<Vec<Fax>> {
    Ok(sqlx::query_as("SELECT f.* FROM faxes f WHERE f.tenant_id=$1 AND ($3 OR owner_id=$2 OR (direction='inbound' AND EXISTS(SELECT 1 FROM fax_endpoints e WHERE e.tenant_id=$1 AND e.id=f.endpoint_id AND e.enabled AND $2=ANY(e.receive_users)))) ORDER BY created_at DESC LIMIT 500").bind(tenant).bind(user).bind(all).fetch_all(db).await?)
}
pub async fn template_for(db: &PgPool, tenant: TenantId, user: Uuid) -> CoreResult<Template> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT header,footer FROM fax_templates WHERE tenant_id=$1 AND user_id=$2")
            .bind(tenant)
            .bind(user)
            .fetch_optional(db)
            .await?;
    let cfg = settings(db, tenant).await?;
    Ok(row
        .map(|(header, footer)| Template { header, footer })
        .unwrap_or(Template {
            header: cfg.header,
            footer: cfg.footer,
        }))
}
pub async fn save_template(
    db: &PgPool,
    tenant: TenantId,
    user: Uuid,
    t: &Template,
) -> CoreResult<()> {
    template(&t.header)?;
    template(&t.footer)?;
    sqlx::query("INSERT INTO fax_templates(tenant_id,user_id,header,footer) VALUES($1,$2,$3,$4) ON CONFLICT(tenant_id,user_id) DO UPDATE SET header=$3,footer=$4").bind(tenant).bind(user).bind(&t.header).bind(&t.footer).execute(db).await?;
    Ok(())
}
pub async fn sender_numbers(db: &PgPool, tenant: TenantId, user: Uuid) -> CoreResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar("SELECT DISTINCT outbound_number_id FROM extensions WHERE tenant_id=$1 AND user_id=$2 AND enabled AND outbound_number_id IS NOT NULL UNION SELECT n.id FROM numbers n JOIN fax_endpoints e ON e.tenant_id=n.tenant_id AND e.id=n.destination_id WHERE n.tenant_id=$1 AND n.enabled AND n.destination_type='fax' AND e.enabled AND (e.owner_id=$2 OR $2=ANY(e.send_users)) ORDER BY 1").bind(tenant).bind(user).fetch_all(db).await?)
}
pub async fn resolve_number(
    db: &PgPool,
    tenant: TenantId,
    user: Uuid,
    selected: Option<Uuid>,
) -> CoreResult<trunks::OutboundRoute> {
    if !user_can(db, tenant, user, Permission::FaxSend).await? {
        return Err(invalid("fax send permission required"));
    }
    let cfg = settings(db, tenant).await?;
    if !cfg.enabled {
        return Err(invalid("fax is disabled"));
    }
    let own = sender_numbers(db, tenant, user).await?;
    let id = if cfg.always_global {
        cfg.global_number_id
    } else {
        selected
            .or_else(|| own.first().copied())
            .or(cfg.global_number_id)
    }
    .ok_or_else(|| invalid("no outgoing fax number"))?;
    if !own.contains(&id)
        && (Some(id) != cfg.global_number_id
            || !user_can(db, tenant, user, Permission::FaxUseGlobal).await?)
    {
        return Err(invalid("fax number not permitted"));
    }
    trunks::outbound_route(db, tenant, id)
        .await?
        .ok_or_else(|| invalid("fax route is unavailable"))
}
#[allow(clippy::too_many_arguments)]
pub async fn create(
    db: &PgPool,
    tenant: TenantId,
    user: Uuid,
    id: Uuid,
    destination: &str,
    selected: Option<Uuid>,
    inputs: Vec<InputFile>,
    sender_email: &str,
    automatic: bool,
    receipt: Option<Receipt>,
) -> CoreResult<Fax> {
    if !number(destination) {
        return Err(invalid(
            "Subject must contain only an international number, e.g. +49123456.",
        ));
    }
    if inputs.is_empty() || inputs.len() > MAX_FILES {
        return Err(invalid("attach 1-10 PDF or image files"));
    }
    let route = resolve_number(db, tenant, user, selected).await?;
    let text = template_for(db, tenant, user).await?;
    let header = if sender_email.is_empty() {
        text.header
    } else {
        email(sender_email)?
    };
    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,681948252))")
        .bind(format!("{tenant}:{user}"))
        .execute(&mut *tx)
        .await?;
    let pending:i64=sqlx::query_scalar("SELECT count(*) FROM faxes WHERE tenant_id=$1 AND owner_id=$2 AND state IN ('preparing','draft','queued','dialing','sending')").bind(tenant).bind(user).fetch_one(&mut *tx).await?;
    if pending >= 20 {
        return Err(invalid("at most 20 pending fax documents per user"));
    }
    if let Some(receipt) = receipt {
        sqlx::query("INSERT INTO fax_mail_receipts(tenant_id,receipt,fingerprint,sender,result) VALUES($1,$2,$3,$4,'accepted')")
            .bind(tenant).bind(receipt.key).bind(receipt.fingerprint).bind(sender_email).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO faxes(id,tenant_id,owner_id,direction,state,number_id,trunk_id,source,destination,sender_email,header,footer,inputs) VALUES($1,$2,$3,'outbound','preparing',$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(id).bind(tenant).bind(user).bind(route.number_id).bind(route.trunk_id).bind(route.e164).bind(destination).bind(sender_email).bind(header).bind(text.footer).bind(serde_json::to_value(inputs).map_err(|_|invalid("invalid files"))?).execute(&mut *tx).await?;
    let mut job = crate::jobs::NewJob::new(
        PREPARE,
        serde_json::json!({"fax_id":id,"automatic":automatic}),
    );
    job.tenant_id = tenant;
    job.max_attempts = 1;
    crate::jobs::enqueue(&mut *tx, job).await?;
    crate::audit::record(
        &mut *tx,
        &crate::audit::Actor {
            tenant,
            user_id: Some(user),
            ip: None,
        },
        "create",
        "fax",
        Some(id.to_string()),
        serde_json::json!({"origin":if sender_email.is_empty(){"web"}else{"mail"}}),
    )
    .await?;
    tx.commit().await?;
    get(db, tenant, id).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_subject_and_whitelist_identity() {
        assert!(number("+49123456"));
        assert!(number("+12025550123"));
        for s in [
            "+49^123456",
            "Fax +49123456",
            "Re: +49123456",
            "0049123456",
            "+049123456",
            "+49 123456",
            "+49123456\n",
            "+1",
            "+1234567890123456",
        ] {
            assert!(!number(s), "{s}");
        }
        assert_eq!(email("Alice@Example.COM").unwrap(), "alice@example.com");
        for s in [
            "Alice <a@example.com>",
            "a@example.com,b@example.com",
            "*@example.com",
            "a@b@c",
            "a\r\n@b",
        ] {
            assert!(email(s).is_err(), "{s}");
        }
    }
    #[test]
    fn templates_are_bounded_and_explicit() {
        assert!(template("{sender} | {page}/{pages}").is_ok());
        for s in ["${system(x)}", "{unknown}", "line\nbreak"] {
            assert!(template(s).is_err());
        }
    }
}
