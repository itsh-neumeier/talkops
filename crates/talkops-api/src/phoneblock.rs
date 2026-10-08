//! PhoneBlock (phoneblock.net) number lookup: `GET /api/num/{+E164}` with
//! the user's API key (`pbt_…`, created under Settings → API keys on
//! phoneblock.net) as Bearer token. Per-call lookups are what PhoneBlock
//! recommends for live call screening (INTEGRATIONS.md, "hybrid approach").

use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;

pub const BASE_URL: &str = "https://phoneblock.net/phoneblock/api";

#[derive(Debug, thiserror::Error)]
pub enum PhoneBlockError {
    #[error("PhoneBlock request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("PhoneBlock rejected the API key")]
    Unauthorized,
    #[error("PhoneBlock answered {0}")]
    Status(u16),
}

/// The fields of PhoneBlock's `PhoneInfo` that decide a block.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhoneInfo {
    #[serde(default)]
    votes: u32,
    /// The account owner blocked the number personally.
    #[serde(default)]
    black_listed: bool,
    /// On PhoneBlock's global whitelist (e.g. emergency or authorities).
    #[serde(default)]
    white_listed: bool,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        talkops_doorbell::ensure_tls_provider();
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
            .timeout(Duration::from_secs(3))
            .user_agent(format!("TalkOps/{}", talkops_core::VERSION))
            .build()
            .expect("HTTP client")
    })
}

/// Spam reports for `number` (E.164). A personal block counts as
/// `u32::MAX`, a whitelisted number as 0.
pub async fn votes(base_url: &str, token: &str, number: &str) -> Result<u32, PhoneBlockError> {
    let res = client()
        .get(format!("{base_url}/num/{number}"))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await?;
    match res.status().as_u16() {
        200 => {}
        401 | 403 => return Err(PhoneBlockError::Unauthorized),
        other => return Err(PhoneBlockError::Status(other)),
    }
    let info: PhoneInfo = res.json().await?;
    Ok(if info.black_listed {
        u32::MAX
    } else if info.white_listed {
        0
    } else {
        info.votes
    })
}
