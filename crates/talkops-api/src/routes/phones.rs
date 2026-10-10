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
use talkops_core::contact_import::{self, ImportReport, ImportRequest};
use talkops_core::phones::{
    self, Contact, ContactInput, Firmware, MediaKind, Phone, PhoneInput, PhoneMedia,
    PhonebookSection, SectionInput,
};
use talkops_core::tenant::TenantId;
use talkops_core::users::Role;
use talkops_provisioning::yealink::{KeyType, LineKey};
use talkops_provisioning::{PhoneModel, PhoneSetting};
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::routes::provisioning::{
    base_url, firmware_path, media_path, media_type, render_phone_config, setting_values,
};

/// Largest firmware image accepted (Yealink images are 20–150 MB).
const FIRMWARE_LIMIT: usize = 512 * 1024 * 1024;
/// Largest custom ringtone any supported phone takes (T5x/T4U: 8 MB).
const RINGTONE_LIMIT: usize = 8 * 1024 * 1024;
/// Largest phone book CSV accepted (exports carry many unused columns).
const IMPORT_LIMIT: usize = 8 * 1024 * 1024;
/// Largest wallpaper Yealink phones take.
const WALLPAPER_LIMIT: usize = 5 * 1024 * 1024;

pub fn router() -> OpenApiRouter<AppState> {
    let upload = OpenApiRouter::new()
        .routes(routes!(list_firmware, upload_firmware))
        .layer(DefaultBodyLimit::max(FIRMWARE_LIMIT));
    let media_upload = OpenApiRouter::new()
        .routes(routes!(list_media, upload_media))
        .layer(DefaultBodyLimit::max(RINGTONE_LIMIT + 64 * 1024));
    let import = OpenApiRouter::new()
        .routes(routes!(import_contacts))
        .layer(DefaultBodyLimit::max(IMPORT_LIMIT));
    OpenApiRouter::new()
        .routes(routes!(list_phones, create_phone))
        .routes(routes!(get_phone, update_phone, delete_phone))
        .routes(routes!(resync_phone))
        .routes(routes!(update_phone_account))
        .routes(routes!(phone_config))
        .routes(routes!(phone_models))
        .routes(routes!(get_phone_settings, update_phone_settings))
        .routes(routes!(update_firmware, delete_firmware))
        .routes(routes!(list_contacts, create_contact))
        .routes(routes!(update_contact, delete_contact))
        .routes(routes!(delete_media))
        .routes(routes!(media_file))
        .routes(routes!(list_sections, create_section))
        .routes(routes!(update_section, delete_section))
        .routes(routes!(provisioning_info))
        .routes(routes!(regenerate_provisioning))
        .merge(upload)
        .merge(media_upload)
        .merge(import)
}

/// A phone model from the catalog (`presets/phones`).
#[derive(Serialize, ToSchema)]
pub struct PhoneModelInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    /// `desk`, `dect`, `conference` or `wifi`.
    pub family: String,
    pub accounts: u16,
    pub line_keys: u16,
    pub video: bool,
    /// Largest custom ringtone in KiB; 0 = not supported.
    pub ringtone_max_kb: u32,
    /// Custom wallpaper supported.
    pub wallpaper: bool,
    /// Line keys can be speed dials.
    pub speed_dial_keys: bool,
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
            ringtone_max_kb: m.ringtone_max_kb,
            wallpaper: m.wallpaper,
            speed_dial_keys: m.speed_dial_keys(),
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
    /// Label shown on the phone (empty = extension number).
    pub phone_label: String,
    /// Caller name sent by the phone (empty = `display_name`).
    pub phone_display_name: String,
}

#[derive(Serialize, ToSchema)]
pub struct PhoneDetail {
    #[serde(flatten)]
    pub phone: Phone,
    pub accounts: Vec<PhoneAccountInfo>,
}

/// Checks the model, the key layout and the comfort settings against the
/// catalog.
fn validate_phone(state: &AppState, input: &PhoneInput) -> ApiResult<()> {
    let model = state
        .phone_catalog
        .get(&input.model)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown phone model `{}`", input.model)))?;
    check_setting_values(state, &input.settings)?;
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
        if k.kind == KeyType::SpeedDial && !model.speed_dial_keys() {
            return Err(ApiError::BadRequest(format!(
                "{} has no speed dial keys (key {})",
                model.name, k.key
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
        // BLF keys may also watch a park slot (`park+*51` … `park+*59`).
        let park = k.kind == KeyType::Blf
            && k.value
                .strip_prefix("park+*7")
                .is_some_and(|n| n.len() == 1 && n.chars().all(|c| ('1'..='9').contains(&c)));
        if matches!(k.kind, KeyType::Blf | KeyType::SpeedDial) && !dialable && !park {
            return Err(ApiError::BadRequest(format!(
                "key {}: invalid number",
                k.key
            )));
        }
    }
    Ok(())
}

/// Checks the ringtone and wallpaper against what the model supports.
async fn validate_media(state: &AppState, auth: &AuthUser, input: &PhoneInput) -> ApiResult<()> {
    let Some(model) = state.phone_catalog.get(&input.model) else {
        return Ok(());
    };
    if let Some(id) = input.ringtone_id {
        if model.ringtone_max_kb == 0 {
            return Err(ApiError::BadRequest(format!(
                "{} does not support custom ringtones",
                model.name
            )));
        }
        let m = phones::get_media(&state.db, auth.tenant, id).await?;
        if m.size_bytes > i64::from(model.ringtone_max_kb) * 1024 {
            return Err(ApiError::BadRequest(format!(
                "ringtone too large for {} (at most {} KB)",
                model.name, model.ringtone_max_kb
            )));
        }
    }
    if input.wallpaper_id.is_some() && !model.wallpaper {
        return Err(ApiError::BadRequest(format!(
            "{} does not support custom wallpapers",
            model.name
        )));
    }
    Ok(())
}

/// Lists provisioned phones (operator or admin).
#[utoipa::path(get, path = "/api/v1/phones", tag = "phones", responses((status = 200, body = [PhoneListItem])))]
pub async fn list_phones(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<PhoneListItem>>> {
    auth.require(Role::Operator)?;
    let assigned = phones::assigned_extensions(&state.db, auth.tenant).await?;
    let list = phones::list(&state.db, auth.tenant)
        .await?
        .into_iter()
        .map(|phone| {
            let extensions = assigned
                .iter()
                .filter(|(id, _, _)| *id == phone.id)
                .map(|(_, number, name)| AssignedExtension {
                    number: number.clone(),
                    display_name: name.clone(),
                })
                .collect();
            PhoneListItem { phone, extensions }
        })
        .collect();
    Ok(Json(list))
}

/// An extension placed on a phone.
#[derive(Serialize, ToSchema)]
pub struct AssignedExtension {
    pub number: String,
    pub display_name: String,
}

/// A phone in the list, with the extensions on it.
#[derive(Serialize, ToSchema)]
pub struct PhoneListItem {
    #[serde(flatten)]
    pub phone: Phone,
    pub extensions: Vec<AssignedExtension>,
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
    validate_media(&state, &auth, &input).await?;
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
            phone_label: a.phone_label,
            phone_display_name: a.phone_display_name,
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
    validate_media(&state, &auth, &input).await?;
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

/// Label and display name of one account on a phone.
#[derive(Deserialize, ToSchema)]
pub struct AccountTexts {
    /// Shown on the phone (line key, idle screen); empty = extension number.
    #[serde(default)]
    pub phone_label: String,
    /// Caller name the phone sends; empty = the extension's display name.
    #[serde(default)]
    pub phone_display_name: String,
}

/// Sets label and display name of an account on the phone (admin). Resync
/// the phone to apply it.
#[utoipa::path(put, path = "/api/v1/phones/{id}/accounts/{device_id}", tag = "phones", params(("id" = Uuid, Path), ("device_id" = Uuid, Path)), request_body = AccountTexts, responses((status = 204)))]
pub async fn update_phone_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, device_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<AccountTexts>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let device = talkops_core::extensions::set_phone_texts(
        &state.db,
        auth.tenant,
        id,
        device_id,
        &input.phone_label,
        &input.phone_display_name,
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "device",
        Some(device_id.to_string()),
        json!({"phone_id": id, "phone_label": device.phone_label,
               "phone_display_name": device.phone_display_name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct ResyncResult {
    /// Number of registrations that were sent a check-sync NOTIFY.
    pub notified: usize,
}

#[derive(Deserialize, ToSchema, Default)]
pub struct ResyncRequest {
    /// Restart the phone as well (otherwise it only reloads its configuration).
    #[serde(default)]
    pub reboot: bool,
}

/// Asks the phone to fetch its configuration again (SIP NOTIFY check-sync),
/// optionally restarting it. Admin.
#[utoipa::path(post, path = "/api/v1/phones/{id}/resync", tag = "phones", params(("id" = Uuid, Path)), request_body = Option<ResyncRequest>, responses((status = 200, body = ResyncResult)))]
pub async fn resync_phone(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    body: Option<Json<ResyncRequest>>,
) -> ApiResult<Json<ResyncResult>> {
    let reboot = body.map(|Json(b)| b.reboot).unwrap_or_default();
    auth.require(Role::Admin)?;
    let phone = phones::get(&state.db, auth.tenant, id).await?;
    let mut notified = 0;
    // One NOTIFY per phone is enough; try the accounts until one is reachable.
    for user in phones::device_usernames(&state.db, phone.id).await? {
        if state.telephony.check_sync(&user, reboot).await {
            notified += 1;
            break;
        }
    }
    audit::record(
        &state.db,
        &auth.actor(),
        if reboot { "reboot" } else { "resync" },
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

/// Checks comfort settings (an object of strings) against the catalog.
fn check_setting_values(state: &AppState, value: &serde_json::Value) -> ApiResult<()> {
    phones::check_settings_shape(value)?;
    state
        .phone_catalog
        .check_settings(&setting_values(value))
        .map_err(ApiError::BadRequest)
}

/// A comfort setting from the catalog.
#[derive(Serialize, ToSchema)]
pub struct PhoneSettingInfo {
    pub key: String,
    /// UI section: `tones`, `cradle`, `calls`, `display`, `notifications`, `audio`.
    pub group: String,
    /// `bool` (values `0`/`1`) or `choice`.
    pub kind: String,
    /// Allowed values, in display order.
    pub values: Vec<String>,
    /// The phone's factory value, if documented.
    pub default: Option<String>,
    /// Phone families that take it (`desk`, `dect`, `conference`, `wifi`).
    pub families: Vec<String>,
    /// Configuration parameter written to the phone.
    pub param: String,
}

impl From<&PhoneSetting> for PhoneSettingInfo {
    fn from(s: &PhoneSetting) -> Self {
        let name = |v: serde_json::Result<serde_json::Value>| {
            v.ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let values = if s.values.is_empty() {
            vec!["1".to_owned(), "0".to_owned()]
        } else {
            s.values.clone()
        };
        Self {
            key: s.key.clone(),
            group: s.group.clone(),
            kind: name(serde_json::to_value(s.kind)),
            values,
            default: s.default.clone(),
            families: s
                .families
                .iter()
                .map(|f| name(serde_json::to_value(f)))
                .collect(),
            param: s.param.clone(),
        }
    }
}

/// The comfort settings catalog and the values for all phones.
#[derive(Serialize, ToSchema)]
pub struct PhoneSettingsView {
    pub catalog: Vec<PhoneSettingInfo>,
    /// Values for all phones (`{"key_tone": "0"}`); a phone's own value wins.
    #[schema(value_type = Object)]
    pub values: serde_json::Value,
}

#[derive(Deserialize, ToSchema)]
pub struct PhoneSettingsInput {
    #[schema(value_type = Object)]
    pub values: serde_json::Value,
}

async fn phone_settings_view(state: &AppState, tenant: TenantId) -> ApiResult<PhoneSettingsView> {
    Ok(PhoneSettingsView {
        catalog: state
            .phone_catalog
            .settings()
            .iter()
            .map(Into::into)
            .collect(),
        values: phones::phone_defaults(&state.db, tenant).await?,
    })
}

/// Comfort settings (key tone, display …) for all phones, with the catalog.
#[utoipa::path(get, path = "/api/v1/phone-settings", tag = "phones", responses((status = 200, body = PhoneSettingsView)))]
pub async fn get_phone_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<PhoneSettingsView>> {
    auth.require(Role::Admin)?;
    Ok(Json(phone_settings_view(&state, auth.tenant).await?))
}

/// Sets the comfort settings for all phones (admin). Phones pick them up
/// with the next resync.
#[utoipa::path(put, path = "/api/v1/phone-settings", tag = "phones", request_body = PhoneSettingsInput, responses((status = 200, body = PhoneSettingsView)))]
pub async fn update_phone_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<PhoneSettingsInput>,
) -> ApiResult<Json<PhoneSettingsView>> {
    auth.require(Role::Admin)?;
    check_setting_values(&state, &input.values)?;
    phones::set_phone_defaults(&state.db, auth.tenant, &input.values).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "phone_settings",
        None,
        json!({"values": input.values}),
    )
    .await?;
    Ok(Json(phone_settings_view(&state, auth.tenant).await?))
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

// --- ringtones and wallpapers ------------------------------------------------------

/// Lists uploaded ringtones and wallpapers (operator or admin).
#[utoipa::path(get, path = "/api/v1/phone-media", tag = "phones", responses((status = 200, body = [PhoneMedia])))]
pub async fn list_media(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<PhoneMedia>>> {
    auth.require(Role::Operator)?;
    Ok(Json(phones::list_media(&state.db, auth.tenant).await?))
}

/// Checks an uploaded file; returns the file extension the phone gets.
fn check_media(kind: MediaKind, data: &[u8]) -> ApiResult<&'static str> {
    let bad = |m: &str| Err(ApiError::BadRequest(m.to_owned()));
    match kind {
        MediaKind::Ringtone => {
            let Ok(reader) = hound::WavReader::new(std::io::Cursor::new(data)) else {
                return bad("ringtone: not a WAV file");
            };
            let spec = reader.spec();
            if spec.sample_rate != 8000
                || spec.channels != 1
                || spec.bits_per_sample != 16
                || spec.sample_format != hound::SampleFormat::Int
            {
                return bad("ringtone: WAV must be 8 kHz, mono, 16 bit");
            }
            if reader.duration() == 0 {
                return bad("ringtone: the file is empty");
            }
            if data.len() > RINGTONE_LIMIT {
                return bad("ringtone: at most 8 MB");
            }
            Ok("wav")
        }
        MediaKind::Wallpaper => {
            if data.len() > WALLPAPER_LIMIT {
                return bad("wallpaper: at most 5 MB");
            }
            if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
                Ok("jpg")
            } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
                Ok("png")
            } else {
                bad("wallpaper: JPEG or PNG required")
            }
        }
    }
}

/// Uploads a ringtone or wallpaper (admin). Multipart fields: `kind`
/// (`ringtone` or `wallpaper`), `name` and `file` (ringtone: WAV 8 kHz mono
/// 16 bit – the web UI converts other formats; wallpaper: JPEG or PNG).
#[utoipa::path(post, path = "/api/v1/phone-media", tag = "phones", request_body(content_type = "multipart/form-data", content = String), responses((status = 200, body = PhoneMedia)))]
pub async fn upload_media(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<PhoneMedia>> {
    auth.require(Role::Admin)?;
    let text_err =
        |e: axum::extract::multipart::MultipartError| ApiError::BadRequest(e.to_string());
    let (mut kind, mut name, mut data) = (None, String::new(), None);
    while let Some(field) = multipart.next_field().await.map_err(text_err)? {
        match field.name() {
            Some("kind") => {
                kind = match field.text().await.map_err(text_err)?.as_str() {
                    "ringtone" => Some(MediaKind::Ringtone),
                    "wallpaper" => Some(MediaKind::Wallpaper),
                    _ => return Err(ApiError::BadRequest("kind: ringtone or wallpaper".into())),
                }
            }
            Some("name") => name = field.text().await.map_err(text_err)?,
            Some("file") => data = Some(field.bytes().await.map_err(text_err)?),
            _ => {}
        }
    }
    let kind = kind.ok_or_else(|| ApiError::BadRequest("field `kind` is required".into()))?;
    let data = data.ok_or_else(|| ApiError::BadRequest("field `file` is required".into()))?;
    let ext = check_media(kind, &data)?;
    let id = Uuid::new_v4();
    // Phones keep custom files by name: a unique name per upload.
    let filename = format!("talkops-{}.{ext}", &id.simple().to_string()[..12]);
    let path = media_path(&state, id, &filename);
    let dir = path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ApiError::Internal(format!("create {}: {e}", dir.display())))?;
    tokio::fs::write(&path, &data)
        .await
        .map_err(|e| ApiError::Internal(format!("write {}: {e}", path.display())))?;
    let media = match phones::create_media(
        &state.db,
        auth.tenant,
        id,
        kind,
        &name,
        &filename,
        data.len() as i64,
    )
    .await
    {
        Ok(m) => m,
        Err(err) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(err.into());
        }
    };
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "phone_media",
        Some(media.id.to_string()),
        json!({"kind": kind.as_str(), "name": media.name, "size_bytes": media.size_bytes}),
    )
    .await?;
    Ok(Json(media))
}

/// The uploaded file, for preview in the web UI (operator or admin).
#[utoipa::path(get, path = "/api/v1/phone-media/{id}/file", tag = "phones", params(("id" = Uuid, Path)), responses((status = 200, content_type = "application/octet-stream", body = Vec<u8>)))]
pub async fn media_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    auth.require(Role::Operator)?;
    let m = phones::get_media(&state.db, auth.tenant, id).await?;
    let data = tokio::fs::read(media_path(&state, m.id, &m.filename))
        .await
        .map_err(|_| ApiError::NotFound)?;
    Ok(([(header::CONTENT_TYPE, media_type(&m.filename))], data).into_response())
}

/// Deletes a ringtone or wallpaper (admin). Phones using it keep their
/// current one until they are reset.
#[utoipa::path(delete, path = "/api/v1/phone-media/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_media(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let m = phones::get_media(&state.db, auth.tenant, id).await?;
    phones::delete_media(&state.db, auth.tenant, id).await?;
    let dir = state.provisioning_dir.join("media").join(id.to_string());
    if let Err(err) = tokio::fs::remove_dir_all(&dir).await {
        tracing::warn!(path = %dir.display(), error = %err, "could not remove phone media");
    }
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "phone_media",
        Some(id.to_string()),
        json!({"kind": m.kind.as_str(), "name": m.name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- phone book sections ----------------------------------------------------------

/// Lists the phone book sections (every logged-in user).
#[utoipa::path(get, path = "/api/v1/phonebook-sections", tag = "phones", responses((status = 200, body = [PhonebookSection])))]
pub async fn list_sections(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<PhonebookSection>>> {
    Ok(Json(phones::list_sections(&state.db, auth.tenant).await?))
}

/// Adds a phone book section (operator or admin).
#[utoipa::path(post, path = "/api/v1/phonebook-sections", tag = "phones", request_body = SectionInput, responses((status = 200, body = PhonebookSection)))]
pub async fn create_section(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<SectionInput>,
) -> ApiResult<Json<PhonebookSection>> {
    auth.require(Role::Operator)?;
    let section = phones::create_section(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "phonebook_section",
        Some(section.id.to_string()),
        json!({"name": section.name}),
    )
    .await?;
    Ok(Json(section))
}

/// Renames a phone book section (operator or admin).
#[utoipa::path(put, path = "/api/v1/phonebook-sections/{id}", tag = "phones", params(("id" = Uuid, Path)), request_body = SectionInput, responses((status = 200, body = PhonebookSection)))]
pub async fn update_section(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<SectionInput>,
) -> ApiResult<Json<PhonebookSection>> {
    auth.require(Role::Operator)?;
    let section = phones::update_section(&state.db, auth.tenant, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "phonebook_section",
        Some(id.to_string()),
        json!({"name": section.name}),
    )
    .await?;
    Ok(Json(section))
}

/// Deletes a phone book section with its contacts (operator or admin).
#[utoipa::path(delete, path = "/api/v1/phonebook-sections/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_section(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Operator)?;
    let section = phones::get_section(&state.db, auth.tenant, id).await?;
    phones::delete_section(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "phonebook_section",
        Some(id.to_string()),
        json!({"name": section.name}),
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

/// Imports contacts from a CSV file (operator or admin). With `dry_run` the
/// file is only checked and the report says what would happen.
#[utoipa::path(post, path = "/api/v1/contacts/import", tag = "phones", request_body = ImportRequest, responses((status = 200, body = ImportReport)))]
pub async fn import_contacts(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<ImportRequest>,
) -> ApiResult<Json<ImportReport>> {
    auth.require(Role::Operator)?;
    let report = contact_import::import(&state.db, auth.tenant, &input).await?;
    if !input.dry_run {
        audit::record(
            &state.db,
            &auth.actor(),
            "import",
            "contact",
            None,
            json!({"created": report.created, "updated": report.updated,
                   "failed": report.failed, "sections_created": report.sections_created}),
        )
        .await?;
    }
    Ok(Json(report))
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
