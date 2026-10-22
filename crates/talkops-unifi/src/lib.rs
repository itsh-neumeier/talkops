//! Read-only client for the UniFi Network application (ADR 0041).
//!
//! The Wi-Fi survey reads the state of one handset each second: connected
//! access point, radio, channel, signal and data rates. Sources:
//!
//! - Integration API (`/proxy/network/integration/v1/...`): version, sites
//!   and access point names. Official, with API keys.
//! - Classic API (`/proxy/network/api/s/<site>/...`): `stat/sta/<mac>` for
//!   the radio values, which the Integration API does not publish.
//!   `stat/device-basic` for access point names on older versions.
//!
//! This crate sends only GET requests. It never changes the UniFi
//! configuration. The API key goes only into the `X-API-KEY` header; it is
//! not part of any URL, log line or error.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;

/// Largest answer TalkOps reads.
const MAX_BODY: usize = 4 * 1024 * 1024;
/// Timeout of one request; the survey polls each second.
const TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, thiserror::Error)]
pub enum UnifiError {
    #[error("UniFi refused the API key")]
    Unauthorized,
    #[error("the UniFi endpoint does not exist")]
    NotFound,
    #[error("the certificate of UniFi does not match the pinned fingerprint")]
    CertificateMismatch,
    #[error("UniFi is not reachable: {0}")]
    Http(String),
    #[error("unexpected answer from UniFi")]
    Invalid,
}

impl UnifiError {
    /// Short class for stored samples (no details, no secrets).
    pub fn class(&self) -> &'static str {
        match self {
            UnifiError::Unauthorized => "unauthorized",
            UnifiError::NotFound => "not_found",
            UnifiError::CertificateMismatch => "certificate",
            UnifiError::Http(_) => "unreachable",
            UnifiError::Invalid => "invalid",
        }
    }
}

/// Connection data.
#[derive(Clone)]
pub struct Config {
    /// Console address without a trailing slash, e.g. `https://192.168.1.1`.
    pub url: String,
    /// Classic site name (`default`).
    pub site: String,
    pub api_key: String,
    /// Pinned certificate (SHA-256, hex); empty for a public CA.
    pub cert_sha256: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("url", &self.url)
            .field("site", &self.site)
            .finish_non_exhaustive()
    }
}

/// The Wi-Fi state of one client.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Station {
    pub mac: String,
    /// MAC of the access point (lowercase, with colons).
    pub ap_mac: Option<String>,
    pub bssid: Option<String>,
    pub essid: Option<String>,
    /// `ng` (2.4 GHz), `na` (5 GHz), `6e` (6 GHz).
    pub radio: Option<String>,
    pub channel: Option<i32>,
    /// Signal in dBm.
    pub rssi: Option<i32>,
    pub tx_rate_kbps: Option<i32>,
    pub rx_rate_kbps: Option<i32>,
}

/// Result of a connection test.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Network application version (Integration API `info`).
    pub version: Option<String>,
    /// Integration API answered.
    pub integration: bool,
    /// Classic `stat/sta` answered (radio values available).
    pub classic: bool,
    /// Site ID of the Integration API for the classic site name.
    pub site_id: Option<String>,
    /// Number of access points found.
    pub access_points: usize,
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

/// Accepts exactly one certificate (`expected`), or records the certificate
/// without a check (`expected = None`, only for the probe without a key).
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

fn ensure_tls_provider() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// A client for the public CAs, or for one pinned certificate.
fn http_client(
    pin: Option<Option<String>>,
    seen: Arc<Mutex<Option<String>>>,
) -> Result<reqwest::Client, UnifiError> {
    ensure_tls_provider();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| UnifiError::Http(e.to_string()))?;
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
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("TalkOps")
        .build()
        .map_err(|e| UnifiError::Http(e.without_url().to_string()))
}

/// Normalizes a MAC address to lowercase with colons (`aa:bb:cc:dd:ee:ff`).
pub fn colon_mac(mac: &str) -> Option<String> {
    let hex: String = mac
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let separators_ok = mac
        .chars()
        .all(|c| c.is_ascii_hexdigit() || ":-.".contains(c));
    if hex.len() != 12 || !separators_ok {
        return None;
    }
    Some(
        hex.as_bytes()
            .chunks(2)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect::<Vec<_>>()
            .join(":"),
    )
}

/// Read-only UniFi client.
pub struct Client {
    http: reqwest::Client,
    seen: Arc<Mutex<Option<String>>>,
    cfg: Config,
}

impl Client {
    pub fn new(cfg: Config) -> Result<Self, UnifiError> {
        let seen = Arc::new(Mutex::new(None));
        let pin = (!cfg.cert_sha256.is_empty()).then(|| Some(cfg.cert_sha256.clone()));
        Ok(Self {
            http: http_client(pin, seen.clone())?,
            seen,
            cfg,
        })
    }

    /// The only request function of this crate: GET with the API key.
    async fn get(&self, path: &str) -> Result<Value, UnifiError> {
        let res = self
            .http
            .get(format!("{}{path}", self.cfg.url))
            .header("X-API-KEY", &self.cfg.api_key)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                let mismatch = !self.cfg.cert_sha256.is_empty()
                    && self
                        .seen
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_deref()
                        .is_some_and(|s| s != self.cfg.cert_sha256);
                if mismatch {
                    UnifiError::CertificateMismatch
                } else {
                    UnifiError::Http(e.without_url().to_string())
                }
            })?;
        match res.status().as_u16() {
            200 => {}
            401 | 403 => return Err(UnifiError::Unauthorized),
            404 => return Err(UnifiError::NotFound),
            // The classic API answers 400 for an unknown client MAC.
            400 if path.contains("/stat/sta/") => return Err(UnifiError::NotFound),
            code => return Err(UnifiError::Http(format!("HTTP status {code}"))),
        }
        let body = res
            .bytes()
            .await
            .map_err(|e| UnifiError::Http(e.without_url().to_string()))?;
        if body.len() > MAX_BODY {
            return Err(UnifiError::Invalid);
        }
        serde_json::from_slice(&body).map_err(|_| UnifiError::Invalid)
    }

    fn classic(&self, rest: &str) -> String {
        format!("/proxy/network/api/s/{}/{rest}", self.cfg.site)
    }

    /// Network application version (Integration API).
    pub async fn version(&self) -> Result<Option<String>, UnifiError> {
        let v = self.get("/proxy/network/integration/v1/info").await?;
        Ok(v.get("applicationVersion")
            .and_then(Value::as_str)
            .map(str::to_owned))
    }

    /// Integration API site ID for the configured classic site name.
    pub async fn site_id(&self) -> Result<Option<String>, UnifiError> {
        let v = self
            .get("/proxy/network/integration/v1/sites?limit=200")
            .await?;
        let sites = v
            .get("data")
            .and_then(Value::as_array)
            .ok_or(UnifiError::Invalid)?;
        Ok(sites
            .iter()
            .find(|s| s.get("internalReference").and_then(Value::as_str) == Some(&self.cfg.site))
            .or_else(|| (sites.len() == 1).then(|| &sites[0]))
            .and_then(|s| s.get("id").and_then(Value::as_str))
            .map(str::to_owned))
    }

    /// Access point names by MAC (lowercase with colons).
    pub async fn ap_names(
        &self,
        site_id: Option<&str>,
    ) -> Result<HashMap<String, String>, UnifiError> {
        let mut names = HashMap::new();
        let classic = self.get(&self.classic("stat/device-basic")).await;
        match classic {
            Ok(v) => {
                for d in v
                    .get("data")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let (Some(mac), Some(name)) = (
                        d.get("mac").and_then(Value::as_str).and_then(colon_mac),
                        d.get("name").and_then(Value::as_str),
                    ) {
                        names.insert(mac, name.to_owned());
                    }
                }
                Ok(names)
            }
            Err(e) => {
                let Some(site) = site_id else {
                    return Err(e);
                };
                let v = self
                    .get(&format!(
                        "/proxy/network/integration/v1/sites/{site}/devices?limit=200"
                    ))
                    .await?;
                for d in v
                    .get("data")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let (Some(mac), Some(name)) = (
                        d.get("macAddress")
                            .and_then(Value::as_str)
                            .and_then(colon_mac),
                        d.get("name").and_then(Value::as_str),
                    ) {
                        names.insert(mac, name.to_owned());
                    }
                }
                Ok(names)
            }
        }
    }

    /// Wi-Fi state of a client; `None` if the client is not connected.
    pub async fn station(&self, mac: &str) -> Result<Option<Station>, UnifiError> {
        let mac = colon_mac(mac).ok_or(UnifiError::Invalid)?;
        let v = match self.get(&self.classic(&format!("stat/sta/{mac}"))).await {
            Ok(v) => v,
            Err(UnifiError::NotFound) => return Ok(None),
            Err(e) => return Err(e),
        };
        Ok(parse_station(&v, &mac))
    }

    /// Tests the connection: version, site and the classic client endpoint.
    /// `probe_mac` is a client MAC to read, if known.
    pub async fn probe(&self, probe_mac: Option<&str>) -> Result<Capabilities, UnifiError> {
        let mut caps = Capabilities::default();
        match self.version().await {
            Ok(v) => {
                caps.version = v;
                caps.integration = true;
            }
            // Wrong key or certificate: stop here with a clear error.
            Err(e @ (UnifiError::Unauthorized | UnifiError::CertificateMismatch)) => {
                return Err(e);
            }
            Err(e) => tracing::debug!(error = %e, "UniFi Integration API not available"),
        }
        if caps.integration {
            caps.site_id = self.site_id().await.unwrap_or(None);
        }
        // The classic API answers for any site path; a list of clients
        // shows that the key works there too.
        match self.get(&self.classic("stat/sta")).await {
            Ok(v) if v.get("data").is_some_and(Value::is_array) => caps.classic = true,
            Ok(_) => {}
            Err(e @ UnifiError::CertificateMismatch) => return Err(e),
            Err(e) => tracing::debug!(error = %e, "UniFi classic API not available"),
        }
        if caps.classic {
            if let Some(mac) = probe_mac {
                let _ = self.station(mac).await;
            }
        }
        caps.access_points = self
            .ap_names(caps.site_id.as_deref())
            .await
            .map(|n| n.len())
            .unwrap_or(0);
        if !caps.integration && !caps.classic {
            return Err(UnifiError::NotFound);
        }
        Ok(caps)
    }
}

/// Reads the client from a classic `stat/sta` answer.
pub fn parse_station(v: &Value, mac: &str) -> Option<Station> {
    let data = v.get("data")?.as_array()?;
    let s = data.iter().find(|s| {
        s.get("mac")
            .and_then(Value::as_str)
            .and_then(colon_mac)
            .as_deref()
            == Some(mac)
    })?;
    let text = |k: &str| s.get(k).and_then(Value::as_str).map(str::to_owned);
    let int = |k: &str| {
        s.get(k)
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok())
    };
    // `signal` is dBm. Older versions give only `rssi`; a negative `rssi`
    // is dBm too (a positive one is signal over noise).
    let rssi = int("signal").or_else(|| int("rssi").filter(|r| *r < 0));
    let ap_mac = text("ap_mac").as_deref().and_then(colon_mac);
    if ap_mac.is_none() && s.get("is_wired").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    Some(Station {
        mac: mac.to_owned(),
        ap_mac,
        bssid: text("bssid").as_deref().and_then(colon_mac),
        essid: text("essid"),
        radio: text("radio"),
        channel: int("channel"),
        rssi,
        tx_rate_kbps: int("tx_rate"),
        rx_rate_kbps: int("rx_rate"),
    })
}

/// The SHA-256 fingerprint of the certificate at `url` (hex). Sends no key.
pub async fn probe_certificate(url: &str) -> Result<String, UnifiError> {
    let seen = Arc::new(Mutex::new(None));
    // Any HTTP status is fine: only the TLS handshake matters.
    let _ = http_client(Some(None), seen.clone())?
        .get(format!("{url}/"))
        .send()
        .await
        .map_err(|e| UnifiError::Http(e.without_url().to_string()))?;
    let hash = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    hash.ok_or_else(|| UnifiError::Http("no certificate received".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;

    #[test]
    fn this_crate_only_reads() {
        // ADR 0041: no write access to UniFi. Each request goes through
        // `Client::get`; no other HTTP method appears in the code.
        let src = include_str!("lib.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap()];
        for forbidden in [
            ".post(",
            ".put(",
            ".patch(",
            ".delete(",
            ".request(",
            "Method::",
        ] {
            assert!(!code.contains(forbidden), "{forbidden}");
        }
        assert_eq!(
            code.matches(".get(format!(").count(),
            2,
            "one request function and the probe"
        );
    }

    #[test]
    fn macs_and_stations() {
        assert_eq!(
            colon_mac("AABBCCDDEEFF").as_deref(),
            Some("aa:bb:cc:dd:ee:ff")
        );
        assert_eq!(
            colon_mac("aa-bb-cc-dd-ee-ff").as_deref(),
            Some("aa:bb:cc:dd:ee:ff")
        );
        assert_eq!(colon_mac("aa:bb:cc:dd:ee"), None);
        assert_eq!(colon_mac("aa:bb:cc:dd:ee:fg"), None);
        let v = json!({"meta": {"rc": "ok"}, "data": [{
            "mac": "80:5e:c0:00:00:01", "ap_mac": "F4:92:BF:00:00:02", "bssid": "f4:92:bf:00:00:03",
            "essid": "Voice", "radio": "na", "channel": 36, "rssi": 45, "signal": -51,
            "tx_rate": 144444, "rx_rate": 130000
        }]});
        let s = parse_station(&v, "80:5e:c0:00:00:01").unwrap();
        assert_eq!(s.ap_mac.as_deref(), Some("f4:92:bf:00:00:02"));
        assert_eq!(s.rssi, Some(-51));
        assert_eq!(s.channel, Some(36));
        assert_eq!(s.tx_rate_kbps, Some(144_444));
        // Only a positive `rssi`: no dBm value.
        let old = json!({"data": [{"mac": "80:5e:c0:00:00:01", "ap_mac": "f4:92:bf:00:00:02", "rssi": 40}]});
        assert_eq!(parse_station(&old, "80:5e:c0:00:00:01").unwrap().rssi, None);
        assert!(parse_station(&v, "80:5e:c0:00:00:09").is_none());
    }

    #[test]
    fn config_debug_hides_the_key() {
        let cfg = Config {
            url: "https://u".into(),
            site: "default".into(),
            api_key: "top-secret".into(),
            cert_sha256: String::new(),
        };
        assert!(!format!("{cfg:?}").contains("top-secret"));
    }

    fn authorized(h: &HeaderMap) -> bool {
        h.get("x-api-key").and_then(|v| v.to_str().ok()) == Some("k1")
    }

    /// A UniFi console with the Integration API and the classic API.
    fn console(integration: bool) -> Router {
        Router::new()
            .route(
                "/proxy/network/integration/v1/info",
                get(move |h: HeaderMap| async move {
                    if !integration {
                        return Err(StatusCode::NOT_FOUND);
                    }
                    if !authorized(&h) {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                    Ok(Json(json!({"applicationVersion": "9.4.19"})))
                }),
            )
            .route(
                "/proxy/network/integration/v1/sites",
                get(|| async {
                    Json(json!({"data": [{"id": "88f7", "internalReference": "default", "name": "Default"}]}))
                }),
            )
            .route(
                "/proxy/network/api/s/default/stat/sta",
                get(|h: HeaderMap| async move {
                    if !authorized(&h) {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                    Ok(Json(json!({"meta": {"rc": "ok"}, "data": []})))
                }),
            )
            .route(
                "/proxy/network/api/s/default/stat/sta/{mac}",
                get(|Path(mac): Path<String>| async move {
                    if mac != "80:5e:c0:00:00:01" {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                    Ok(Json(json!({"data": [{"mac": mac, "ap_mac": "f4:92:bf:00:00:02",
                        "radio": "na", "channel": 44, "signal": -63, "tx_rate": 86000}]})))
                }),
            )
            .route(
                "/proxy/network/api/s/default/stat/device-basic",
                get(|| async {
                    Json(json!({"data": [{"mac": "f4:92:bf:00:00:02", "name": "Hall", "type": "uap"}]}))
                }),
            )
    }

    async fn serve(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}")
    }

    fn client(url: String, key: &str) -> Client {
        Client::new(Config {
            url,
            site: "default".into(),
            api_key: key.into(),
            cert_sha256: String::new(),
        })
        .unwrap()
    }

    #[tokio::test]
    async fn probes_and_reads_a_station() {
        let url = serve(console(true)).await;
        let c = client(url.clone(), "k1");
        let caps = c.probe(Some("805ec0000001")).await.unwrap();
        assert_eq!(caps.version.as_deref(), Some("9.4.19"));
        assert!(caps.integration && caps.classic);
        assert_eq!(caps.site_id.as_deref(), Some("88f7"));
        assert_eq!(caps.access_points, 1);
        let s = c.station("80-5E-C0-00-00-01").await.unwrap().unwrap();
        assert_eq!(s.rssi, Some(-63));
        assert_eq!(s.channel, Some(44));
        assert_eq!(
            c.ap_names(None)
                .await
                .unwrap()
                .get("f4:92:bf:00:00:02")
                .map(String::as_str),
            Some("Hall")
        );
        // An unknown client is not connected, not an error.
        assert_eq!(c.station("805ec0000009").await.unwrap(), None);

        assert!(matches!(
            client(url, "wrong").probe(None).await,
            Err(UnifiError::Unauthorized)
        ));
    }

    #[tokio::test]
    async fn works_without_the_integration_api() {
        let url = serve(console(false)).await;
        let caps = client(url, "k1").probe(None).await.unwrap();
        assert!(!caps.integration && caps.classic);
        assert_eq!(caps.version, None);
    }

    #[tokio::test]
    async fn unreachable_console() {
        let c = client("http://127.0.0.1:1".into(), "k1");
        let err = c.station("805ec0000001").await.unwrap_err();
        assert_eq!(err.class(), "unreachable");
        assert!(!err.to_string().contains("k1"));
    }
}
