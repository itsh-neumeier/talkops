//! Keeps a single ESL connection to FreeSWITCH alive and shares it.

use std::sync::Arc;
use std::time::Duration;

use talkops_esl::EslClient;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct EslHandle(Arc<RwLock<Option<Arc<EslClient>>>>);

impl EslHandle {
    /// The current client, if connected.
    pub async fn get(&self) -> Option<Arc<EslClient>> {
        let guard = self.0.read().await;
        guard.as_ref().filter(|c| c.is_connected()).cloned()
    }

    /// Spawns the reconnect loop. FreeSWITCH may start after TalkOps (it
    /// fetches its configuration from us), so connection failures are normal
    /// during startup and retried with capped backoff.
    pub fn spawn_supervisor(&self, addr: String, password: String) {
        let handle = self.clone();
        tokio::spawn(async move {
            let mut delay = Duration::from_secs(1);
            loop {
                if handle.get().await.is_none() {
                    match EslClient::connect(&addr, &password, Duration::from_secs(5)).await {
                        Ok(client) => {
                            tracing::info!(%addr, "connected to FreeSWITCH event socket");
                            *handle.0.write().await = Some(Arc::new(client));
                            delay = Duration::from_secs(1);
                        }
                        Err(err) => {
                            tracing::warn!(%addr, error = %err, retry_in = ?delay, "FreeSWITCH event socket unavailable");
                            delay = (delay * 2).min(Duration::from_secs(30));
                        }
                    }
                }
                tokio::time::sleep(delay).await;
            }
        });
    }
}
