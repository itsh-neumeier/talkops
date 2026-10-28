//! Cloud transcription connections (ADR 0024): OpenAI or another server with
//! the OpenAI interface (`POST {url}/audio/transcriptions`), such as Groq,
//! Mistral or a self-hosted Speaches. A tenant can have several. The key is
//! stored encrypted (ADR 0008) and never returned.
//!
//! The kinds `coral` (coral-stt, ADR 0046) and `cuda` (cuda-stt, ADR 0049)
//! are speech services on this server: the same interface, on a loopback
//! address only, without costs, and with a local fallback when they are not
//! available.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

/// The fixed base URL of connections of kind `openai`.
pub const OPENAI_URL: &str = "https://api.openai.com/v1";
/// The default base URL of connections of kind `coral`.
pub const CORAL_URL: &str = "http://127.0.0.1:8095/v1";
/// The default base URL of connections of kind `cuda`.
pub const CUDA_URL: &str = "http://127.0.0.1:8096/v1";

/// A speech service on this server (`coral` or `cuda`).
pub fn is_local_kind(kind: &str) -> bool {
    matches!(kind, "coral" | "cuda")
}

/// A connection as shown in the UI (without the key).
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Provider {
    pub id: Uuid,
    pub name: String,
    /// `openai`, `compatible`, `coral` or `cuda`.
    pub kind: String,
    /// Base URL including the version, e.g. `https://api.openai.com/v1`.
    pub url: String,
    pub has_key: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ProviderInput {
    pub name: String,
    /// `openai` (URL is fixed), `compatible`, `coral` or `cuda`.
    pub kind: String,
    /// Ignored for `openai`; empty means [CORAL_URL] or [CUDA_URL].
    #[serde(default)]
    pub url: String,
    /// New key; `null` keeps the stored one, `""` removes it.
    #[serde(default)]
    pub key: Option<String>,
}

/// A connection with a model and the decrypted key, for the media worker.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub name: String,
    /// `openai`, `compatible`, `coral` or `cuda`.
    pub kind: String,
    pub url: String,
    pub model: String,
    pub key: Option<String>,
}

impl ApiConfig {
    /// The transcription endpoint.
    pub fn endpoint(&self) -> String {
        format!("{}/audio/transcriptions", self.url.trim_end_matches('/'))
    }

    /// The WebSocket URL of a Realtime transcription session.
    pub fn realtime_endpoint(&self) -> String {
        let url = self.url.trim_end_matches('/');
        let url = match url.strip_prefix("https://") {
            Some(rest) => format!("wss://{rest}"),
            None => format!("ws://{}", url.strip_prefix("http://").unwrap_or(url)),
        };
        format!("{url}/realtime?intent=transcription")
    }

    /// Label for transcripts and usage, e.g. `OpenAI/gpt-transcribe`.
    pub fn label(&self) -> String {
        format!("{}/{}", self.name, self.model)
    }

    /// A speech service on this server (coral-stt or cuda-stt).
    pub fn is_local(&self) -> bool {
        is_local_kind(&self.kind)
    }

    /// The base URL without the API version (`/health` of coral-stt and
    /// cuda-stt).
    pub fn root(&self) -> String {
        let url = self.url.trim_end_matches('/');
        url.strip_suffix("/v1").unwrap_or(url).to_owned()
    }
}

/// A loopback host: `127.x.x.x`, `localhost` or `[::1]`. Audio for a
/// `coral` or `cuda` connection does not leave the server.
pub fn is_loopback_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or_default();
    let host_port = rest.split('/').next().unwrap_or_default();
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next().unwrap_or_default()
    } else {
        host_port.split(':').next().unwrap_or_default()
    };
    host == "localhost"
        || host == "::1"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_loopback())
}

const COLUMNS: &str = "id, name, kind, url, key_enc IS NOT NULL AS has_key";

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<Provider>> {
    let sql =
        format!("SELECT {COLUMNS} FROM transcription_providers WHERE tenant_id = $1 ORDER BY name");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?)
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Provider> {
    let sql =
        format!("SELECT {COLUMNS} FROM transcription_providers WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?)
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

/// Model names as providers use them, e.g. `Systran/faster-whisper-large-v3`.
pub fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 200
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':'))
}

/// Checked values: name, kind, URL and the key change (`None` = keep,
/// `Some(None)` = remove).
type Checked = (String, String, String, Option<Option<String>>);

fn check(secrets: &SecretBox, input: &ProviderInput) -> CoreResult<Checked> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err(CoreError::Validation(
            "name must have 1 to 60 characters".into(),
        ));
    }
    let url = match input.kind.as_str() {
        "openai" => OPENAI_URL.to_owned(),
        "compatible" => input.url.trim().trim_end_matches('/').to_owned(),
        kind @ ("coral" | "cuda") => match input.url.trim().trim_end_matches('/') {
            "" if kind == "coral" => CORAL_URL.to_owned(),
            "" => CUDA_URL.to_owned(),
            url => url.to_owned(),
        },
        _ => {
            return Err(CoreError::Validation(
                "kind must be openai, compatible, coral or cuda".into(),
            ));
        }
    };
    if !valid_url(&url) {
        return Err(CoreError::Validation(
            "API URL must start with http:// or https://".into(),
        ));
    }
    if is_local_kind(&input.kind) && !is_loopback_url(&url) {
        return Err(CoreError::Validation(
            "a Coral or CUDA connection must use a loopback address (127.0.0.1, localhost or [::1])"
                .into(),
        ));
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
    Ok((name.to_owned(), input.kind.clone(), url, key_enc))
}

pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    input: &ProviderInput,
) -> CoreResult<Provider> {
    let (name, kind, url, key_enc) = check(secrets, input)?;
    let sql = format!(
        "INSERT INTO transcription_providers (tenant_id, name, kind, url, key_enc)
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(name)
        .bind(kind)
        .bind(url)
        .bind(key_enc.flatten())
        .fetch_one(pool)
        .await?)
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
    input: &ProviderInput,
) -> CoreResult<Provider> {
    let (name, kind, url, key_enc) = check(secrets, input)?;
    let sql = format!(
        "UPDATE transcription_providers SET name = $3, kind = $4, url = $5,
             key_enc = CASE WHEN $6 THEN $7 ELSE key_enc END, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(name)
        .bind(kind)
        .bind(url)
        .bind(key_enc.is_some())
        .bind(key_enc.flatten())
        .fetch_one(pool)
        .await?)
}

/// Deletes a connection that no profile uses.
pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let used: Vec<String> = sqlx::query_scalar(
        "SELECT purpose FROM transcription_profiles
         WHERE tenant_id = $1 AND provider_id = $2 ORDER BY purpose",
    )
    .bind(tenant)
    .bind(id)
    .fetch_all(pool)
    .await?;
    if !used.is_empty() {
        return Err(CoreError::Conflict(format!(
            "connection is in use for: {}",
            used.join(", ")
        )));
    }
    let done = sqlx::query("DELETE FROM transcription_providers WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// A connection with the decrypted key and the given model. `secrets` is
/// only needed when the connection has a key.
pub async fn config(
    pool: &PgPool,
    tenant: TenantId,
    secrets: Option<&SecretBox>,
    id: Uuid,
    model: &str,
) -> CoreResult<ApiConfig> {
    let (name, kind, url, key_enc): (String, String, String, Option<String>) = sqlx::query_as(
        "SELECT name, kind, url, key_enc FROM transcription_providers
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .fetch_one(pool)
    .await?;
    let key = match (key_enc, secrets) {
        (Some(k), Some(secrets)) => Some(secrets.decrypt(&k)?),
        (Some(_), None) => {
            return Err(CoreError::Validation(
                "TALKOPS_SECRET_KEY is not set (needed for the API key)".into(),
            ));
        }
        (None, _) => None,
    };
    Ok(ApiConfig {
        name,
        kind,
        url,
        model: model.to_owned(),
        key,
    })
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

    #[test]
    fn endpoints() {
        let mut cfg = ApiConfig {
            name: "OpenAI".into(),
            kind: "openai".into(),
            url: "https://api.openai.com/v1/".into(),
            model: "gpt-live-transcribe".into(),
            key: None,
        };
        assert_eq!(
            cfg.endpoint(),
            "https://api.openai.com/v1/audio/transcriptions"
        );
        assert_eq!(
            cfg.realtime_endpoint(),
            "wss://api.openai.com/v1/realtime?intent=transcription"
        );
        assert_eq!(cfg.label(), "OpenAI/gpt-live-transcribe");
        cfg.url = "http://10.0.0.5:8000/v1".into();
        assert_eq!(
            cfg.realtime_endpoint(),
            "ws://10.0.0.5:8000/v1/realtime?intent=transcription"
        );
        assert!(!cfg.is_local());
        cfg.url = CORAL_URL.into();
        cfg.kind = "coral".into();
        assert!(cfg.is_local());
        cfg.kind = "cuda".into();
        assert!(cfg.is_local());
        assert_eq!(cfg.root(), "http://127.0.0.1:8095");
        assert_eq!(
            cfg.realtime_endpoint(),
            "ws://127.0.0.1:8095/v1/realtime?intent=transcription"
        );
    }

    #[test]
    fn loopback_urls() {
        assert!(is_loopback_url("http://127.0.0.1:8095/v1"));
        assert!(is_loopback_url("http://127.1.2.3/v1"));
        assert!(is_loopback_url("http://localhost:8095/v1"));
        assert!(is_loopback_url("http://[::1]:8095/v1"));
        assert!(!is_loopback_url("http://192.168.1.5:8095/v1"));
        assert!(!is_loopback_url("http://127.0.0.1.example.com/v1"));
        assert!(!is_loopback_url("http://localhost.example.com/v1"));
    }
}
