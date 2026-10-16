//! Portainer API client (ADR 0032). Before each backup, TalkOps reads the
//! stack file and the stack environment of its own stack and stores them
//! as the encrypted imported configuration. Portainer often uses a
//! self-signed certificate: the administrator pins its SHA-256
//! fingerprint (trust on first use, like SFTP host keys).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::portainer;
use talkops_core::tenant::TenantId;

use crate::backup::BackupConfig;
use crate::recovery::{self, DeploymentInfo, DeploymentInput};

#[derive(Debug, thiserror::Error)]
pub enum PortainerError {
    #[error("Portainer refused the access token")]
    Unauthorized,
    #[error("the stack does not exist in Portainer")]
    NotFound,
    #[error("the certificate of Portainer does not match the pinned fingerprint")]
    CertificateMismatch,
    #[error("Portainer is not reachable: {0}")]
    Http(String),
    #[error("{0}")]
    Invalid(String),
}

/// A stack in the stack list.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Stack {
    #[serde(rename(deserialize = "Id"))]
    pub id: i32,
    #[serde(rename(deserialize = "Name"))]
    pub name: String,
}

#[derive(Deserialize)]
struct StackDetails {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Env", default)]
    env: Option<Vec<EnvVar>>,
}

#[derive(Deserialize)]
struct EnvVar {
    name: String,
    #[serde(default)]
    value: String,
}

#[derive(Deserialize)]
struct StackFile {
    #[serde(rename = "StackFileContent")]
    content: String,
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

/// Accepts exactly one certificate (`expected`), or records the
/// certificate without a check (`expected = None`, only for the probe,
/// which sends no token).
#[derive(Debug)]
struct Pinned {
    expected: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let hash = sha256_hex(end_entity.as_ref());
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(hash.clone());
        match &self.expected {
            None => Ok(ServerCertVerified::assertion()),
            Some(expected) if *expected == hash => Ok(ServerCertVerified::assertion()),
            Some(_) => Err(rustls::Error::General(
                "certificate fingerprint mismatch".into(),
            )),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A client for the public CAs, or for one pinned certificate.
fn client(pin: Option<Option<String>>, seen: Arc<Mutex<Option<String>>>) -> reqwest::Client {
    talkops_doorbell::ensure_tls_provider();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions");
    let tls = match pin {
        None => {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            builder.with_root_certificates(roots).with_no_client_auth()
        }
        Some(expected) => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pinned {
                expected,
                seen,
                provider,
            }))
            .with_no_client_auth(),
    };
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(format!("TalkOps/{}", talkops_core::VERSION))
        .build()
        .expect("Portainer HTTP client")
}

/// Connection data for one request.
pub struct Api<'a> {
    pub url: &'a str,
    pub token: &'a str,
    /// Pinned certificate (hex); empty for a public CA.
    pub cert_sha256: &'a str,
}

impl Api<'_> {
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, PortainerError> {
        let seen = Arc::new(Mutex::new(None));
        let pin = (!self.cert_sha256.is_empty()).then(|| Some(self.cert_sha256.to_owned()));
        let pinned = pin.is_some();
        let res = client(pin, seen.clone())
            .get(format!("{}{path}", self.url))
            .header("X-API-Key", self.token)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                let mismatch = pinned
                    && seen
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_deref()
                        .is_some_and(|s| s != self.cert_sha256);
                if mismatch {
                    PortainerError::CertificateMismatch
                } else {
                    PortainerError::Http(e.without_url().to_string())
                }
            })?;
        match res.status().as_u16() {
            200 => {}
            401 | 403 => return Err(PortainerError::Unauthorized),
            404 => return Err(PortainerError::NotFound),
            code => return Err(PortainerError::Http(format!("HTTP status {code}"))),
        }
        let body = res
            .bytes()
            .await
            .map_err(|e| PortainerError::Http(e.without_url().to_string()))?;
        if body.len() > 4 * recovery::CONFIG_LIMIT as usize {
            return Err(PortainerError::Invalid("Portainer answer too large".into()));
        }
        serde_json::from_slice(&body)
            .map_err(|_| PortainerError::Invalid("unexpected answer from Portainer".into()))
    }

    pub async fn stacks(&self) -> Result<Vec<Stack>, PortainerError> {
        let mut stacks: Vec<Stack> = self.get("/api/stacks").await?;
        stacks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(stacks)
    }

    /// The stack as deployment files: `docker-compose.yml` and `.env`.
    pub async fn deployment(
        &self,
        stack_id: i32,
        default_project: &str,
    ) -> Result<(String, DeploymentInput), PortainerError> {
        let details: StackDetails = self.get(&format!("/api/stacks/{stack_id}")).await?;
        let file: StackFile = self.get(&format!("/api/stacks/{stack_id}/file")).await?;
        let mut files = BTreeMap::from([("docker-compose.yml".to_owned(), file.content)]);
        if let Some(env) = details.env.filter(|e| !e.is_empty()) {
            files.insert(".env".into(), env_file(&env));
        }
        let project = if details
            .name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
            && !details.name.is_empty()
        {
            details.name.clone()
        } else {
            default_project.to_owned()
        };
        Ok((
            details.name,
            DeploymentInput {
                files,
                project_name: project,
                profiles: String::new(),
                origin: Some("portainer".into()),
            },
        ))
    }
}

/// Stack environment as `.env`. The backup password stays out: it must
/// come from a separate place (ADR 0021).
fn env_file(env: &[EnvVar]) -> String {
    let mut out = String::new();
    for var in env {
        let valid_name = var
            .name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && var
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid_name || var.name == "TALKOPS_BACKUP_PASSWORD" {
            continue;
        }
        if var.value.contains(['\n', '\r']) {
            let escaped = var
                .value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\r', "")
                .replace('\n', "\\n");
            out.push_str(&format!("{}=\"{escaped}\"\n", var.name));
        } else {
            out.push_str(&format!("{}={}\n", var.name, var.value));
        }
    }
    out
}

/// The SHA-256 fingerprint of the certificate at `url` (hex), or `None`
/// for `http://`. Sends no token.
pub async fn probe_certificate(url: &str) -> Result<Option<String>, PortainerError> {
    if url.starts_with("http://") {
        return Ok(None);
    }
    let seen = Arc::new(Mutex::new(None));
    // Any HTTP status is fine: only the TLS handshake matters.
    let _ = client(Some(None), seen.clone())
        .get(format!("{url}/api/system/status"))
        .send()
        .await
        .map_err(|e| PortainerError::Http(e.without_url().to_string()))?;
    let hash = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    hash.map(Some)
        .ok_or_else(|| PortainerError::Http("no certificate received".into()))
}

/// Reads the stack and stores it as the imported configuration.
pub async fn sync(
    cfg: &BackupConfig,
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
) -> Result<DeploymentInfo, PortainerError> {
    let conn = portainer::get(db, tenant)
        .await
        .map_err(|e| PortainerError::Invalid(e.to_string()))?
        .ok_or_else(|| PortainerError::Invalid("no Portainer connection".into()))?;
    let result = async {
        let token = portainer::token(db, tenant, secrets)
            .await
            .map_err(|e| PortainerError::Invalid(e.to_string()))?;
        let api = Api {
            url: &conn.url,
            token: &token,
            cert_sha256: &conn.cert_sha256,
        };
        let (name, input) = api.deployment(conn.stack_id, &cfg.project_name).await?;
        recovery::validate_configuration(&input)
            .map_err(|e| PortainerError::Invalid(e.to_string()))?;
        recovery::store_import(cfg, secrets, &input)
            .map_err(|e| PortainerError::Invalid(e.to_string()))?;
        Ok::<_, PortainerError>((
            name,
            recovery::deployment_info(&input, "portainer", chrono::Utc::now()),
        ))
    }
    .await;
    match result {
        Ok((name, info)) => {
            let _ = portainer::record_sync(db, tenant, Ok(&name)).await;
            Ok(info)
        }
        Err(err) => {
            let _ = portainer::record_sync(db, tenant, Err(&err.to_string())).await;
            Err(err)
        }
    }
}

/// Before a backup: refreshes the configuration from Portainer when a
/// connection exists and no mounted files take precedence. A failure keeps
/// the last stored copy; the backup continues.
pub async fn refresh(cfg: &BackupConfig, db: &PgPool) {
    let Some(key) = &cfg.deployment_key else {
        return;
    };
    use age::secrecy::ExposeSecret;
    let Ok(secrets) = SecretBox::from_hex(key.expose_secret()) else {
        return;
    };
    if recovery::mounted_active(cfg) {
        return;
    }
    let tenant = TenantId::DEFAULT;
    if !matches!(portainer::get(db, tenant).await, Ok(Some(_))) {
        return;
    }
    match sync(cfg, db, &secrets, tenant).await {
        Ok(info) => {
            tracing::info!(files = ?info.files, "deployment configuration read from Portainer")
        }
        Err(err) => {
            tracing::warn!(error = %err, "cannot read the stack from Portainer; using the last copy")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_environment_as_env_file() {
        let env = vec![
            EnvVar {
                name: "POSTGRES_PASSWORD".into(),
                value: "a b#c".into(),
            },
            EnvVar {
                name: "TALKOPS_BACKUP_PASSWORD".into(),
                value: "never".into(),
            },
            EnvVar {
                name: "BAD NAME".into(),
                value: "x".into(),
            },
            EnvVar {
                name: "MULTI".into(),
                value: "line1\nline\"2".into(),
            },
        ];
        let text = env_file(&env);
        assert_eq!(
            text,
            "POSTGRES_PASSWORD=a b#c\nMULTI=\"line1\\nline\\\"2\"\n"
        );
        assert_eq!(
            recovery::env_value(&text, "POSTGRES_PASSWORD").as_deref(),
            Some("a b#c")
        );
        assert!(!text.contains("never"));
    }
}
