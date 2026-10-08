//! Decides whether an inbound call is blocked: anonymous callers, the
//! tenant's own list and the PhoneBlock community list (phoneblock.net).
//!
//! PhoneBlock is asked per call with a short timeout; answers are cached so
//! repeated calls do not query it again, and an unreachable service never
//! blocks a call.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use talkops_core::blocking;
use talkops_core::crypto::SecretBox;
use talkops_core::error::CoreResult;
use talkops_core::tenant::TenantId;

/// How long a PhoneBlock answer is reused.
const CACHE_TTL: Duration = Duration::from_secs(6 * 3600);
const CACHE_MAX: usize = 10_000;
/// A call waits at most this long for PhoneBlock.
const LOOKUP_TIMEOUT: Duration = Duration::from_millis(1500);

/// Why a call was blocked (logged and shown in the call list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    Anonymous,
    /// Own list, with the matching pattern.
    List(String),
    /// PhoneBlock, with the number of spam reports.
    PhoneBlock(u32),
}

impl Blocked {
    pub fn reason(&self) -> String {
        match self {
            Blocked::Anonymous => "anonymous".into(),
            Blocked::List(p) => format!("list:{p}"),
            Blocked::PhoneBlock(votes) => format!("phoneblock:{votes}"),
        }
    }
}

pub struct SpamCheck {
    secrets: SecretBox,
    /// Base URL of the PhoneBlock API (overridable for tests).
    base_url: String,
    cache: Mutex<HashMap<String, (Instant, u32)>>,
}

impl SpamCheck {
    pub fn new(secrets: SecretBox) -> Self {
        Self::with_base_url(secrets, crate::phoneblock::BASE_URL)
    }

    pub fn with_base_url(secrets: SecretBox, base_url: &str) -> Self {
        Self {
            secrets,
            base_url: base_url.trim_end_matches('/').to_owned(),
            cache: Mutex::default(),
        }
    }

    /// `caller` is the E.164 number, `None` for anonymous calls.
    pub async fn check(
        &self,
        db: &PgPool,
        tenant: TenantId,
        caller: Option<&str>,
    ) -> CoreResult<Option<Blocked>> {
        let settings = blocking::effective(db, tenant, &self.secrets).await?;
        let Some(caller) = caller else {
            return Ok(settings.block_anonymous.then_some(Blocked::Anonymous));
        };
        if let Some(entry) = blocking::find(db, tenant, caller).await? {
            return Ok(Some(Blocked::List(entry.pattern)));
        }
        let Some(token) = settings.phoneblock_token else {
            return Ok(None);
        };
        let min = u32::try_from(settings.phoneblock_min_votes).unwrap_or(4);
        let votes = match self.cached(caller) {
            Some(v) => v,
            None => {
                let lookup = crate::phoneblock::votes(&self.base_url, &token, caller);
                match tokio::time::timeout(LOOKUP_TIMEOUT, lookup).await {
                    Ok(Ok(v)) => {
                        self.remember(caller, v);
                        v
                    }
                    Ok(Err(err)) => {
                        tracing::warn!(error = %err, "PhoneBlock lookup failed");
                        return Ok(None);
                    }
                    Err(_) => {
                        tracing::warn!("PhoneBlock lookup timed out");
                        return Ok(None);
                    }
                }
            }
        };
        Ok((votes >= min).then_some(Blocked::PhoneBlock(votes)))
    }

    fn cached(&self, caller: &str) -> Option<u32> {
        let cache = self.cache.lock().expect("spam cache poisoned");
        cache
            .get(caller)
            .filter(|(at, _)| at.elapsed() < CACHE_TTL)
            .map(|(_, v)| *v)
    }

    fn remember(&self, caller: &str, votes: u32) {
        let mut cache = self.cache.lock().expect("spam cache poisoned");
        if cache.len() >= CACHE_MAX {
            cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
            if cache.len() >= CACHE_MAX {
                cache.clear();
            }
        }
        cache.insert(caller.to_owned(), (Instant::now(), votes));
    }
}
