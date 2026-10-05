//! Provisioned phones, firmware images, shared phonebook contacts and the
//! provisioning credentials shown to the administrator.

use std::collections::HashSet;

use axum::Json;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use talkops_core::audit;
use talkops_core::phones::{self, Contact, ContactInput, Firmware, Phone, PhoneInput};
use talkops_core::users::Role;
use talkops_provisioning::PhoneModel;
use talkops_provisioning::yealink::{KeyType, LineKey};
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::routes::provisioning::{base_url, firmware_path, render_phone_config};

/// Largest firmware image accepted (Yealink images are 20–150 MB).
const FIRMWARE_LIMIT: usize = 512 * 1024 * 1024;

pub fn router() -> OpenApiRouter<AppState> {
    let upload = OpenApiRouter::new()
        .routes(routes!(list_firmware, upload_firmware))
        .layer(DefaultBodyLimit::max(FIRMWARE_LIMIT));
    OpenApiRouter::new()
        .routes(routes!(list_phones, create_phone))
        .routes(routes!(get_phone, update_phone, delete_phone))
        .routes(routes!(resync_phone))
        .routes(routes!(phone_config))
        .routes(routes!(phone_models))
        .routes(routes!(update_firmware, delete_firmware))
        .routes(routes!(list_contacts, create_contact))
        .routes(routes!(update_contact, delete_contact))
        .routes(routes!(provisioning_info))
        .routes(routes!(regenerate_provisioning))
        .merge(upload)
}

/// A phone model from the catalog (`presets/phones`).
#[derive(Serialize, ToSchema)]
pub struct PhoneModelInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    /// `desk`, `dect` or `conference`.
    pub family: String,
    pub accounts: u16,
    pub line_keys: u16,
    pub video: bool,
}

impl From<&PhoneModel> for PhoneModelInfo {
    fn from(m: &PhoneModel) -> Self {
        Self {
            id: m.id.clone(),
            name: m.name.clone(),
            vendor: m.vendor.clone(),
            family: serde_json::to_value(m.family)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            accounts: m.accounts,
            line_keys: m.line_keys,
            video: m.video,
        }
    }
}

/// A SIP account placed on a phone.
#[derive(Serialize, ToSchema)]
pub struct PhoneAccountInfo {
    pub account_index: i16,
    pub device_id: Uuid,
    pub extension_id: Uuid,
    pub extension_number: String,
    pub display_name: String,
}

#[derive(Serialize, ToSchema)]
pub struct PhoneDetail {
    #[serde(flatten)]
    pub phone: Phone,
    pub accounts: Vec<PhoneAccountInfo>,
}

/// Checks the model and the key layout against the catalog.
fn validate_phone(state: &AppState, input: &PhoneInput) -> ApiResult<()> {
    let model = state
        .phone_catalog
        .get(&input.model)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown phone model `{}`", input.model)))?;
    let keys: Vec<LineKey> = serde_json::from_value(input.line_keys.clone())
        .map_err(|e| ApiError::BadRequest(format!("invalid line_keys: {e}")))?;
    let mut seen = HashSet::new();
    for k in &keys {
        if k.key < 1 || k.key > model.line_keys {
            return Err(ApiError::BadRequest(format!(
                "key {} does not exist on {}",
                k.key, model.name
            )));
        }
        if !seen.insert(k.key) {
            return Err(ApiError::BadRequest(format!(
                "key {} is configured twice",
                k.key
            )));
        }
        if k.account < 1 || k.account > model.accounts {
            return Err(ApiError::BadRequest(format!(
                "key {}: invalid account",
                k.key
            )));
        }
        if k.label.chars().count() > 32 {
            return Err(ApiError::BadRequest(format!(
                "key {}: label too long",
                k.key
            )));
        }
        let dialable = !k.value.is_empty()
            && k.value.len() <= 32
            && k.value
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '*' | '#' | '+'));
        if matches!(k.kind, KeyType::Blf | KeyType::SpeedDial) && !dialable {
            return Err(ApiError::BadRequest(format!(
                "key {}: invalid number",
                k.key
            )));
        }
    }
    Ok(())
}

/// Lists provisioned phones (operator or admin).
#[utoipa::path(get, path = "/api/v1/phones", tag = "phones", responses((status = 200, body = [Phone])))]
pub async fn list_phones(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Phone>>> {
    auth.require(Role::Operator)?;
    Ok(Json(phones::list(&state.db, auth.tenant).await?))
}

/// Adds a phone by MAC address (admin).
#[utoipa::path(post, path = "/api/v1/phones", tag = "phones", request_body = PhoneInput, responses((status = 200, body = Phone)))]
pub async fn create_phone(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<PhoneInput>,
) -> ApiResult<Json<Phone>> {
    auth.require(Role::Admin)?;
    validate_phone(&state, &input)?;
    let phone = phones::create(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "phone",
        Some(phone.id.to_string()),
        json!({"mac": phone.mac, "model": phone.model}),
    )
    .await?;
    Ok(Json(phone))
}

/// Returns a phone with the accounts placed on it.
#[utoipa::path(get, path = "/api/v1/phones/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 200, body = PhoneDetail)))]
pub async fn get_phone(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<PhoneDetail>> {
    auth.require(Role::Operator)?;
    let phone = phones::get(&state.db, auth.tenant, id).await?;
    let accounts = phones::accounts(&state.db, phone.id)
        .await?
        .into_iter()
        .map(|a| PhoneAccountInfo {
            account_index: a.account_index,
            device_id: a.device_id,
            extension_id: a.extension_id,
            extension_number: a.extension_number,
            display_name: a.display_name,
        })
        .collect();
    Ok(Json(PhoneDetail { phone, accounts }))
}

/// Updates a phone (admin). Call resync afterwards to apply it.
#[utoipa::path(put, path = "/api/v1/phones/{id}", tag = "phones", params(("id" = Uuid, Path)), request_body = PhoneInput, responses((status = 200, body = Phone)))]
pub async fn update_phone(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<PhoneInput>,
) -> ApiResult<Json<Phone>> {
    auth.require(Role::Admin)?;
    validate_phone(&state, &input)?;
    let phone = phones::update(&state.db, auth.tenant, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "phone",
        Some(id.to_string()),
        json!({"mac": phone.mac, "model": phone.model}),
    )
    .await?;
    Ok(Json(phone))
}

/// Removes a phone (admin). Its devices stay, unassigned.
#[utoipa::path(delete, path = "/api/v1/phones/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_phone(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let phone = phones::get(&state.db, auth.tenant, id).await?;
    phones::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "phone",
        Some(id.to_string()),
        json!({"mac": phone.mac}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct ResyncResult {
    /// Number of registrations that were sent a check-sync NOTIFY.
    pub notified: usize,
}

/// Asks the phone to fetch its configuration again (SIP NOTIFY check-sync;
/// Yealink phones reboot to apply it). Admin.
#[utoipa::path(post, path = "/api/v1/phones/{id}/resync", tag = "phones", params(("id" = Uuid, Path)), responses((status = 200, body = ResyncResult)))]
pub async fn resync_phone(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ResyncResult>> {
    auth.require(Role::Admin)?;
    let phone = phones::get(&state.db, auth.tenant, id).await?;
    let mut notified = 0;
    // One NOTIFY per phone is enough; try the accounts until one is reachable.
    for user in phones::device_usernames(&state.db, phone.id).await? {
        if state.telephony.check_sync(&user).await {
            notified += 1;
            break;
        }
    }
    audit::record(
        &state.db,
        &auth.actor(),
        "resync",
        "phone",
        Some(id.to_string()),
        json!({"mac": phone.mac}),
    )
    .await?;
    Ok(Json(ResyncResult { notified }))
}

/// Shows the generated configuration file of a phone (admin). It contains
/// SIP passwords, so every view is audited.
#[utoipa::path(get, path = "/api/v1/phones/{id}/config", tag = "phones", params(("id" = Uuid, Path)), responses((status = 200, content_type = "text/plain", body = String)))]
pub async fn phone_config(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    auth.require(Role::Admin)?;
    let phone = phones::get(&state.db, auth.tenant, id).await?;
    let body = render_phone_config(&state, auth.tenant, &phone, &headers).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "view_config",
        "phone",
        Some(id.to_string()),
        json!({"mac": phone.mac}),
    )
    .await?;
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response())
}

/// Lists supported phone models.
#[utoipa::path(get, path = "/api/v1/phone-models", tag = "phones", responses((status = 200, body = [PhoneModelInfo])))]
pub async fn phone_models(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<PhoneModelInfo>>> {
    auth.require(Role::Operator)?;
    Ok(Json(state.phone_catalog.all().map(Into::into).collect()))
}

// --- firmware -------------------------------------------------------------------

/// Lists uploaded firmware images (admin).
#[utoipa::path(get, path = "/api/v1/firmware", tag = "phones", responses((status = 200, body = [Firmware])))]
pub async fn list_firmware(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Firmware>>> {
    auth.require(Role::Admin)?;
    Ok(Json(phones::list_firmware(&state.db, auth.tenant).await?))
}

fn valid_filename(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Uploads a firmware image (admin). Multipart fields: `model` (catalog id)
/// and `file`. The image is not activated automatically.
#[utoipa::path(post, path = "/api/v1/firmware", tag = "phones", request_body(content_type = "multipart/form-data", content = String), responses((status = 200, body = Firmware)))]
pub async fn upload_firmware(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<Firmware>> {
    auth.require(Role::Admin)?;
    let id = Uuid::new_v4();
    let dir = state.provisioning_dir.join("firmware").join(id.to_string());
    let result = receive_firmware(&state, &mut multipart, id, &dir).await;
    let (model, filename, size, sha256) = match result {
        Ok(v) => v,
        Err(err) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(err);
        }
    };
    let fw =
        match phones::create_firmware(&state.db, auth.tenant, id, &model, &filename, size, &sha256)
            .await
        {
            Ok(fw) => fw,
            Err(err) => {
                let _ = tokio::fs::remove_dir_all(&dir).await;
                return Err(err.into());
            }
        };
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "firmware",
        Some(fw.id.to_string()),
        json!({"model": fw.model, "filename": fw.filename, "sha256": fw.sha256}),
    )
    .await?;
    Ok(Json(fw))
}

async fn receive_firmware(
    state: &AppState,
    multipart: &mut Multipart,
    id: Uuid,
    dir: &std::path::Path,
) -> ApiResult<(String, String, i64, String)> {
    let bad = |m: &str| ApiError::BadRequest(m.to_owned());
    let mut model = None;
    let mut file = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
    {
        match field.name() {
            Some("model") => {
                model = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::BadRequest(e.to_string()))?,
                )
            }
            Some("file") => {
                let name = field
                    .file_name()
                    .and_then(|n| n.rsplit(['/', '\\']).next())
                    .unwrap_or_default()
                    .to_owned();
                if !valid_filename(&name) {
                    return Err(bad("invalid file name (allowed: A-Z a-z 0-9 . _ -)"));
                }
                tokio::fs::create_dir_all(dir)
                    .await
                    .map_err(|e| ApiError::Internal(format!("create {}: {e}", dir.display())))?;
                let path = firmware_path(state, id, &name);
                let mut out = tokio::fs::File::create(&path)
                    .await
                    .map_err(|e| ApiError::Internal(format!("create {}: {e}", path.display())))?;
                let mut hasher = Sha256::new();
                let mut size: i64 = 0;
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?
                {
                    hasher.update(&chunk);
                    size += chunk.len() as i64;
                    out.write_all(&chunk)
                        .await
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                }
                out.flush()
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                if size == 0 {
                    return Err(bad("file is empty"));
                }
                file = Some((name, size, hex::encode(hasher.finalize())));
            }
            _ => {}
        }
    }
    let model = model.ok_or_else(|| bad("field `model` is required"))?;
    if state.phone_catalog.get(&model).is_none() {
        return Err(ApiError::BadRequest(format!(
            "unknown phone model `{model}`"
        )));
    }
    let (name, size, sha) = file.ok_or_else(|| bad("field `file` is required"))?;
    Ok((model, name, size, sha))
}

#[derive(Deserialize, ToSchema)]
pub struct FirmwareUpdate {
    /// Make this the image phones of its model install.
    pub active: bool,
}

/// Activates or deactivates a firmware image (admin). Only one image per
/// model is active; phones pick it up on their next resync.
#[utoipa::path(put, path = "/api/v1/firmware/{id}", tag = "phones", params(("id" = Uuid, Path)), request_body = FirmwareUpdate, responses((status = 200, body = Firmware)))]
pub async fn update_firmware(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<FirmwareUpdate>,
) -> ApiResult<Json<Firmware>> {
    auth.require(Role::Admin)?;
    let fw = phones::set_firmware_active(&state.db, auth.tenant, id, input.active).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "firmware",
        Some(id.to_string()),
        json!({"model": fw.model, "filename": fw.filename, "active": fw.active}),
    )
    .await?;
    Ok(Json(fw))
}

/// Deletes a firmware image (admin).
#[utoipa::path(delete, path = "/api/v1/firmware/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_firmware(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let fw = phones::get_firmware(&state.db, auth.tenant, id).await?;
    phones::delete_firmware(&state.db, auth.tenant, id).await?;
    let dir = state.provisioning_dir.join("firmware").join(id.to_string());
    if let Err(err) = tokio::fs::remove_dir_all(&dir).await {
        tracing::warn!(path = %dir.display(), error = %err, "could not remove firmware");
    }
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "firmware",
        Some(id.to_string()),
        json!({"model": fw.model, "filename": fw.filename}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- contacts -------------------------------------------------------------------

/// Lists the shared phonebook (every logged-in user).
#[utoipa::path(get, path = "/api/v1/contacts", tag = "phones", responses((status = 200, body = [Contact])))]
pub async fn list_contacts(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Contact>>> {
    Ok(Json(phones::list_contacts(&state.db, auth.tenant).await?))
}

/// Adds a phonebook contact (operator or admin).
#[utoipa::path(post, path = "/api/v1/contacts", tag = "phones", request_body = ContactInput, responses((status = 200, body = Contact)))]
pub async fn create_contact(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<ContactInput>,
) -> ApiResult<Json<Contact>> {
    auth.require(Role::Operator)?;
    let c = phones::create_contact(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "contact",
        Some(c.id.to_string()),
        json!({"name": c.name}),
    )
    .await?;
    Ok(Json(c))
}

/// Updates a phonebook contact (operator or admin).
#[utoipa::path(put, path = "/api/v1/contacts/{id}", tag = "phones", params(("id" = Uuid, Path)), request_body = ContactInput, responses((status = 200, body = Contact)))]
pub async fn update_contact(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<ContactInput>,
) -> ApiResult<Json<Contact>> {
    auth.require(Role::Operator)?;
    let c = phones::update_contact(&state.db, auth.tenant, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "contact",
        Some(id.to_string()),
        json!({"name": c.name}),
    )
    .await?;
    Ok(Json(c))
}

/// Deletes a phonebook contact (operator or admin).
#[utoipa::path(delete, path = "/api/v1/contacts/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_contact(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Operator)?;
    phones::delete_contact(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "contact",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- provisioning credentials ---------------------------------------------------

/// Where and how phones fetch their configuration.
#[derive(Serialize, ToSchema)]
pub struct ProvisioningInfo {
    /// Provisioning server URL (enter in the phone or in DHCP option 66).
    pub url: String,
    /// The same URL with credentials embedded, for DHCP option 66.
    pub url_with_credentials: String,
    pub username: String,
    pub password: String,
    /// Admin password set on provisioned phones (web UI user `admin`).
    pub phone_admin_password: String,
}

async fn info(
    state: &AppState,
    auth: &AuthUser,
    headers: &HeaderMap,
    regenerate: bool,
) -> ApiResult<ProvisioningInfo> {
    let s =
        phones::provisioning_secrets(&state.db, auth.tenant, &state.secrets, regenerate).await?;
    let prov = talkops_provisioning::yealink::Provisioning {
        base_url: base_url(headers),
        username: s.username.clone(),
        password: s.password.clone(),
    };
    Ok(ProvisioningInfo {
        url: prov.base_url.clone(),
        url_with_credentials: prov.authenticated_base(),
        username: s.username,
        password: s.password,
        phone_admin_password: s.phone_admin_password,
    })
}

/// Shows the provisioning URL and credentials (admin, audited).
#[utoipa::path(get, path = "/api/v1/provisioning", tag = "phones", responses((status = 200, body = ProvisioningInfo)))]
pub async fn provisioning_info(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
) -> ApiResult<Json<ProvisioningInfo>> {
    auth.require(Role::Admin)?;
    let info = info(&state, &auth, &headers, false).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "view_secret",
        "provisioning",
        None,
        json!({}),
    )
    .await?;
    Ok(Json(info))
}

/// Generates new provisioning and phone admin passwords (admin). Phones keep
/// working until their next resync; update DHCP option 66 first.
#[utoipa::path(post, path = "/api/v1/provisioning/regenerate", tag = "phones", responses((status = 200, body = ProvisioningInfo)))]
pub async fn regenerate_provisioning(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
) -> ApiResult<Json<ProvisioningInfo>> {
    auth.require(Role::Admin)?;
    let info = info(&state, &auth, &headers, true).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "regenerate",
        "provisioning",
        None,
        json!({}),
    )
    .await?;
    Ok(Json(info))
}
