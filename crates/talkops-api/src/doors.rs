//! Door stations at runtime: rings (event + snapshot), opening the door,
//! the per-station event listener and webhooks (Home Assistant).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::doors::{self, DoorEvent, DoorStation};
use talkops_core::tenant::TenantId;
use talkops_doorbell::{Config, Event, Vto};
use uuid::Uuid;

use crate::{AppState, MediaPaths};

/// What door station handling needs from the application state.
#[derive(Clone)]
pub struct DoorCtx {
    pub db: PgPool,
    pub secrets: SecretBox,
    pub media: Arc<MediaPaths>,
}

impl From<&AppState> for DoorCtx {
    fn from(s: &AppState) -> Self {
        Self {
            db: s.db.clone(),
            secrets: s.secrets.clone(),
            media: s.media.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DoorError {
    #[error("the door station has no HTTP access configured")]
    NoApi,
    #[error("the door station has only {0} door(s)")]
    NoSuchDoor(i16),
    #[error(transparent)]
    Device(#[from] talkops_doorbell::Error),
    #[error(transparent)]
    Core(#[from] talkops_core::error::CoreError),
}

impl From<DoorError> for crate::error::ApiError {
    fn from(err: DoorError) -> Self {
        use crate::error::ApiError;
        match err {
            DoorError::NoApi | DoorError::NoSuchDoor(_) => ApiError::BadRequest(err.to_string()),
            DoorError::Device(e) => ApiError::Device(e.to_string()),
            DoorError::Core(e) => e.into(),
        }
    }
}

pub use talkops_doorbell::ensure_tls_provider;

impl DoorCtx {
    async fn vto(&self, station: Uuid) -> Result<Vto, DoorError> {
        let conn = doors::connection(&self.db, &self.secrets, station)
            .await?
            .ok_or(DoorError::NoApi)?;
        Ok(Vto::new(Config {
            host: conn.host,
            port: conn.port,
            username: conn.username,
            password: conn.password,
        })?)
    }

    /// Opens door `door` (1 or 2); `by` says who asked (logged).
    pub async fn open(
        &self,
        tenant: TenantId,
        station: &DoorStation,
        door: i16,
        by: Value,
    ) -> Result<(), DoorError> {
        if !(1..=station.doors).contains(&door) {
            return Err(DoorError::NoSuchDoor(station.doors));
        }
        let result = match self.vto(station.id).await {
            Ok(vto) => vto.open_door(door as u8).await.map_err(DoorError::from),
            Err(e) => Err(e),
        };
        let mut detail = json!({ "door": door, "by": by, "ok": result.is_ok() });
        if let Err(err) = &result {
            detail["error"] = json!(err.to_string());
            tracing::warn!(station = %station.name, error = %err, "opening the door failed");
        } else {
            tracing::info!(station = %station.name, door, "door opened");
        }
        self.record(tenant, station, "open_command", detail).await;
        result
    }

    /// A ring was routed: log it with a snapshot and tell the webhook.
    pub async fn ring(&self, tenant: TenantId, station: Uuid, dialed: &str) {
        let Ok(station) = doors::get(&self.db, tenant, station).await else {
            return;
        };
        let event = match doors::add_event(
            &self.db,
            tenant,
            station.id,
            "ring",
            json!({ "dialed": dialed }),
        )
        .await
        {
            Ok(e) => e,
            Err(err) => {
                tracing::error!(error = %err, "cannot store door event");
                return;
            }
        };
        let event = if station.snapshots && station.has_api() {
            self.attach_snapshot(tenant, &station, event).await
        } else {
            event
        };
        self.notify(&station, &event).await;
    }

    async fn attach_snapshot(
        &self,
        tenant: TenantId,
        station: &DoorStation,
        mut event: DoorEvent,
    ) -> DoorEvent {
        let jpeg = match self.vto(station.id).await {
            Ok(vto) => vto.snapshot().await.map_err(DoorError::from),
            Err(e) => Err(e),
        };
        let jpeg = match jpeg {
            Ok(j) => j,
            Err(err) => {
                tracing::warn!(station = %station.name, error = %err, "snapshot failed");
                return event;
            }
        };
        let file = doors::snapshot_file(tenant, event.created_at, event.id);
        let path = self.media.snapshots.join(&file);
        let written = async {
            if let Some(dir) = path.parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            tokio::fs::write(&path, &jpeg).await
        }
        .await;
        match written {
            Ok(()) => {
                if doors::set_snapshot(&self.db, event.id, &file).await.is_ok() {
                    event.snapshot = Some(file);
                    event.has_snapshot = true;
                }
            }
            Err(err) => tracing::warn!(error = %err, "cannot store snapshot"),
        }
        event
    }

    /// Current picture of a station (live view).
    pub async fn snapshot(&self, station: Uuid) -> Result<bytes::Bytes, DoorError> {
        Ok(self.vto(station).await?.snapshot().await?)
    }

    /// Tests the HTTP access; returns the device type.
    pub async fn test(&self, station: Uuid) -> Result<String, DoorError> {
        Ok(self.vto(station).await?.device_type().await?)
    }

    /// Stores an event and tells the webhook.
    pub async fn record(&self, tenant: TenantId, station: &DoorStation, kind: &str, detail: Value) {
        match doors::add_event(&self.db, tenant, station.id, kind, detail).await {
            Ok(event) => self.notify(station, &event).await,
            Err(err) => tracing::error!(error = %err, "cannot store door event"),
        }
    }

    /// Posts the event to the station's webhook (best effort, in the background).
    async fn notify(&self, station: &DoorStation, event: &DoorEvent) {
        if !station.has_webhook {
            return;
        }
        let url = match doors::webhook_url(&self.db, &self.secrets, station.id).await {
            Ok(Some(url)) => url,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!(error = %err, "cannot read webhook");
                return;
            }
        };
        let body = webhook_body(station, event);
        tokio::spawn(async move {
            for attempt in 0..3u32 {
                match webhook_client().post(&url).json(&body).send().await {
                    Ok(res) if res.status().is_success() => return,
                    Ok(res) => tracing::warn!(status = %res.status(), "webhook rejected"),
                    Err(err) => tracing::warn!(error = %err, "webhook failed"),
                }
                tokio::time::sleep(Duration::from_secs(2u64 << attempt)).await;
            }
        });
    }
}

/// JSON sent to webhooks (stable format, documented in the user guide).
pub fn webhook_body(station: &DoorStation, event: &DoorEvent) -> Value {
    json!({
        "event": event.kind,
        "event_id": event.id,
        "door_station": { "id": station.id, "name": station.name },
        "detail": event.detail,
        "has_snapshot": event.has_snapshot,
        "at": event.created_at,
    })
}

fn webhook_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        ensure_tls_provider();
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .timeout(Duration::from_secs(10))
            .build()
            .expect("webhook HTTP client")
    })
}

/// Maps a device event to a door event (`None`: not logged).
pub fn classify(event: &Event) -> Option<(&'static str, Value)> {
    let d = &event.data;
    match event.code.as_str() {
        "AccessControl" => Some((
            "opened",
            json!({ "method": d["Method"], "user": d["UserID"], "status": d["Status"] }),
        )),
        "DoorStatus" => match d["Status"].as_str() {
            Some("Open") => Some(("door_open", json!({ "index": event.index }))),
            Some("Close") => Some(("door_closed", json!({ "index": event.index }))),
            _ => None,
        },
        // Unlock failed (BackKeyLight state 9).
        "BackKeyLight" if d["State"].as_i64() == Some(9) => Some(("unlock_failed", json!({}))),
        "AlarmLocal" | "ProfileAlarmTransmit" | "Tamper" | "ChassisIntruded"
            if event.action != "Stop" =>
        {
            Some(("alarm", json!({ "code": event.code, "data": d })))
        }
        _ => None,
    }
}

/// Follows one station's event stream until it ends or fails.
async fn listen(ctx: &DoorCtx, tenant: TenantId, station: &DoorStation) -> Result<(), DoorError> {
    let vto = ctx.vto(station.id).await?;
    let model = vto.device_type().await.ok();
    let mut stream = vto.events(10).await?;
    if doors::set_online(&ctx.db, station.id, true, model.as_deref()).await? {
        ctx.record(tenant, station, "online", json!({ "model": model }))
            .await;
    }
    while let Some(events) = stream.next().await? {
        for event in &events {
            if let Some((kind, detail)) = classify(event) {
                ctx.record(tenant, station, kind, detail).await;
            }
        }
    }
    Ok(())
}

/// Keeps one listener per enabled station with an HTTP API; picks up
/// changes every 30 seconds.
pub fn spawn_listeners(ctx: DoorCtx) {
    tokio::spawn(async move {
        let mut running: HashMap<Uuid, (String, tokio::task::JoinHandle<()>)> = HashMap::new();
        loop {
            let wanted: Vec<(TenantId, DoorStation)> = doors::with_api(&ctx.db)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|(_, s)| s.events_enabled)
                .collect();
            // Restart a listener when its connection settings change.
            let key = |s: &DoorStation| format!("{}:{}:{}", s.host, s.port, s.username);
            running.retain(|id, (k, handle)| {
                let keep = wanted.iter().any(|(_, s)| s.id == *id && key(s) == *k)
                    && !handle.is_finished();
                if !keep {
                    handle.abort();
                }
                keep
            });
            for (tenant, station) in wanted {
                if running.contains_key(&station.id) {
                    continue;
                }
                let ctx2 = ctx.clone();
                let k = key(&station);
                let id = station.id;
                let handle = tokio::spawn(async move {
                    let mut backoff = 5;
                    loop {
                        let result = listen(&ctx2, tenant, &station).await;
                        if let Err(err) = &result {
                            tracing::debug!(station = %station.name, error = %err, "door station events");
                        }
                        if let Ok(true) = doors::set_online(&ctx2.db, station.id, false, None).await
                        {
                            tracing::warn!(station = %station.name, "door station offline");
                            ctx2.record(tenant, &station, "offline", json!({})).await;
                            backoff = 5;
                        }
                        tokio::time::sleep(Duration::from_secs(backoff)).await;
                        backoff = (backoff * 2).min(60);
                    }
                });
                running.insert(id, (k, handle));
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}

/// Deletes door events past the retention period, with their snapshots.
pub async fn purge(
    db: &PgPool,
    snapshots: &std::path::Path,
) -> talkops_core::error::CoreResult<usize> {
    let files = doors::purge_events(db, 500).await?;
    for f in &files {
        let _ = tokio::fs::remove_file(snapshots.join(f)).await;
    }
    Ok(files.len())
}

/// Removes snapshot files (after deleting a station).
pub async fn remove_files(snapshots: PathBuf, files: Vec<String>) {
    for f in files {
        let _ = tokio::fs::remove_file(snapshots.join(f)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: &str, action: &str, data: Value) -> Event {
        Event {
            code: code.into(),
            action: action.into(),
            index: 0,
            data,
        }
    }

    #[test]
    fn classifies_device_events() {
        let (kind, detail) = classify(&ev(
            "AccessControl",
            "Pulse",
            json!({"Method": 4, "UserID": "101"}),
        ))
        .unwrap();
        assert_eq!(kind, "opened");
        assert_eq!(detail["method"], 4);
        assert_eq!(
            classify(&ev("DoorStatus", "Pulse", json!({"Status": "Open"})))
                .unwrap()
                .0,
            "door_open"
        );
        assert_eq!(
            classify(&ev("DoorStatus", "Pulse", json!({"Status": "Close"})))
                .unwrap()
                .0,
            "door_closed"
        );
        assert_eq!(
            classify(&ev("BackKeyLight", "Pulse", json!({"State": 9})))
                .unwrap()
                .0,
            "unlock_failed"
        );
        assert!(classify(&ev("BackKeyLight", "Pulse", json!({"State": 1}))).is_none());
        assert_eq!(
            classify(&ev("AlarmLocal", "Start", json!({}))).unwrap().0,
            "alarm"
        );
        assert!(classify(&ev("AlarmLocal", "Stop", json!({}))).is_none());
        assert!(classify(&ev("Invite", "Pulse", json!({}))).is_none());
    }
}
