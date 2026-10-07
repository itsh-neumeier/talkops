//! Single sign-on with OpenID Connect (authorization code flow with PKCE).
//!
//! `/api/v1/auth/oidc/start` redirects to the provider; its callback
//! exchanges the code, verifies the ID token against the provider's keys
//! (signature, issuer, audience, expiry, nonce) and starts a session. The
//! user's role comes from the groups claim.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use talkops_core::identity::{self, ExternalAccount, IdentitySettings};
use talkops_core::tenant::TenantId;
use talkops_core::{audit, crypto};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::routes::auth::{RequestMeta, new_session};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(oidc_info))
        .routes(routes!(oidc_start))
        .routes(routes!(oidc_callback))
}

/// Provider metadata from `/.well-known/openid-configuration`.
#[derive(Debug, Clone, Deserialize)]
pub struct Discovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        crate::doors::ensure_tls_provider();
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .timeout(Duration::from_secs(10))
            // Never follow redirects to other hosts with credentials.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("OIDC HTTP client")
    })
}

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("provider: {0}")]
    Provider(String),
    #[error("invalid ID token: {0}")]
    Token(String),
}

async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, OidcError> {
    let res = http()
        .get(url)
        .send()
        .await
        .map_err(|e| OidcError::Provider(e.to_string()))?;
    if !res.status().is_success() {
        return Err(OidcError::Provider(format!("{url}: {}", res.status())));
    }
    res.json()
        .await
        .map_err(|e| OidcError::Provider(format!("{url}: {e}")))
}

type ProviderCache = Mutex<HashMap<String, (Instant, Discovery, JwkSet)>>;

fn cache() -> &'static ProviderCache {
    static CACHE: OnceLock<ProviderCache> = OnceLock::new();
    CACHE.get_or_init(ProviderCache::default)
}

/// Discovery document and keys, cached for an hour per issuer.
async fn provider(issuer: &str) -> Result<(Discovery, JwkSet), OidcError> {
    if let Some((at, d, k)) = cache().lock().expect("cache lock").get(issuer) {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok((d.clone(), k.clone()));
        }
    }
    let d: Discovery = get_json(&format!("{issuer}/.well-known/openid-configuration")).await?;
    if d.issuer.trim_end_matches('/') != issuer {
        return Err(OidcError::Provider(format!(
            "issuer mismatch: discovery says {}",
            d.issuer
        )));
    }
    let keys: JwkSet = get_json(&d.jwks_uri).await?;
    cache()
        .lock()
        .expect("cache lock")
        .insert(issuer.to_owned(), (Instant::now(), d.clone(), keys.clone()));
    Ok((d, keys))
}

/// Forgets cached metadata, e.g. after the provider rotated its keys.
fn forget(issuer: &str) {
    cache().lock().expect("cache lock").remove(issuer);
}

fn redirect_uri(s: &IdentitySettings, meta: &RequestMeta) -> String {
    let base = if s.public_url.is_empty() {
        format!(
            "{}://{}",
            if meta.https { "https" } else { "http" },
            meta.host.as_deref().unwrap_or("localhost")
        )
    } else {
        s.public_url.clone()
    };
    format!("{base}/api/v1/auth/oidc/callback")
}

fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn redirect(location: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(location) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// Back to the login page with an error code the UI translates.
fn fail(code: &str) -> Response {
    redirect(&format!("/login?sso_error={code}"))
}

#[derive(Serialize, ToSchema)]
pub struct OidcInfo {
    pub enabled: bool,
    /// Text of the login button (empty: default text).
    pub label: String,
}

/// Whether single sign-on is offered on the login page.
#[utoipa::path(get, path = "/api/v1/auth/oidc", tag = "auth", responses((status = 200, body = OidcInfo)))]
pub async fn oidc_info(State(state): State<AppState>) -> Response {
    match identity::get(&state.db, TenantId::DEFAULT).await {
        Ok(s) => Json(OidcInfo {
            enabled: s.oidc_enabled,
            label: s.oidc_button_label,
        })
        .into_response(),
        Err(err) => crate::error::ApiError::from(err).into_response(),
    }
}

/// Starts the login at the identity provider (browser redirect).
#[utoipa::path(get, path = "/api/v1/auth/oidc/start", tag = "auth", responses((status = 303)))]
pub async fn oidc_start(State(state): State<AppState>, meta: RequestMeta) -> Response {
    let tenant = TenantId::DEFAULT;
    let Ok(s) = identity::get(&state.db, tenant).await else {
        return fail("unavailable");
    };
    if !s.oidc_enabled {
        return fail("disabled");
    }
    let disc = match provider(&s.oidc_issuer).await {
        Ok((d, _)) => d,
        Err(err) => {
            tracing::warn!(error = %err, "OIDC discovery failed");
            return fail("provider");
        }
    };
    let (Ok(verifier), Ok(nonce)) = (crypto::random_token(32), crypto::random_token(16)) else {
        return fail("unavailable");
    };
    let redirect_to = redirect_uri(&s, &meta);
    let Ok(state_value) =
        identity::begin_oidc(&state.db, tenant, &verifier, &nonce, &redirect_to).await
    else {
        return fail("unavailable");
    };
    let sep = if disc.authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };
    redirect(&format!(
        "{}{sep}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
        disc.authorization_endpoint,
        urlencode(&s.oidc_client_id),
        urlencode(&redirect_to),
        urlencode(&s.oidc_scopes),
        urlencode(&state_value),
        urlencode(&nonce),
        pkce_challenge(&verifier),
    ))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}

const UNKNOWN_KEY: &str = "unknown signing key";

/// Verifies the ID token; returns its claims.
pub fn verify_id_token(
    token: &str,
    keys: &JwkSet,
    issuer: &str,
    client_id: &str,
    nonce: &str,
) -> Result<HashMap<String, Value>, OidcError> {
    let bad = |e: &dyn std::fmt::Display| OidcError::Token(e.to_string());
    let header = jsonwebtoken::decode_header(token).map_err(|e| bad(&e))?;
    let allowed = [
        Algorithm::RS256,
        Algorithm::RS384,
        Algorithm::RS512,
        Algorithm::PS256,
        Algorithm::ES256,
        Algorithm::ES384,
        Algorithm::EdDSA,
    ];
    if !allowed.contains(&header.alg) {
        return Err(OidcError::Token(format!(
            "algorithm {:?} not allowed",
            header.alg
        )));
    }
    let jwk = match header.kid.as_deref() {
        Some(kid) => keys.find(kid),
        None if keys.keys.len() == 1 => keys.keys.first(),
        None => None,
    }
    .ok_or_else(|| OidcError::Token(UNKNOWN_KEY.into()))?;
    let key = DecodingKey::from_jwk(jwk).map_err(|e| bad(&e))?;
    let mut v = Validation::new(header.alg);
    v.set_audience(&[client_id]);
    v.set_issuer(&[issuer, &format!("{issuer}/")]);
    v.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    v.leeway = 60;
    let data =
        jsonwebtoken::decode::<HashMap<String, Value>>(token, &key, &v).map_err(|e| bad(&e))?;
    if data.claims.get("nonce").and_then(Value::as_str) != Some(nonce) {
        return Err(OidcError::Token("nonce mismatch".into()));
    }
    Ok(data.claims)
}

/// Groups from a claim that is a list or a single string.
fn groups(claims: &HashMap<String, Value>, claim: &str) -> Vec<String> {
    match claims.get(claim) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|g| g.as_str().map(str::to_owned))
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// The provider redirects here after the login.
#[utoipa::path(get, path = "/api/v1/auth/oidc/callback", tag = "auth", params(CallbackQuery), responses((status = 303)))]
pub async fn oidc_callback(
    State(state): State<AppState>,
    meta: RequestMeta,
    Query(q): Query<CallbackQuery>,
) -> Response {
    if let Some(err) = &q.error {
        tracing::info!(error = %err.chars().take(64).collect::<String>(), "OIDC login aborted by provider");
        return fail("denied");
    }
    let (Some(code), Some(state_value)) = (q.code, q.state) else {
        return fail("invalid");
    };
    let Ok(Some(pending)) = identity::take_oidc(&state.db, &state_value).await else {
        return fail("expired");
    };
    let tenant = pending.tenant_id;
    let Ok(s) = identity::get(&state.db, tenant).await else {
        return fail("unavailable");
    };
    if !s.oidc_enabled {
        return fail("disabled");
    }
    let secret = identity::secrets(&state.db, tenant, &state.secrets)
        .await
        .ok()
        .and_then(|(oidc, _)| oidc);
    let claims = match exchange(&s, secret.as_deref(), &code, &pending).await {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(error = %err, "OIDC login failed");
            return fail("provider");
        }
    };
    let Some(sub) = claims.get("sub").and_then(Value::as_str) else {
        return fail("invalid");
    };
    let text = |k: &str| claims.get(k).and_then(Value::as_str).map(str::to_owned);
    let username = text(&s.oidc_username_claim)
        .or_else(|| text("email"))
        .unwrap_or_else(|| sub.to_owned());
    let groups = groups(&claims, &s.oidc_groups_claim);
    let Some(role) = identity::map_role(&s, &groups) else {
        tracing::info!(%username, "OIDC login denied: not in an allowed group");
        return fail("forbidden");
    };
    let account = ExternalAccount {
        source: "oidc",
        external_id: format!("{}|{sub}", s.oidc_issuer),
        username: username.clone(),
        display_name: text("name").unwrap_or_else(|| username.clone()),
        email: text("email"),
        role,
    };
    let user = match identity::upsert_user(&state.db, tenant, &account).await {
        Ok(u) => u,
        Err(err) => {
            tracing::warn!(error = %err, "OIDC user could not be created");
            return fail("conflict");
        }
    };
    if !user.enabled {
        return fail("forbidden");
    }
    let actor = audit::Actor {
        tenant,
        user_id: Some(user.id),
        ip: meta.ip.clone(),
    };
    let _ = audit::record(
        &state.db,
        &actor,
        "login",
        "user",
        Some(user.id.to_string()),
        serde_json::json!({"source": "oidc", "role": user.role}),
    )
    .await;
    match new_session(&state, user, &meta).await {
        Ok((cookie, _)) => {
            let mut res = redirect("/");
            res.headers_mut().insert(header::SET_COOKIE, cookie);
            res
        }
        Err(_) => fail("unavailable"),
    }
}

async fn exchange(
    s: &IdentitySettings,
    secret: Option<&str>,
    code: &str,
    pending: &identity::PendingLogin,
) -> Result<HashMap<String, Value>, OidcError> {
    let (disc, keys) = provider(&s.oidc_issuer).await?;
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", pending.redirect_uri.as_str()),
        ("client_id", s.oidc_client_id.as_str()),
        ("code_verifier", pending.verifier.as_str()),
    ];
    // client_secret_post: accepted by Keycloak, Authentik, Entra ID, Google,
    // Zitadel, Authelia; public clients send no secret (PKCE only).
    if let Some(secret) = secret {
        form.push(("client_secret", secret));
    }
    let res = http()
        .post(&disc.token_endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|e| OidcError::Provider(e.to_string()))?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(OidcError::Provider(format!(
            "token endpoint {status}: {}",
            body.chars().take(200).collect::<String>()
        )));
    }
    let token: TokenResponse = res
        .json()
        .await
        .map_err(|e| OidcError::Provider(e.to_string()))?;
    let verify = |keys: &JwkSet| {
        verify_id_token(
            &token.id_token,
            keys,
            &s.oidc_issuer,
            &s.oidc_client_id,
            &pending.nonce,
        )
    };
    match verify(&keys) {
        Err(OidcError::Token(m)) if m == UNKNOWN_KEY => {
            // The provider may have rotated its keys: fetch them once more.
            forget(&s.oidc_issuer);
            let (_, keys) = provider(&s.oidc_issuer).await?;
            verify(&keys)
        }
        other => other,
    }
}
