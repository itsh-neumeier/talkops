//! Portainer API connection (ADR 0032). TalkOps reads the stack file and
//! the stack environment of its own stack before each backup, so recovery
//! backups contain the deployment configuration without a manual import.
//! The access token is stored encrypted (ADR 0008) and never returned.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct PortainerConnection {
    pub url: String,
    pub stack_id: i32,
    /// SHA-256 of the pinned certificate (hex); empty for a public CA.
    pub cert_sha256: String,
    pub stack_name: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Clone, Deserialize, utoipa::ToSchema)]
pub struct PortainerInput {
    pub url: String,
    pub stack_id: i32,
    #[serde(default)]
    pub cert_sha256: String,
    /// New access token; `null` keeps the stored token.
    #[serde(default)]
    pub token: Option<String>,
}

impl std::fmt::Debug for PortainerInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortainerInput")
            .field("url", &self.url)
            .field("stack_id", &self.stack_id)
            .finish_non_exhaustive()
    }
}

/// `https://` URL, or `http://` to the local host only (the token would
/// cross the network without encryption).
pub fn check_url(url: &str) -> CoreResult<String> {
    let url = url.trim().trim_end_matches('/');
    if !crate::transcription_api::valid_url(url) {
        return Err(CoreError::Validation(
            "the Portainer URL must start with https://".into(),
        ));
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let host = rest.split(['/', ':']).next().unwrap_or_default();
        let host = if rest.starts_with('[') {
            rest.split(']')
                .next()
                .unwrap_or_default()
                .trim_start_matches('[')
        } else {
            host
        };
        if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
            return Err(CoreError::Validation(
                "use https:// for Portainer on another host; http:// is only allowed for 127.0.0.1"
                    .into(),
            ));
        }
    }
    Ok(url.to_owned())
}

pub fn check_token(token: &str) -> CoreResult<()> {
    if token.is_empty()
        || token.len() > 500
        || token.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(CoreError::Validation("invalid access token".into()));
    }
    Ok(())
}

pub fn check_fingerprint(fp: &str) -> CoreResult<String> {
    let fp = fp.trim().to_ascii_lowercase().replace(':', "");
    if !fp.is_empty() && (fp.len() != 64 || !fp.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err(CoreError::Validation(
            "invalid certificate fingerprint".into(),
        ));
    }
    Ok(fp)
}

const COLUMNS: &str = "url, stack_id, cert_sha256, stack_name, last_sync_at, last_error";

pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<Option<PortainerConnection>> {
    let sql = format!("SELECT {COLUMNS} FROM portainer_connections WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .fetch_optional(pool)
        .await?)
}

/// The decrypted access token.
pub async fn token(pool: &PgPool, tenant: TenantId, secrets: &SecretBox) -> CoreResult<String> {
    let enc: String =
        sqlx::query_scalar("SELECT token_enc FROM portainer_connections WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_one(pool)
            .await?;
    Ok(secrets.decrypt(&enc)?)
}

/// Creates or changes the connection. A new connection needs a token.
pub async fn save(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    input: &PortainerInput,
) -> CoreResult<PortainerConnection> {
    let url = check_url(&input.url)?;
    let cert = check_fingerprint(&input.cert_sha256)?;
    if input.stack_id <= 0 {
        return Err(CoreError::Validation("select a stack".into()));
    }
    let token_enc = match input
        .token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => {
            check_token(t)?;
            Some(secrets.encrypt(t)?)
        }
        None => None,
    };
    if token_enc.is_none() && get(pool, tenant).await?.is_none() {
        return Err(CoreError::Validation("enter the access token".into()));
    }
    let sql = format!(
        "INSERT INTO portainer_connections (tenant_id, url, stack_id, cert_sha256, token_enc)
         SELECT $1, $2, $3, $4, COALESCE($5, (SELECT token_enc FROM portainer_connections WHERE tenant_id = $1))
         ON CONFLICT (tenant_id) DO UPDATE SET url = EXCLUDED.url, stack_id = EXCLUDED.stack_id,
             cert_sha256 = EXCLUDED.cert_sha256, token_enc = EXCLUDED.token_enc,
             stack_name = NULL, last_error = NULL, updated_at = now()
         RETURNING {COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(url)
        .bind(input.stack_id)
        .bind(cert)
        .bind(token_enc)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

pub async fn delete(pool: &PgPool, tenant: TenantId) -> CoreResult<()> {
    sqlx::query("DELETE FROM portainer_connections WHERE tenant_id = $1")
        .bind(tenant)
        .execute(pool)
        .await?;
    Ok(())
}

/// Records the outcome of a sync (`Ok(stack name)` or `Err(message)`).
pub async fn record_sync(
    pool: &PgPool,
    tenant: TenantId,
    result: Result<&str, &str>,
) -> CoreResult<()> {
    let (name, error) = match result {
        Ok(n) => (Some(n), None),
        Err(e) => (None, Some(e.chars().take(500).collect::<String>())),
    };
    sqlx::query(
        "UPDATE portainer_connections SET last_sync_at = now(),
             stack_name = COALESCE($2, stack_name), last_error = $3
         WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(name)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_tokens_and_fingerprints() {
        assert_eq!(
            check_url("https://portainer.local:9443/").unwrap(),
            "https://portainer.local:9443"
        );
        assert!(check_url("http://127.0.0.1:9000").is_ok());
        assert!(check_url("http://localhost:9000").is_ok());
        assert!(check_url("http://[::1]:9000").is_ok());
        assert!(check_url("http://nas.local:9000").is_err());
        assert!(check_url("ftp://x").is_err());
        assert!(check_token("ptr_abc").is_ok());
        assert!(check_token("a b").is_err());
        let fp = "AB:".repeat(31) + "AB";
        assert_eq!(check_fingerprint(&fp).unwrap(), "ab".repeat(32));
        assert_eq!(check_fingerprint("").unwrap(), "");
        assert!(check_fingerprint("abc").is_err());
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn store_connection_with_encrypted_token(pool: PgPool) {
        let secrets = SecretBox::from_hex(&"cd".repeat(32)).unwrap();
        let tenant = TenantId::DEFAULT;
        let mut input = PortainerInput {
            url: "https://127.0.0.1:9443".into(),
            stack_id: 3,
            cert_sha256: String::new(),
            token: None,
        };
        assert!(matches!(
            save(&pool, tenant, &secrets, &input).await,
            Err(CoreError::Validation(_))
        ));
        input.token = Some("ptr_secret-token".into());
        let saved = save(&pool, tenant, &secrets, &input).await.unwrap();
        assert_eq!(saved.stack_id, 3);
        let raw: String = sqlx::query_scalar("SELECT token_enc FROM portainer_connections")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!raw.contains("ptr_secret"));
        // A change without a token keeps it.
        input.token = None;
        input.stack_id = 4;
        save(&pool, tenant, &secrets, &input).await.unwrap();
        assert_eq!(
            token(&pool, tenant, &secrets).await.unwrap(),
            "ptr_secret-token"
        );
        record_sync(&pool, tenant, Ok("talkops")).await.unwrap();
        let c = get(&pool, tenant).await.unwrap().unwrap();
        assert_eq!(c.stack_name.as_deref(), Some("talkops"));
        assert!(c.last_error.is_none() && c.last_sync_at.is_some());
        delete(&pool, tenant).await.unwrap();
        assert!(get(&pool, tenant).await.unwrap().is_none());
    }
}
