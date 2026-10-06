//! Door stations (Dahua VTO): the station is the device of an extension, so
//! it registers and calls like a phone. TalkOps routes its rings, opens the
//! door through the station's HTTP API and keeps an event log with
//! snapshots.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::crypto::{self, SecretBox};
use crate::error::{CoreError, CoreResult};
use crate::numbering;
use crate::tenant::TenantId;
use crate::trunks::NumberDestination;

/// A button of a multi-button station: the number it dials and where that goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Button {
    pub number: String,
    #[serde(rename = "type")]
    pub kind: NumberDestination,
    #[serde(default)]
    pub id: Option<Uuid>,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct DoorStation {
    pub id: Uuid,
    pub name: String,
    pub extension_id: Uuid,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub has_password: bool,
    pub doors: i16,
    pub destination_type: NumberDestination,
    pub destination_id: Option<Uuid>,
    #[schema(value_type = Vec<Button>)]
    pub buttons: Json<Vec<Button>>,
    pub events_enabled: bool,
    pub snapshots: bool,
    pub has_webhook: bool,
    pub has_api_token: bool,
    pub online: bool,
    pub model: String,
    pub last_seen: Option<DateTime<Utc>>,
    pub enabled: bool,
}

impl DoorStation {
    /// Has an HTTP API to talk to (door opener, snapshots, events).
    pub fn has_api(&self) -> bool {
        !self.host.is_empty()
    }

    /// Destination for a ring: the button whose number was dialed, else the
    /// station's default. Dahua stations dial e.g. `9901` or `9901#0`.
    pub fn route(&self, dialed: &str) -> (NumberDestination, Option<Uuid>) {
        let base = |n: &str| n.split('#').next().unwrap_or(n).to_owned();
        self.buttons
            .iter()
            .find(|b| b.number == dialed)
            .or_else(|| {
                self.buttons
                    .iter()
                    .find(|b| base(&b.number) == base(dialed))
            })
            .map(|b| (b.kind, b.id))
            .unwrap_or((self.destination_type, self.destination_id))
    }
}

const COLUMNS: &str = "id, name, extension_id, host, port, username, password_enc IS NOT NULL AS has_password, \
     doors, destination_type, destination_id, buttons, events_enabled, snapshots, \
     webhook_url_enc IS NOT NULL AS has_webhook, api_token_hash IS NOT NULL AS has_api_token, \
     online, model, last_seen, enabled";

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DoorStationInput {
    pub name: String,
    pub extension_id: Uuid,
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i32,
    #[serde(default = "default_user")]
    pub username: String,
    /// `None` keeps the stored password, `""` removes it.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "one")]
    pub doors: i16,
    #[serde(default = "no_destination")]
    pub destination_type: NumberDestination,
    #[serde(default)]
    pub destination_id: Option<Uuid>,
    #[serde(default)]
    pub buttons: Vec<Button>,
    #[serde(default = "yes")]
    pub events_enabled: bool,
    #[serde(default = "yes")]
    pub snapshots: bool,
    /// `None` keeps the stored webhook, `""` removes it.
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn default_port() -> i32 {
    80
}
fn default_user() -> String {
    "admin".into()
}
fn one() -> i16 {
    1
}
fn yes() -> bool {
    true
}
fn no_destination() -> NumberDestination {
    NumberDestination::None
}

fn valid_host(h: &str) -> bool {
    h.len() <= 253
        && h.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:-[]".contains(&b))
}

fn valid_dialed(n: &str) -> bool {
    (1..=16).contains(&n.len())
        && n.bytes()
            .all(|b| b.is_ascii_digit() || b == b'#' || b == b'*')
}

async fn validate(pool: &PgPool, tenant: TenantId, input: &DoorStationInput) -> CoreResult<()> {
    let invalid = |m: &str| Err(CoreError::Validation(m.into()));
    if input.name.trim().is_empty() || input.name.len() > 64 {
        return invalid("name must have 1-64 characters");
    }
    if !valid_host(input.host.trim()) {
        return invalid("invalid host");
    }
    if !(1..=65535).contains(&input.port) {
        return invalid("invalid port");
    }
    if !(1..=2).contains(&input.doors) {
        return invalid("a station has one or two doors");
    }
    if input.username.len() > 64 {
        return invalid("user name too long");
    }
    if let Some(url) = input.webhook_url.as_deref().filter(|u| !u.is_empty()) {
        if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() > 2000 {
            return invalid("the webhook must be an http(s) URL");
        }
    }
    let ext: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM extensions WHERE tenant_id = $1 AND id = $2")
            .bind(tenant)
            .bind(input.extension_id)
            .fetch_optional(pool)
            .await?;
    if ext.is_none() {
        return invalid("unknown extension");
    }
    numbering::check_destination(pool, tenant, input.destination_type, input.destination_id)
        .await?;
    if input.buttons.len() > 40 {
        return invalid("at most 40 buttons");
    }
    for (i, b) in input.buttons.iter().enumerate() {
        if !valid_dialed(&b.number) {
            return invalid("button numbers have 1-16 digits, # or *");
        }
        if input.buttons[..i].iter().any(|o| o.number == b.number) {
            return invalid("duplicate button number");
        }
        numbering::check_destination(pool, tenant, b.kind, b.id).await?;
    }
    Ok(())
}

pub async fn list<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<DoorStation>> {
    let sql = format!("SELECT {COLUMNS} FROM door_stations WHERE tenant_id = $1 ORDER BY name");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<DoorStation> {
    let sql = format!("SELECT {COLUMNS} FROM door_stations WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

/// The door station behind an extension (calls from it are rings).
pub async fn for_extension<'e>(
    db: impl PgExecutor<'e>,
    extension: Uuid,
) -> CoreResult<Option<DoorStation>> {
    let sql = format!("SELECT {COLUMNS} FROM door_stations WHERE extension_id = $1 AND enabled");
    Ok(sqlx::query_as(&sql)
        .bind(extension)
        .fetch_optional(db)
        .await?)
}

/// Enabled stations with an HTTP API, all tenants (event listeners).
pub async fn with_api(pool: &PgPool) -> CoreResult<Vec<(TenantId, DoorStation)>> {
    #[derive(FromRow)]
    struct Row {
        tenant_id: TenantId,
        #[sqlx(flatten)]
        station: DoorStation,
    }
    let sql = format!(
        "SELECT tenant_id, {COLUMNS} FROM door_stations WHERE enabled AND host <> '' ORDER BY id"
    );
    let rows: Vec<Row> = sqlx::query_as(&sql).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| (r.tenant_id, r.station)).collect())
}

fn encrypt_opt(secrets: &SecretBox, v: Option<&str>) -> CoreResult<Option<Option<String>>> {
    Ok(match v {
        None => None,
        Some("") => Some(None),
        Some(v) => Some(Some(secrets.encrypt(v)?)),
    })
}

pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    input: &DoorStationInput,
) -> CoreResult<DoorStation> {
    validate(pool, tenant, input).await?;
    let password = encrypt_opt(secrets, input.password.as_deref())?.flatten();
    let webhook = encrypt_opt(secrets, input.webhook_url.as_deref())?.flatten();
    let sql = format!(
        "INSERT INTO door_stations (tenant_id, name, extension_id, host, port, username, password_enc,
                                    doors, destination_type, destination_id, buttons, events_enabled,
                                    snapshots, webhook_url_enc, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
         RETURNING {COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(input.name.trim())
        .bind(input.extension_id)
        .bind(input.host.trim())
        .bind(input.port)
        .bind(input.username.trim())
        .bind(password)
        .bind(input.doors)
        .bind(input.destination_type)
        .bind(input.destination_id)
        .bind(Json(&input.buttons))
        .bind(input.events_enabled)
        .bind(input.snapshots)
        .bind(webhook)
        .bind(input.enabled)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(d) if d.is_unique_violation() => {
                CoreError::Validation("this extension is already a door station".into())
            }
            e => e.into(),
        })
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
    input: &DoorStationInput,
) -> CoreResult<DoorStation> {
    validate(pool, tenant, input).await?;
    let password = encrypt_opt(secrets, input.password.as_deref())?;
    let webhook = encrypt_opt(secrets, input.webhook_url.as_deref())?;
    let sql = format!(
        "UPDATE door_stations SET name = $3, extension_id = $4, host = $5, port = $6, username = $7,
             password_enc = CASE WHEN $8 THEN $9 ELSE password_enc END,
             doors = $10, destination_type = $11, destination_id = $12, buttons = $13,
             events_enabled = $14, snapshots = $15,
             webhook_url_enc = CASE WHEN $16 THEN $17 ELSE webhook_url_enc END,
             enabled = $18, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
    );
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(input.name.trim())
        .bind(input.extension_id)
        .bind(input.host.trim())
        .bind(input.port)
        .bind(input.username.trim())
        .bind(password.is_some())
        .bind(password.flatten())
        .bind(input.doors)
        .bind(input.destination_type)
        .bind(input.destination_id)
        .bind(Json(&input.buttons))
        .bind(input.events_enabled)
        .bind(input.snapshots)
        .bind(webhook.is_some())
        .bind(webhook.flatten())
        .bind(input.enabled)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(d) if d.is_unique_violation() => {
                CoreError::Validation("this extension is already a door station".into())
            }
            e => e.into(),
        })
}

/// Deletes a station and returns the snapshot files of its events.
pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Vec<String>> {
    let mut tx = pool.begin().await?;
    let files: Vec<String> = sqlx::query_scalar(
        "SELECT snapshot FROM door_events WHERE door_station_id = $1 AND snapshot IS NOT NULL",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let n = sqlx::query("DELETE FROM door_stations WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    tx.commit().await?;
    Ok(files)
}

/// HTTP API access of a station (password decrypted).
#[derive(Debug, Clone)]
pub struct Connection {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

pub async fn connection(
    pool: &PgPool,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<Option<Connection>> {
    let row: Option<(String, i32, String, Option<String>)> = sqlx::query_as(
        "SELECT host, port, username, password_enc FROM door_stations WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some((host, port, username, password)) = row.filter(|r| !r.0.is_empty()) else {
        return Ok(None);
    };
    Ok(Some(Connection {
        host,
        port: port as u16,
        username,
        password: password
            .map(|p| secrets.decrypt(&p))
            .transpose()?
            .unwrap_or_default(),
    }))
}

pub async fn webhook_url(
    pool: &PgPool,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<Option<String>> {
    let enc: Option<Option<String>> =
        sqlx::query_scalar("SELECT webhook_url_enc FROM door_stations WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(enc.flatten().map(|e| secrets.decrypt(&e)).transpose()?)
}

/// Creates a new API token (shown once) that may open the door.
pub async fn new_api_token(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<String> {
    let token = crypto::random_token(24)?;
    let n = sqlx::query(
        "UPDATE door_stations SET api_token_hash = $3 WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(crypto::token_digest(&token))
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(token)
}

pub async fn revoke_api_token(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    sqlx::query("UPDATE door_stations SET api_token_hash = NULL WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// The station a valid API token belongs to.
pub async fn by_api_token(
    pool: &PgPool,
    id: Uuid,
    token: &str,
) -> CoreResult<Option<(TenantId, DoorStation)>> {
    #[derive(FromRow)]
    struct Row {
        tenant_id: TenantId,
        #[sqlx(flatten)]
        station: DoorStation,
    }
    let sql = format!(
        "SELECT tenant_id, {COLUMNS} FROM door_stations
         WHERE id = $1 AND enabled AND api_token_hash = $2"
    );
    let row: Option<Row> = sqlx::query_as(&sql)
        .bind(id)
        .bind(crypto::token_digest(token))
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| (r.tenant_id, r.station)))
}

/// Records whether the event stream is connected; `true` if that changed.
pub async fn set_online(
    pool: &PgPool,
    id: Uuid,
    online: bool,
    model: Option<&str>,
) -> CoreResult<bool> {
    let changed: Option<bool> = sqlx::query_scalar(
        "UPDATE door_stations d SET online = $2, model = COALESCE($3, d.model),
             last_seen = CASE WHEN $2 THEN now() ELSE d.last_seen END
         FROM (SELECT online AS before FROM door_stations WHERE id = $1) old
         WHERE d.id = $1 RETURNING old.before <> $2",
    )
    .bind(id)
    .bind(online)
    .bind(model)
    .fetch_optional(pool)
    .await?;
    Ok(changed.unwrap_or(false))
}

// --- events ------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct DoorEvent {
    pub id: Uuid,
    pub door_station_id: Uuid,
    pub kind: String,
    pub detail: serde_json::Value,
    pub has_snapshot: bool,
    #[serde(skip)]
    pub snapshot: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub const EVENT_KINDS: &[&str] = &[
    "ring",
    "open_command",
    "opened",
    "door_open",
    "door_closed",
    "unlock_failed",
    "alarm",
    "online",
    "offline",
];

const E_COLUMNS: &str =
    "id, door_station_id, kind, detail, snapshot IS NOT NULL AS has_snapshot, snapshot, created_at";

/// Snapshot file of an event, relative to the snapshots volume.
pub fn snapshot_file(tenant: TenantId, now: DateTime<Utc>, event: Uuid) -> String {
    format!("{tenant}/{}/{event}.jpg", now.format("%Y-%m"))
}

pub async fn add_event(
    pool: &PgPool,
    tenant: TenantId,
    station: Uuid,
    kind: &str,
    detail: serde_json::Value,
) -> CoreResult<DoorEvent> {
    if !EVENT_KINDS.contains(&kind) {
        return Err(CoreError::Validation(format!("unknown event kind {kind}")));
    }
    let sql = format!(
        "INSERT INTO door_events (tenant_id, door_station_id, kind, detail)
         VALUES ($1, $2, $3, $4) RETURNING {E_COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(station)
        .bind(kind)
        .bind(detail)
        .fetch_one(pool)
        .await?)
}

pub async fn set_snapshot(pool: &PgPool, event: Uuid, file: &str) -> CoreResult<()> {
    sqlx::query("UPDATE door_events SET snapshot = $2 WHERE id = $1")
        .bind(event)
        .bind(file)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_event(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<DoorEvent> {
    let sql = format!("SELECT {E_COLUMNS} FROM door_events WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?)
}

/// Newest events first, optionally of one station.
pub async fn list_events(
    pool: &PgPool,
    tenant: TenantId,
    station: Option<Uuid>,
    limit: i64,
) -> CoreResult<Vec<DoorEvent>> {
    let sql = format!(
        "SELECT {E_COLUMNS} FROM door_events
         WHERE tenant_id = $1 AND ($2::uuid IS NULL OR door_station_id = $2)
         ORDER BY created_at DESC LIMIT $3"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(station)
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?)
}

/// Deletes events past the tenant's recording retention (snapshots show
/// people, like recordings); returns their snapshot files.
pub async fn purge_events(pool: &PgPool, limit: i64) -> CoreResult<Vec<String>> {
    let files: Vec<Option<String>> = sqlx::query_scalar(
        "DELETE FROM door_events WHERE id IN (
             SELECT e.id FROM door_events e
             JOIN tenant_settings s ON s.tenant_id = e.tenant_id
             WHERE s.recording_retention_days > 0
               AND e.created_at < now() - make_interval(days => s.recording_retention_days)
             ORDER BY e.created_at LIMIT $1)
         RETURNING snapshot",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(files.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(buttons: Vec<Button>) -> DoorStation {
        DoorStation {
            id: Uuid::nil(),
            name: "Door".into(),
            extension_id: Uuid::nil(),
            host: String::new(),
            port: 80,
            username: "admin".into(),
            has_password: false,
            doors: 1,
            destination_type: NumberDestination::RingGroup,
            destination_id: Some(Uuid::from_u128(1)),
            buttons: Json(buttons),
            events_enabled: true,
            snapshots: true,
            has_webhook: false,
            has_api_token: false,
            online: false,
            model: String::new(),
            last_seen: None,
            enabled: true,
        }
    }

    #[test]
    fn routes_buttons() {
        let s = station(vec![
            Button {
                number: "9902".into(),
                kind: NumberDestination::Extension,
                id: Some(Uuid::from_u128(2)),
            },
            Button {
                number: "9903#1".into(),
                kind: NumberDestination::Extension,
                id: Some(Uuid::from_u128(3)),
            },
        ]);
        assert_eq!(s.route("9901").1, Some(Uuid::from_u128(1)));
        assert_eq!(s.route("9902").1, Some(Uuid::from_u128(2)));
        assert_eq!(s.route("9902#0").1, Some(Uuid::from_u128(2)));
        assert_eq!(s.route("9903#1").1, Some(Uuid::from_u128(3)));
        assert_eq!(s.route("9903").1, Some(Uuid::from_u128(3)));
        assert!(!s.has_api());
        assert!(valid_dialed("9901#0") && !valid_dialed("99a") && !valid_dialed(""));
        assert!(valid_host("192.168.1.20") && valid_host("[fe80::1]") && !valid_host("a b"));
    }
}
