//! OpenAI-compatible transcription API (`POST {url}/audio/transcriptions`):
//! OpenAI, Groq, Mistral or a self-hosted server such as Speaches. The key
//! is stored encrypted (ADR 0008) and never returned.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

/// Settings as shown in the UI (without the key).
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct TranscriptionApi {
    /// Base URL including the version, e.g. `https://api.openai.com/v1`.
    pub url: String,
    pub model: String,
    pub has_key: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct TranscriptionApiInput {
    pub url: String,
    pub model: String,
    /// New key; `null` keeps the stored one, `""` removes it.
    #[serde(default)]
    pub key: Option<String>,
}

/// Settings including the decrypted key, for the media worker.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub url: String,
    pub model: String,
    pub key: Option<String>,
}

impl ApiConfig {
    /// The transcription endpoint.
    pub fn endpoint(&self) -> String {
        format!("{}/audio/transcriptions", self.url.trim_end_matches('/'))
    }
}

const COLUMNS: &str = "transcription_api_url AS url, transcription_api_model AS model, \
                       transcription_api_key_enc IS NOT NULL AS has_key";

pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<TranscriptionApi> {
    let sql = format!("SELECT {COLUMNS} FROM tenant_settings WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_one(pool).await?)
}

/// `http(s)://host[:port][/path]` without spaces, quotes or a query.
pub fn valid_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(rest) => {
            let host = rest.split('/').next().unwrap_or_default();
            !host.is_empty()
                && url.len() <= 300
                && !url.chars().any(|c| {
                    c.is_whitespace()
                        || c.is_control()
                        || matches!(c, '"' | '\'' | '?' | '#' | '\\')
                })
        }
        None => false,
    }
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 200
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':'))
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    input: &TranscriptionApiInput,
) -> CoreResult<TranscriptionApi> {
    let url = input.url.trim().trim_end_matches('/');
    let model = input.model.trim();
    if !valid_url(url) {
        return Err(CoreError::Validation(
            "API URL must start with http:// or https://".into(),
        ));
    }
    if !valid_model(model) {
        return Err(CoreError::Validation("invalid model name".into()));
    }
    let key_enc = match input.key.as_deref().map(str::trim) {
        None => None,
        Some("") => Some(None),
        Some(k)
            if k.len() > 500
                || k.chars()
                    .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\\')) =>
        {
            return Err(CoreError::Validation("invalid API key".into()));
        }
        Some(k) => Some(Some(secrets.encrypt(k)?)),
    };
    let sql = format!(
        "UPDATE tenant_settings SET transcription_api_url = $2, transcription_api_model = $3,
             transcription_api_key_enc = CASE WHEN $4 THEN $5 ELSE transcription_api_key_enc END,
             updated_at = now()
         WHERE tenant_id = $1 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(url)
        .bind(model)
        .bind(key_enc.is_some())
        .bind(key_enc.flatten())
        .fetch_one(pool)
        .await?)
}

/// The configuration with the decrypted key.
pub async fn config(pool: &PgPool, tenant: TenantId, secrets: &SecretBox) -> CoreResult<ApiConfig> {
    let (url, model, key_enc): (String, String, Option<String>) = sqlx::query_as(
        "SELECT transcription_api_url, transcription_api_model, transcription_api_key_enc
         FROM tenant_settings WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_one(pool)
    .await?;
    let key = key_enc.map(|k| secrets.decrypt(&k)).transpose()?;
    Ok(ApiConfig { url, model, key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert!(valid_url("https://api.openai.com/v1"));
        assert!(valid_url("http://192.168.1.5:8000/v1"));
        assert!(!valid_url("ftp://x"));
        assert!(!valid_url("https://"));
        assert!(!valid_url("https://a b/v1"));
        assert!(!valid_url("https://x/v1?k=1"));
        assert!(!valid_url("file:///etc/passwd"));
    }

    #[test]
    fn models() {
        assert!(valid_model("whisper-1"));
        assert!(valid_model("Systran/faster-whisper-large-v3"));
        assert!(!valid_model(""));
        assert!(!valid_model("a b"));
    }
}
