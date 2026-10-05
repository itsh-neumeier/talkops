//! Extensions, devices and the self-service view of a user's own phones.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::extensions::{self, Device, DeviceInput, Extension, ExtensionInput};
use talkops_core::users::Role;
use talkops_core::{phones, settings};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::fsxml::SIP_DOMAIN;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_extensions, create_extension))
        .routes(routes!(get_extension, update_extension, delete_extension))
        .routes(routes!(list_devices, create_device))
        .routes(routes!(update_device, delete_device))
        .routes(routes!(device_credentials))
        .routes(routes!(reset_device_password))
        .routes(routes!(my_phones))
        .routes(routes!(update_call_settings))
}

/// Settings users change themselves (also via feature codes on the phone).
#[derive(Deserialize, ToSchema)]
pub struct CallSettings {
    /// Do not disturb (`*78` / `*79`).
    pub dnd: bool,
    /// Unconditional forwarding target (`*72<number>` / `*73`); empty = off.
    #[serde(default)]
    pub forward_all: Option<String>,
}

/// Rejects account slots the phone model does not have.
async fn check_phone_slot(state: &AppState, auth: &AuthUser, input: &DeviceInput) -> ApiResult<()> {
    let (Some(phone_id), Some(slot)) = (input.phone_id, input.account_index) else {
        return Ok(());
    };
    let phone = phones::get(&state.db, auth.tenant, phone_id).await?;
    let max = state
        .phone_catalog
        .get(&phone.model)
        .map_or(1, |m| m.accounts);
    if slot < 1 || slot as u16 > max {
        return Err(ApiError::BadRequest(format!(
            "account slot must be between 1 and {max}"
        )));
    }
    Ok(())
}

/// SIP credentials of a device, for manual phone setup.
#[derive(Serialize, ToSchema)]
pub struct DeviceCredentials {
    pub device_id: Uuid,
    pub sip_username: String,
    pub sip_password: String,
    /// SIP domain/realm phones may use (any host name or IP of the server works too).
    pub sip_domain: &'static str,
}

#[derive(Serialize, ToSchema)]
pub struct ExtensionWithDevices {
    #[serde(flatten)]
    pub extension: Extension,
    pub devices: Vec<Device>,
}

async fn emergency_numbers(state: &AppState, auth: &AuthUser) -> ApiResult<Vec<String>> {
    Ok(settings::get(&state.db, auth.tenant)
        .await?
        .emergency_numbers)
}

/// May `auth` see/manage the extension? Admins always; users only their own.
async fn check_owner(
    state: &AppState,
    auth: &AuthUser,
    ext: &Extension,
    write: bool,
) -> ApiResult<()> {
    let needed = if write { Role::Admin } else { Role::Operator };
    if auth.role >= needed || ext.user_id == Some(auth.id) {
        let _ = state;
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

/// Lists all extensions (operator or admin).
#[utoipa::path(get, path = "/api/v1/extensions", tag = "extensions", responses((status = 200, body = [Extension])))]
pub async fn list_extensions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Extension>>> {
    auth.require(Role::Operator)?;
    Ok(Json(extensions::list(&state.db, auth.tenant).await?))
}

/// Creates an extension (admin).
#[utoipa::path(post, path = "/api/v1/extensions", tag = "extensions", request_body = ExtensionInput, responses((status = 200, body = Extension)))]
pub async fn create_extension(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<ExtensionInput>,
) -> ApiResult<Json<Extension>> {
    auth.require(Role::Admin)?;
    let emergency = emergency_numbers(&state, &auth).await?;
    let ext = extensions::create(&state.db, auth.tenant, &input, &emergency).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "extension",
        Some(ext.id.to_string()),
        json!({"number": ext.number}),
    )
    .await?;
    Ok(Json(ext))
}

/// Returns an extension with its devices.
#[utoipa::path(get, path = "/api/v1/extensions/{id}", tag = "extensions", params(("id" = Uuid, Path)), responses((status = 200, body = ExtensionWithDevices)))]
pub async fn get_extension(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ExtensionWithDevices>> {
    let extension = extensions::get(&state.db, auth.tenant, id).await?;
    check_owner(&state, &auth, &extension, false).await?;
    let devices = extensions::list_devices(&state.db, auth.tenant, id).await?;
    Ok(Json(ExtensionWithDevices { extension, devices }))
}

/// Updates an extension (admin).
#[utoipa::path(put, path = "/api/v1/extensions/{id}", tag = "extensions", params(("id" = Uuid, Path)), request_body = ExtensionInput, responses((status = 200, body = Extension)))]
pub async fn update_extension(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<ExtensionInput>,
) -> ApiResult<Json<Extension>> {
    auth.require(Role::Admin)?;
    let emergency = emergency_numbers(&state, &auth).await?;
    let ext = extensions::update(&state.db, auth.tenant, id, &input, &emergency).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "extension",
        Some(id.to_string()),
        json!({"number": ext.number, "enabled": ext.enabled}),
    )
    .await?;
    Ok(Json(ext))
}

/// Deletes an extension and its devices (admin).
#[utoipa::path(delete, path = "/api/v1/extensions/{id}", tag = "extensions", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_extension(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let ext = extensions::get(&state.db, auth.tenant, id).await?;
    extensions::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "extension",
        Some(id.to_string()),
        json!({"number": ext.number}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the devices of an extension.
#[utoipa::path(get, path = "/api/v1/extensions/{id}/devices", tag = "extensions", params(("id" = Uuid, Path)), responses((status = 200, body = [Device])))]
pub async fn list_devices(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<Device>>> {
    let ext = extensions::get(&state.db, auth.tenant, id).await?;
    check_owner(&state, &auth, &ext, false).await?;
    Ok(Json(
        extensions::list_devices(&state.db, auth.tenant, id).await?,
    ))
}

/// Adds a device with a generated SIP password (admin). The response contains
/// the credentials once; they can be shown again via the credentials endpoint.
#[utoipa::path(post, path = "/api/v1/extensions/{id}/devices", tag = "extensions", params(("id" = Uuid, Path)), request_body = DeviceInput, responses((status = 200, body = DeviceCredentials)))]
pub async fn create_device(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<DeviceInput>,
) -> ApiResult<Json<DeviceCredentials>> {
    auth.require(Role::Admin)?;
    check_phone_slot(&state, &auth, &input).await?;
    let ext = extensions::get(&state.db, auth.tenant, id).await?;
    let (device, password) =
        extensions::create_device(&state.db, auth.tenant, &state.secrets, &ext, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "device",
        Some(device.id.to_string()),
        json!({"extension": ext.number, "sip_username": device.sip_username}),
    )
    .await?;
    Ok(Json(DeviceCredentials {
        device_id: device.id,
        sip_username: device.sip_username,
        sip_password: password,
        sip_domain: SIP_DOMAIN,
    }))
}

/// Updates a device (admin).
#[utoipa::path(put, path = "/api/v1/devices/{id}", tag = "extensions", params(("id" = Uuid, Path)), request_body = DeviceInput, responses((status = 200, body = Device)))]
pub async fn update_device(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<DeviceInput>,
) -> ApiResult<Json<Device>> {
    auth.require(Role::Admin)?;
    check_phone_slot(&state, &auth, &input).await?;
    let device = extensions::update_device(&state.db, auth.tenant, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "device",
        Some(id.to_string()),
        json!({"enabled": device.enabled}),
    )
    .await?;
    Ok(Json(device))
}

/// Deletes a device (admin).
#[utoipa::path(delete, path = "/api/v1/devices/{id}", tag = "extensions", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_device(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let device = extensions::get_device(&state.db, auth.tenant, id).await?;
    extensions::delete_device(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "device",
        Some(id.to_string()),
        json!({"sip_username": device.sip_username}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Shows the SIP credentials of a device (admin or the owning user). Audited.
#[utoipa::path(get, path = "/api/v1/devices/{id}/credentials", tag = "extensions", params(("id" = Uuid, Path)), responses((status = 200, body = DeviceCredentials)))]
pub async fn device_credentials(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DeviceCredentials>> {
    let device = extensions::get_device(&state.db, auth.tenant, id).await?;
    let ext = extensions::get(&state.db, auth.tenant, device.extension_id).await?;
    check_owner(&state, &auth, &ext, true).await?;
    let password = state
        .secrets
        .decrypt(&device.sip_password_enc)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    audit::record(
        &state.db,
        &auth.actor(),
        "view_credentials",
        "device",
        Some(id.to_string()),
        json!({"sip_username": device.sip_username}),
    )
    .await?;
    Ok(Json(DeviceCredentials {
        device_id: device.id,
        sip_username: device.sip_username,
        sip_password: password,
        sip_domain: SIP_DOMAIN,
    }))
}

/// Generates a new SIP password for a device (admin or the owning user).
#[utoipa::path(post, path = "/api/v1/devices/{id}/reset-password", tag = "extensions", params(("id" = Uuid, Path)), responses((status = 200, body = DeviceCredentials)))]
pub async fn reset_device_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DeviceCredentials>> {
    let device = extensions::get_device(&state.db, auth.tenant, id).await?;
    let ext = extensions::get(&state.db, auth.tenant, device.extension_id).await?;
    check_owner(&state, &auth, &ext, true).await?;
    let password =
        extensions::reset_device_password(&state.db, auth.tenant, &state.secrets, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "reset_password",
        "device",
        Some(id.to_string()),
        json!({"sip_username": device.sip_username}),
    )
    .await?;
    Ok(Json(DeviceCredentials {
        device_id: device.id,
        sip_username: device.sip_username,
        sip_password: password,
        sip_domain: SIP_DOMAIN,
    }))
}

/// The logged-in user's extensions and devices (self-service).
#[utoipa::path(get, path = "/api/v1/me/phones", tag = "extensions", responses((status = 200, body = [ExtensionWithDevices])))]
pub async fn my_phones(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<ExtensionWithDevices>>> {
    let mut out = Vec::new();
    for extension in extensions::list_for_user(&state.db, auth.tenant, auth.id).await? {
        let devices = extensions::list_devices(&state.db, auth.tenant, extension.id).await?;
        out.push(ExtensionWithDevices { extension, devices });
    }
    Ok(Json(out))
}

/// Sets DND and unconditional forwarding (admin or the owning user).
#[utoipa::path(put, path = "/api/v1/extensions/{id}/call-settings", tag = "extensions", params(("id" = Uuid, Path)), request_body = CallSettings, responses((status = 200, body = Extension)))]
pub async fn update_call_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<CallSettings>,
) -> ApiResult<Json<Extension>> {
    let ext = extensions::get(&state.db, auth.tenant, id).await?;
    check_owner(&state, &auth, &ext, true).await?;
    let forward = input
        .forward_all
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty());
    if forward == Some(ext.number.as_str()) {
        return Err(ApiError::BadRequest(
            "an extension cannot forward to itself".into(),
        ));
    }
    let mut tx = state.db.begin().await?;
    extensions::set_dnd(&mut *tx, auth.tenant, id, input.dnd).await?;
    extensions::set_forward_all(&mut *tx, auth.tenant, id, forward).await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update_call_settings",
        "extension",
        Some(id.to_string()),
        json!({"number": ext.number, "dnd": input.dnd, "forward_all": forward}),
    )
    .await?;
    Ok(Json(extensions::get(&state.db, auth.tenant, id).await?))
}
