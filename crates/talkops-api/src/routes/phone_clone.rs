//! Copy validated configuration, never a phone's identity or account credentials.
use std::collections::BTreeSet;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use talkops_core::{
    audit,
    extensions::{self, DeviceInput, DeviceKind},
    phones::{self, Phone, PhoneInput},
    roles::Permission,
};
use talkops_provisioning::{
    keys,
    yealink::{KeyType, LineKey},
};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::AuthUser,
    error::{ApiError, ApiResult},
};

#[derive(Serialize, ToSchema)]
pub struct ClonePreview {
    pub source_id: Uuid,
    pub source_name: String,
    pub source_revision: String,
    pub configuration: PhoneInput,
    /// Field paths excluded from the copy, without any field values.
    pub excluded: Vec<String>,
    pub http_exception: bool,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub account_index: i16,
    pub extension_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CloneInput {
    pub source_revision: String,
    #[serde(default)]
    pub mac: String,
    pub name: String,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
}

/// Derive the draft from known fields. New fields must opt in to copying.
fn draft(state: &AppState, source: &Phone) -> ApiResult<ClonePreview> {
    let model = state
        .phone_catalog
        .get(&source.model)
        .ok_or(ApiError::NotFound)?;
    let mut excluded = vec![
        "id",
        "mac",
        "name",
        "sip_credentials",
        "account_assignments",
        "account_labels",
        "last_seen_at",
        "last_ip",
        "last_firmware",
        "diagnostics",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let source_specific = |value: &str| {
        let value = value.to_ascii_lowercase();
        let compact: String = value.chars().filter(|c| !matches!(c, ':' | '-')).collect();
        compact.contains(&source.mac.to_ascii_lowercase()) || value.contains(&source.id.to_string())
    };
    let mut settings = serde_json::Map::new();
    for (key, value) in source.settings.as_object().into_iter().flatten() {
        let allowed = state.phone_catalog.settings().iter().any(|setting| {
            setting.key == *key
                && setting.vendor == model.vendor
                && (setting.models.is_empty() || setting.models.contains(&model.id))
                && setting.families.contains(&model.family)
                && value
                    .as_str()
                    .is_some_and(|v| setting.allows(v) && !source_specific(v))
        });
        if allowed {
            settings.insert(key.clone(), value.clone());
        } else {
            excluded.push(format!("settings.{key}"));
        }
    }
    let mut provisioning: keys::Overrides = serde_json::from_value(source.provisioning.clone())
        .map_err(|_| ApiError::BadRequest("source has unknown provisioning fields".into()))?;
    let effective = keys::effective_model(model, &provisioning).map_err(ApiError::BadRequest)?;
    let model = &effective;
    provisioning.defer_unassigned_keys = true;
    let fields = talkops_provisioning::parameters::schema(model).map_err(ApiError::BadRequest)?;
    provisioning.parameters.retain(|key, value| {
        let keep = !value.starts_with("v1:")
            && !talkops_provisioning::parameters::url_contains_secret(value)
            && fields
                .iter()
                .any(|f| f.key == *key && f.copyable && f.kind != "secret")
            && !source_specific(value);
        if !keep {
            excluded.push(format!("provisioning.parameters.{key}"));
        }
        keep
    });
    provisioning.vendor_settings.retain(|key, value| {
        let copyable = talkops_provisioning::vendors::profile(&model.id).is_some_and(|profile| {
            profile
                .settings
                .iter()
                .any(|s| s.key == *key && s.copyable && s.kind != "secret")
        });
        let keep = copyable
            && !value.starts_with("v1:")
            && !talkops_provisioning::parameters::url_contains_secret(value)
            && !source_specific(value);
        if !keep {
            excluded.push(format!("provisioning.vendor_settings.{key}"));
        }
        keep
    });
    let mut line_keys: Vec<LineKey> = serde_json::from_value(source.line_keys.clone())
        .map_err(|_| ApiError::BadRequest("source has invalid line keys".into()))?;
    line_keys.retain(|key| {
        let keep = key.kind != KeyType::XmlBrowser
            && !talkops_provisioning::parameters::url_contains_secret(&key.value)
            && !source_specific(&key.value);
        if !keep {
            excluded.push(format!("line_keys.{}", key.key));
        }
        keep
    });
    provisioning.keys.retain(|key| {
        let native_url = key.native_function.as_deref().is_some_and(|id| {
            talkops_provisioning::vendors::functions(&model.vendor)
                .iter()
                .any(|f| {
                    f.uuid == id
                        && ["url", "xml", "browser", "http"]
                            .iter()
                            .any(|s| f.kind.contains(s))
                })
        });
        let keep = key.kind != KeyType::XmlBrowser
            && !native_url
            && !talkops_provisioning::parameters::url_contains_secret(&key.value)
            && !source_specific(&key.value);
        if !keep {
            excluded.push(format!("provisioning.keys.{}.{}", key.group, key.key));
        }
        keep
    });
    if source_specific(&provisioning.endpoint) {
        provisioning.endpoint.clear();
        excluded.push("provisioning.endpoint".into());
    }
    if source_specific(&provisioning.sip_host) {
        provisioning.sip_host.clear();
        excluded.push("provisioning.sip_host".into());
    }
    let http_exception = provisioning.endpoint.starts_with("http://");
    let configuration = PhoneInput {
        mac: String::new(),
        name: String::new(),
        model: source.model.clone(),
        line_keys: json!(line_keys),
        ringtone_id: source.ringtone_id,
        wallpaper_id: source.wallpaper_id,
        phonebook_sections: source.phonebook_sections.clone(),
        settings: json!(settings),
        provisioning: Some(json!(provisioning)),
    };
    // Exclude volatile telemetry, but include every source field that affects the draft.
    let source_revision = hex::encode(Sha256::digest(
        serde_json::to_vec(&json!({
            "model":source.model,"mac":source.mac, "line_keys":source.line_keys, "settings":source.settings,
            "provisioning":source.provisioning, "ringtone_id":source.ringtone_id,
            "wallpaper_id":source.wallpaper_id, "phonebook_sections":source.phonebook_sections,
        }))
        .map_err(|e| ApiError::Internal(e.to_string()))?,
    ));
    Ok(ClonePreview {
        source_id: source.id,
        source_name: source.name.clone(),
        source_revision,
        configuration,
        excluded,
        http_exception,
    })
}

#[utoipa::path(post, path="/api/v1/phones/{id}/clone-preview",tag="phones",params(("id"=Uuid,Path)),responses((status=200,body=ClonePreview)))]
pub async fn preview(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ClonePreview>> {
    auth.require_perm(Permission::PhonesManage)?;
    let source = phones::get(&state.db, auth.tenant, id).await?;
    let preview = draft(&state, &source)?;
    super::phones::validate_phone(&state, &preview.configuration, None)?;
    super::phones::validate_media(&state, &auth, &preview.configuration).await?;
    phones::check_references(
        &mut *state.db.acquire().await?,
        auth.tenant,
        &preview.configuration,
    )
    .await?;
    let overrides: keys::Overrides = serde_json::from_value(source.provisioning.clone())
        .map_err(|_| ApiError::BadRequest("invalid source provisioning settings".into()))?;
    super::provisioning_admin::template_files(
        &state,
        auth.tenant,
        &source.model,
        overrides.template_id.as_deref(),
    )
    .await?;
    Ok(Json(preview))
}

#[utoipa::path(post, path="/api/v1/phones/{id}/clone",tag="phones",params(("id"=Uuid,Path)),request_body=CloneInput,responses((status=200,body=Phone),(status=409,description="Source changed or identity already exists")))]
pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<CloneInput>,
) -> ApiResult<Json<Phone>> {
    auth.require_perm(Permission::PhonesManage)?;
    if !input.assignments.is_empty() {
        auth.require_perm(Permission::ExtensionsManage)?;
    }
    let source = phones::get(&state.db, auth.tenant, id).await?;
    let preview = draft(&state, &source)?;
    if input.source_revision != preview.source_revision {
        return Err(ApiError::Conflict(
            "source configuration changed; reload the copy preview".into(),
        ));
    }
    let mut configuration = preview.configuration;
    configuration.mac = input.mac;
    if configuration.mac.is_empty()
        && state
            .phone_catalog
            .get(&configuration.model)
            .is_some_and(|m| m.family == talkops_provisioning::PhoneFamily::App)
    {
        configuration.mac = Uuid::new_v4().simple().to_string()[..12].into();
    }
    configuration.name = input.name;
    super::phones::validate_phone(&state, &configuration, None)?;
    super::phones::validate_media(&state, &auth, &configuration).await?;
    let model = state
        .phone_catalog
        .get(&configuration.model)
        .ok_or(ApiError::NotFound)?;
    let overrides: keys::Overrides =
        serde_json::from_value(configuration.provisioning.clone().unwrap_or_default())
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let effective = keys::effective_model(model, &overrides).map_err(ApiError::BadRequest)?;
    let model = &effective;
    super::provisioning_admin::template_files(
        &state,
        auth.tenant,
        &model.id,
        overrides.template_id.as_deref(),
    )
    .await?;
    let mut slots = BTreeSet::new();
    for assignment in &input.assignments {
        if assignment.account_index < 1
            || assignment.account_index as u16 > model.accounts
            || !slots.insert(assignment.account_index)
        {
            return Err(ApiError::BadRequest(
                "invalid or duplicate account slot".into(),
            ));
        }
    }
    // Finish pool-based validation before reserving a transaction connection.
    // Recheck the source under its lock so a concurrent edit cannot change the copy.
    let mut tx = state.db.begin().await?;
    let _: Uuid =
        sqlx::query_scalar("SELECT id FROM phones WHERE tenant_id=$1 AND id=$2 FOR SHARE")
            .bind(auth.tenant)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let locked_source = phones::get(&mut *tx, auth.tenant, id).await?;
    if draft(&state, &locked_source)?.source_revision != input.source_revision {
        return Err(ApiError::Conflict(
            "source configuration changed; reload the copy preview".into(),
        ));
    }
    let phone = phones::create_on(&mut tx, auth.tenant, &configuration).await?;
    for assignment in &input.assignments {
        let extension = extensions::get(&mut *tx, auth.tenant, assignment.extension_id).await?;
        let device = DeviceInput {
            name: phone.name.clone(),
            kind: match model.family {
                talkops_provisioning::PhoneFamily::Dect => DeviceKind::Dect,
                talkops_provisioning::PhoneFamily::Wifi => DeviceKind::Wifi,
                talkops_provisioning::PhoneFamily::App => DeviceKind::Softphone,
                talkops_provisioning::PhoneFamily::Intercom => DeviceKind::Door,
                talkops_provisioning::PhoneFamily::Ata
                | talkops_provisioning::PhoneFamily::Gateway
                | talkops_provisioning::PhoneFamily::Paging => DeviceKind::Other,
                _ => DeviceKind::Desk,
            },
            sip_username: None,
            phone_id: Some(phone.id),
            account_index: Some(assignment.account_index),
            phone_label: String::new(),
            phone_display_name: String::new(),
            codec_profile: None,
            video: None,
            sip_password_format: None,
            enabled: true,
        };
        let (device, _) =
            extensions::create_device_on(&mut tx, auth.tenant, &state.secrets, &extension, &device)
                .await?;
        audit::record(
            &mut *tx,
            &auth.actor(),
            "create",
            "device",
            Some(device.id.to_string()),
            json!({"extension":extension.number,"phone_id":phone.id}),
        )
        .await?;
    }
    audit::record(
        &mut *tx,
        &auth.actor(),
        "clone",
        "phone",
        Some(phone.id.to_string()),
        json!({"source_id":id,"model":phone.model,"accounts":slots.len()}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(phone))
}
