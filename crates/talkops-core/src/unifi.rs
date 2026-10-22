//! Read-only connection to the UniFi Network application (ADR 0041). The
//! API key is stored encrypted (ADR 0008) and never returned.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct UnifiConnection {
    pub url: String,
    pub site: String,
    /// SHA-256 of the pinned certificate (hex); empty for a public CA.
    pub cert_sha256: String,
    /// Network application version of the last test.
    pub version: Option<String>,
    /// Endpoints that answered in the last test.
    #[schema(value_type = Object)]
    pub capabilities: serde_json::Value,
    pub last_check_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Clone, Deserialize, utoipa::ToSchema)]
pub struct UnifiInput {
    /// Console address, e.g. `https://192.168.1.1`.
    pub url: String,
    #[serde(default = "default_site")]
    pub site: String,
    #[serde(default)]
    pub cert_sha256: String,
    /// New API key; `null` or empty keeps the stored key.
    #[serde(default)]
    pub api_key: Option<String>,
}

fn default_site() -> String {
    "default".into()
}

impl std::fmt::Debug for UnifiInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiInput")
            .field("url", &self.url)
            .field("site", &self.site)
            .finish_non_exhaustive()
    }
}

/// `https://` only: the API key must not cross the network unencrypted.
pub fn check_url(url: &str) -> CoreResult<String> {
    let url = url.trim().trim_end_matches('/');
    let host = url.strip_prefix("https://").unwrap_or_default();
    if host.is_empty()
        || url.len() > 200
        || host.contains(['?', '#', '@', ' '])
        || host.chars().any(|c| c.is_control())
    {
        return Err(CoreError::Validation(
            "the UniFi address must start with https://".into(),
        ));
    }
    Ok(url.to_owned())
}

pub fn check_site(site: &str) -> CoreResult<String> {
    let site = site.trim();
    if site.is_empty()
        || site.len() > 64
        || !site
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(CoreError::Validation("invalid UniFi site name".into()));
    }
    Ok(site.to_owned())
}

pub fn check_api_key(key: &str) -> CoreResult<()> {
    if key.is_empty() || key.len() > 200 || key.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(CoreError::Validation("invalid API key".into()));
    }
    Ok(())
}

const COLUMNS: &str = "url, site, cert_sha256, version, capabilities, last_check_at, last_error";

pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<Option<UnifiConnection>> {
    let sql = format!("SELECT {COLUMNS} FROM unifi_connections WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .fetch_optional(pool)
        .await?)
}

/// The decrypted API key.
pub async fn api_key(pool: &PgPool, tenant: TenantId, secrets: &SecretBox) -> CoreResult<String> {
    let enc: String =
        sqlx::query_scalar("SELECT api_key_enc FROM unifi_connections WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_one(pool)
            .await?;
    Ok(secrets.decrypt(&enc)?)
}

/// Creates or changes the connection. A new connection needs an API key.
pub async fn save(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    input: &UnifiInput,
) -> CoreResult<UnifiConnection> {
    let url = check_url(&input.url)?;
    let site = check_site(&input.site)?;
    let cert = crate::portainer::check_fingerprint(&input.cert_sha256)?;
    let key_enc = match input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        Some(k) => {
            check_api_key(k)?;
            Some(secrets.encrypt(k)?)
        }
        None => None,
    };
    if key_enc.is_none() && get(pool, tenant).await?.is_none() {
        return Err(CoreError::Validation("enter the API key".into()));
    }
    let sql = format!(
        "INSERT INTO unifi_connections (tenant_id, url, site, cert_sha256, api_key_enc)
         SELECT $1, $2, $3, $4, COALESCE($5, (SELECT api_key_enc FROM unifi_connections WHERE tenant_id = $1))
         ON CONFLICT (tenant_id) DO UPDATE SET url = EXCLUDED.url, site = EXCLUDED.site,
             cert_sha256 = EXCLUDED.cert_sha256, api_key_enc = EXCLUDED.api_key_enc,
             version = NULL, capabilities = '{{}}', last_check_at = NULL, last_error = NULL,
             updated_at = now()
         RETURNING {COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(url)
        .bind(site)
        .bind(cert)
        .bind(key_enc)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

pub async fn delete(pool: &PgPool, tenant: TenantId) -> CoreResult<()> {
    sqlx::query("DELETE FROM unifi_connections WHERE tenant_id = $1")
        .bind(tenant)
        .execute(pool)
        .await?;
    Ok(())
}

/// Records the result of a connection test.
pub async fn record_check(
    pool: &PgPool,
    tenant: TenantId,
    result: Result<(&str, &serde_json::Value), &str>,
) -> CoreResult<()> {
    let (version, caps, error) = match result {
        Ok((v, c)) => (Some(v.to_owned()), Some(c.clone()), None),
        Err(e) => (None, None, Some(e.chars().take(300).collect::<String>())),
    };
    sqlx::query(
        "UPDATE unifi_connections SET last_check_at = now(), last_error = $4,
             version = COALESCE($2, version), capabilities = COALESCE($3, capabilities)
         WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(version)
    .bind(caps)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_input() {
        assert_eq!(
            check_url("https://192.168.1.1/").unwrap(),
            "https://192.168.1.1"
        );
        assert!(check_url("http://192.168.1.1").is_err());
        assert!(check_url("https://user@host").is_err());
        assert!(check_url("https://").is_err());
        assert_eq!(check_site(" default ").unwrap(), "default");
        assert!(check_site("a/b").is_err());
        assert!(check_api_key("abc def").is_err());
        assert!(check_api_key("Zx9-abc_DEF").is_ok());
        let input = UnifiInput {
            url: "https://u".into(),
            site: "default".into(),
            cert_sha256: String::new(),
            api_key: Some("secret-key".into()),
        };
        assert!(!format!("{input:?}").contains("secret-key"));
    }
}
