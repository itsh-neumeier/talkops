//! SMTP settings for outgoing mail (voicemail notifications).

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

/// SMTP settings as shown in the UI (without the password).
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct SmtpSettings {
    /// Empty = mail disabled.
    pub host: String,
    pub port: i32,
    /// `starttls`, `tls` (implicit, port 465) or `none`.
    pub security: String,
    pub username: String,
    pub has_password: bool,
    /// Sender address, e.g. `TalkOps <pbx@example.com>`.
    pub from: String,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct SmtpInput {
    pub host: String,
    pub port: i32,
    pub security: String,
    #[serde(default)]
    pub username: String,
    /// New password; `null` keeps the stored one, `""` removes it.
    #[serde(default)]
    pub password: Option<String>,
    pub from: String,
}

/// Settings including the decrypted password, for sending.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: String,
    pub password: Option<String>,
    pub from: String,
}

const COLUMNS: &str = "smtp_host AS host, smtp_port AS port, smtp_security AS security, \
                       smtp_username AS username, smtp_password_enc IS NOT NULL AS has_password, \
                       smtp_from AS \"from\"";

pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<SmtpSettings> {
    let sql = format!("SELECT {COLUMNS} FROM tenant_settings WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_one(pool).await?)
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    input: &SmtpInput,
) -> CoreResult<SmtpSettings> {
    let host = input.host.trim();
    if !host.is_empty() {
        if !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
        {
            return Err(CoreError::Validation("invalid SMTP host".into()));
        }
        if !input.from.contains('@') {
            return Err(CoreError::Validation("sender address is required".into()));
        }
    }
    if !["starttls", "tls", "none"].contains(&input.security.as_str()) {
        return Err(CoreError::Validation("invalid SMTP security mode".into()));
    }
    if !(1..=65535).contains(&input.port) {
        return Err(CoreError::Validation("invalid SMTP port".into()));
    }
    let password_enc = match input.password.as_deref() {
        None => None,
        Some("") => Some(None),
        Some(p) => Some(Some(secrets.encrypt(p)?)),
    };
    let sql = format!(
        "UPDATE tenant_settings SET smtp_host = $2, smtp_port = $3, smtp_security = $4,
             smtp_username = $5, smtp_from = $6,
             smtp_password_enc = CASE WHEN $7 THEN $8 ELSE smtp_password_enc END,
             updated_at = now()
         WHERE tenant_id = $1 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(host)
        .bind(input.port)
        .bind(&input.security)
        .bind(input.username.trim())
        .bind(input.from.trim())
        .bind(password_enc.is_some())
        .bind(password_enc.flatten())
        .fetch_one(pool)
        .await?)
}

/// The configuration for sending, or `None` if no SMTP host is set.
pub async fn config(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
) -> CoreResult<Option<SmtpConfig>> {
    let (host, port, security, username, password_enc, from): (
        String,
        i32,
        String,
        String,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT smtp_host, smtp_port, smtp_security, smtp_username, smtp_password_enc, smtp_from
         FROM tenant_settings WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_one(pool)
    .await?;
    if host.is_empty() {
        return Ok(None);
    }
    let password = password_enc.map(|p| secrets.decrypt(&p)).transpose()?;
    Ok(Some(SmtpConfig {
        host,
        port: port as u16,
        security,
        username,
        password,
        from,
    }))
}
