//! Phone auto-provisioning (`/provisioning/...`), fetched by the phones
//! themselves: Yealink configuration files, XML phonebooks, firmware images
//! and action-URL events.
//!
//! Phones authenticate with HTTP basic auth (credentials from DHCP option 66
//! or the phone's auto-provisioning settings). Action URLs cannot carry basic
//! auth on all firmware versions, so `events` also accepts `key=<password>`.

use super::provisioning_admin;
use std::collections::{BTreeMap, HashMap};
use talkops_provisioning::{adapter, keys};

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use talkops_core::extensions::CodecProfile;
use talkops_core::phones::{self, Phone};
use talkops_core::tenant::TenantId;
use talkops_core::{extensions, numbering, settings};
use talkops_provisioning::SettingValues;
use talkops_provisioning::dial_now::{self, DialNowInput};
use talkops_provisioning::phonebook::{self, Entry};
use talkops_provisioning::yealink::{
    self, Account, AccountCodecs, LineKey, Locale, MediaFile, PhoneSetup, Provisioning,
    RemotePhonebook,
};
use talkops_provisioning::{catalog::firmware_from_user_agent, normalize_mac};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{client_ip, is_https};
use crate::error::{ApiError, ApiResult};
use crate::util::constant_time_eq;

/// Feature code phones dial for voicemail (Phase 3).
pub const VOICEMAIL_CODE: &str = "*97";

/// Provisioning base URL as seen by the client (`http://host:port/provisioning`).
pub fn base_url(headers: &HeaderMap) -> String {
    let scheme = if is_https(headers) { "https" } else { "http" };
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .filter(|h| {
            h.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
        })
        .unwrap_or("localhost");
    format!("{scheme}://{host}/provisioning")
}

/// Host part of the `Host` header (without port), used as SIP server address.
fn sip_host(headers: &HeaderMap) -> String {
    let url = base_url(headers);
    let host = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .split('/')
        .next()
        .unwrap_or_default();
    if let Some(v6) = host.strip_prefix('[') {
        return format!("[{}]", v6.split(']').next().unwrap_or_default());
    }
    host.split(':').next().unwrap_or_default().to_owned()
}

async fn provisioning(state: &AppState, headers: &HeaderMap) -> ApiResult<(Provisioning, String)> {
    let secrets =
        phones::provisioning_secrets(&state.db, TenantId::DEFAULT, &state.secrets, false).await?;
    Ok((
        Provisioning {
            base_url: configured_base(state, TenantId::DEFAULT, headers).await?,
            username: secrets.username,
            password: secrets.password,
        },
        secrets.phone_admin_password,
    ))
}

fn basic_auth(headers: &HeaderMap) -> Option<Vec<u8>> {
    let encoded = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Basic ")?;
    STANDARD.decode(encoded.trim()).ok()
}

fn text(content_type: &'static str, body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

/// Download URL and file name of an uploaded ringtone or wallpaper.
async fn media_file(
    state: &AppState,
    tenant: TenantId,
    auth_base: &str,
    id: Option<Uuid>,
) -> ApiResult<Option<MediaFile>> {
    let Some(id) = id else {
        return Ok(None);
    };
    let m = phones::get_media(&state.db, tenant, id).await?;
    Ok(Some(MediaFile {
        url: format!("{auth_base}/media/{}/{}", m.id, m.filename),
        filename: m.filename,
    }))
}

/// Renders the per-phone configuration file.
pub async fn render_phone_config(
    state: &AppState,
    tenant: TenantId,
    phone: &Phone,
    headers: &HeaderMap,
) -> ApiResult<String> {
    render_phone_config_inner(state, tenant, phone, headers, RenderOptions::default()).await
}
pub async fn render_phone_artifact(
    state: &AppState,
    tenant: TenantId,
    phone: &Phone,
    headers: &HeaderMap,
    artifact: &str,
) -> ApiResult<String> {
    render_phone_config_inner(
        state,
        tenant,
        phone,
        headers,
        RenderOptions {
            artifact: Some(artifact),
            ..Default::default()
        },
    )
    .await
}
pub async fn preview_with_files(
    state: &AppState,
    tenant: TenantId,
    phone: &Phone,
    files: &BTreeMap<String, String>,
) -> ApiResult<String> {
    render_phone_config_inner(
        state,
        tenant,
        phone,
        &HeaderMap::new(),
        RenderOptions {
            redacted: true,
            files_override: Some(files),
            ..Default::default()
        },
    )
    .await
}
pub async fn preview_phone(
    state: &AppState,
    tenant: TenantId,
    phone: &Phone,
    headers: &HeaderMap,
) -> ApiResult<String> {
    render_phone_config_inner(
        state,
        tenant,
        phone,
        headers,
        RenderOptions {
            redacted: true,
            ..Default::default()
        },
    )
    .await
}
pub async fn preview_phone_accounts(
    state: &AppState,
    tenant: TenantId,
    phone: &Phone,
    headers: &HeaderMap,
    labels: &BTreeMap<String, (String, String)>,
) -> ApiResult<String> {
    render_phone_config_inner(
        state,
        tenant,
        phone,
        headers,
        RenderOptions {
            redacted: true,
            labels: Some(labels),
            ..Default::default()
        },
    )
    .await
}
#[derive(Default)]
struct RenderOptions<'a> {
    redacted: bool,
    files_override: Option<&'a BTreeMap<String, String>>,
    labels: Option<&'a BTreeMap<String, (String, String)>>,
    artifact: Option<&'a str>,
}
async fn render_phone_config_inner(
    state: &AppState,
    tenant: TenantId,
    phone: &Phone,
    headers: &HeaderMap,
    options: RenderOptions<'_>,
) -> ApiResult<String> {
    let RenderOptions {
        redacted,
        files_override,
        labels,
        artifact,
    } = options;
    let model = state
        .phone_catalog
        .get(&phone.model)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown phone model {}", phone.model)))?;
    let mut overrides: keys::Overrides = serde_json::from_value(phone.provisioning.clone())
        .map_err(|_| ApiError::Internal("invalid phone provisioning overrides".into()))?;
    let effective_model = keys::effective_model(model, &overrides).map_err(ApiError::BadRequest)?;
    let model = &effective_model;
    keys::validate(model, &overrides).map_err(ApiError::BadRequest)?;
    let fields = talkops_provisioning::parameters::schema(model).map_err(ApiError::BadRequest)?;
    for (group, values) in [
        ("parameters", &mut overrides.parameters),
        ("vendor_settings", &mut overrides.vendor_settings),
    ] {
        for (key, value) in values {
            let declared = if group == "parameters" {
                fields.iter().any(|f| f.key == *key && f.kind == "secret")
            } else {
                talkops_provisioning::vendors::profile(&model.id).is_some_and(|p| {
                    p.settings
                        .iter()
                        .any(|s| s.key == *key && s.kind == "secret")
                })
            };
            let encrypted = value.starts_with("v1:");
            if redacted
                && (encrypted
                    || declared
                    || talkops_provisioning::parameters::url_contains_secret(value))
            {
                *value = "REDACTED".into();
            } else if encrypted {
                *value = state.secrets.decrypt(value).map_err(|_| {
                    ApiError::Internal("invalid encrypted manufacturer setting".into())
                })?;
            }
        }
    }
    let options = provisioning_admin::options(state, tenant).await?;
    let templates = match files_override {
        Some(files) => files.clone(),
        None => {
            provisioning_admin::template_files(
                state,
                tenant,
                &phone.model,
                overrides.template_id.as_deref(),
            )
            .await?
        }
    };
    let (mut prov, mut admin_password) = provisioning(state, headers).await?;
    prov.base_url = if !overrides.endpoint.is_empty() {
        overrides.endpoint.trim_end_matches('/').into()
    } else if !options.endpoint.is_empty() {
        options.endpoint.trim_end_matches('/').into()
    } else {
        base_url(headers)
    };
    let directory_token = if redacted {
        "REDACTED".to_owned()
    } else {
        cisco_directory_token(&prov.password, phone.id)
    };
    if redacted {
        prov.password = "REDACTED".into();
        admin_password = "REDACTED".into();
    }
    let mut accounts = Vec::new();
    for mut a in phones::accounts(&state.db, phone.id).await? {
        if let Some((label, display)) =
            labels.and_then(|labels| labels.get(&a.device_id.to_string()))
        {
            a.phone_label = label.clone();
            a.phone_display_name = display.clone();
        }
        let password = state
            .secrets
            .decrypt(&a.sip_password_enc)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        accounts.push(Account {
            index: a.account_index.max(1) as u16,
            label: a.label().to_owned(),
            display_name: a.caller_name().to_owned(),
            codecs: if a.effective_codec_profile() == CodecProfile::Standard {
                AccountCodecs::PhoneDefault
            } else {
                AccountCodecs::Profiles
            },
            username: a.sip_username,
            password: if redacted {
                "REDACTED".into()
            } else {
                password
            },
        });
    }
    let keys: Vec<LineKey> = serde_json::from_value(phone.line_keys.clone()).unwrap_or_default();
    let firmware_url = phones::active_firmware(&state.db, tenant, &phone.model)
        .await?
        .map(|fw| {
            format!(
                "{}/firmware/{}/{}",
                prov.authenticated_base(),
                fw.id,
                fw.filename
            )
        });
    let auth_base = prov.authenticated_base();
    let ringtone = media_file(state, tenant, &auth_base, phone.ringtone_id).await?;
    let wallpaper = media_file(state, tenant, &auth_base, phone.wallpaper_id).await?;
    let mut phonebooks = Vec::new();
    for id in phone
        .phonebook_sections
        .iter()
        .take(phones::MAX_PHONE_SECTIONS)
    {
        if let Ok(section) = phones::get_section(&state.db, tenant, *id).await {
            phonebooks.push(RemotePhonebook {
                name: section.name,
                url: format!("{auth_base}/phonebook/section/{id}.xml"),
            });
        }
    }
    let plan = settings::get(&state.db, tenant).await?.dial_plan();
    let internal = numbering::all_numbers(&state.db, tenant).await?;
    let dial_now = dial_now::rules(&DialNowInput {
        emergency: &plan.emergency_numbers,
        fixed_codes: numbering::FIXED_FEATURE_CODES,
        code_prefixes: numbering::FEATURE_CODE_PREFIXES,
        internal: &internal,
        local_dialing: !plan.area_code.is_empty(),
    });
    let mut setup = PhoneSetup {
        provisioning: overrides.clone(),
        asset_base: auth_base.clone(),
        name: phone.name.clone(),
        mac: phone.mac.clone(),
        model,
        accounts,
        keys,
        sip_host: if !overrides.sip_host.is_empty() {
            overrides.sip_host.clone()
        } else if !options.sip_host.is_empty() {
            options.sip_host.clone()
        } else {
            sip_host(headers)
        },
        sip_port: state.profile.internal_port,
        firmware_url,
        voicemail_code: VOICEMAIL_CODE.to_owned(),
        ringtone,
        wallpaper,
        phonebooks,
        dial_now,
        settings: state.phone_catalog.effective_settings(
            model,
            &setting_values(&phones::phone_defaults(&state.db, tenant).await?),
            &setting_values(&phone.settings),
        ),
    };
    if setup.provisioning.defer_unassigned_keys {
        for key in &mut setup.keys {
            if keys::requires_account(key.kind)
                && !setup.accounts.iter().any(|a| a.index == key.account)
            {
                key.kind = talkops_provisioning::yealink::KeyType::None;
                key.value.clear();
                key.label.clear();
            }
        }
        for key in &mut setup.provisioning.keys {
            if (key.native_function.is_some() || keys::requires_account(key.kind))
                && !setup.accounts.iter().any(|a| a.index == key.account)
            {
                key.native_function = None;
                key.kind = talkops_provisioning::yealink::KeyType::None;
                key.value.clear();
                key.label.clear();
            }
        }
    }
    let settings = settings::get(&state.db, tenant).await?;
    let language = if !overrides.language.is_empty() {
        &overrides.language
    } else if !options.language.is_empty() {
        &options.language
    } else {
        &settings.default_language
    };
    let time_zone = if !overrides.time_zone.is_empty() {
        &overrides.time_zone
    } else if !options.time_zone.is_empty() {
        &options.time_zone
    } else {
        &settings.timezone
    };
    let ntp_server = if !overrides.ntp_server.is_empty() {
        overrides.ntp_server.as_str()
    } else if !options.ntp_server.is_empty() {
        &options.ntp_server
    } else {
        "pool.ntp.org"
    };
    if talkops_provisioning::vendors::profile(&model.id).is_some() {
        let entries = phone_entries(state, tenant, phone).await?;
        let directory = if talkops_provisioning::vendors::cisco_enterprise(&model.id) {
            cisco_directory_url(&prov.base_url, phone.id, &directory_token)
        } else {
            String::new()
        };
        let cisco_locales = if talkops_provisioning::vendors::cisco_enterprise(&model.id) {
            crate::cisco_locale::list(&state.provisioning_dir).await
        } else {
            Vec::new()
        };
        let common = talkops_provisioning::vendors::Common {
            cisco_locale: talkops_provisioning::vendors::select_cisco_locale(
                &cisco_locales,
                overrides
                    .vendor_settings
                    .get("cisco_enterprise_locale")
                    .map_or("", String::as_str),
                language,
            ),
            provisioning: &prov,
            admin_password: &admin_password,
            timezone: time_zone,
            language,
            ntp_server,
            contacts: &entries,
            directory_url: &directory,
        };
        let profile =
            talkops_provisioning::vendors::profile(&model.id).ok_or(ApiError::NotFound)?;
        let artifact = artifact
            .or(profile.primary.as_deref())
            .ok_or(ApiError::NotFound)?;
        let format = &profile
            .artifacts
            .iter()
            .find(|a| a.name == artifact)
            .ok_or(ApiError::NotFound)?
            .format;
        let body =
            talkops_provisioning::vendors::render(&setup, &common, Some(artifact), &templates)
                .map_err(ApiError::BadRequest)?
                .0;
        let body = talkops_provisioning::parameters::apply(
            &body,
            artifact,
            format.starts_with("xml"),
            &overrides.parameters,
        )
        .map_err(ApiError::BadRequest)?;
        // Hash again: parameter overrides change the file after rendering.
        return Ok(
            if talkops_provisioning::vendors::cisco_enterprise(&model.id)
                && profile.primary.as_deref() == Some(artifact)
            {
                talkops_provisioning::vendors::cisco_version_stamp(&body)
            } else {
                body
            },
        );
    }
    let device = adapter::render(&setup, &templates).map_err(ApiError::BadRequest)?;
    let mut locale = Locale::new(time_zone, language);
    locale.ntp_server = ntp_server.to_owned();
    let common = yealink::render_common_with(&prov, &admin_password, &locale, language, &templates)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    if device.len() + common.len() > 524288 {
        return Err(ApiError::BadRequest(
            "configuration output is too large".into(),
        ));
    }
    // The per-device file repeats common settings with the device endpoint. A common
    // fetch cannot identify every phone, and must not overwrite an HTTP exception.
    let body = format!("{common}\n{device}");
    talkops_provisioning::parameters::apply_yealink(&body, model, &overrides.parameters)
        .map_err(ApiError::BadRequest)
}

/// Phone book entries for a phone: enabled extensions, then the global
/// contacts and the contacts of the phone's sections.
async fn phone_entries(state: &AppState, tenant: TenantId, phone: &Phone) -> ApiResult<Vec<Entry>> {
    let mut entries = extensions::list(&state.db, tenant)
        .await?
        .into_iter()
        .filter(|e| e.enabled)
        .map(|e| Entry {
            name: e.display_name,
            numbers: vec![("Extension".into(), format!("**{}", e.number))],
        })
        .collect::<Vec<_>>();
    for contact in phones::list_contacts(&state.db, tenant).await? {
        if contact
            .section_id
            .is_some_and(|id| !phone.phonebook_sections.contains(&id))
        {
            continue;
        }
        let numbers = [
            ("Work", &contact.phone_work),
            ("Mobile", &contact.phone_mobile),
            ("Other", &contact.phone_other),
        ]
        .into_iter()
        .filter(|(_, n)| !n.is_empty())
        .map(|(kind, n)| (kind.to_owned(), n.clone()))
        .collect();
        entries.push(Entry {
            name: contact.display_name(),
            numbers,
        });
    }
    Ok(entries)
}

/// Token in the Cisco directory URL. Enterprise phones cannot send the
/// provisioning credentials, so the URL carries a per-phone value derived
/// from the provisioning password. Regenerating the credentials changes it.
pub fn cisco_directory_token(provisioning_password: &str, phone: Uuid) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!(
        "talkops-cisco-directory:{phone}:{provisioning_password}"
    ));
    hex::encode(&digest[..16])
}

/// `directoryURL` for a Cisco Enterprise phone; empty if TalkOps has no
/// reachable address (TFTP export without a configured endpoint).
fn cisco_directory_url(base: &str, phone: Uuid, token: &str) -> String {
    let base = base.trim_end_matches('/');
    match url::Url::parse(base) {
        Ok(url) if url.host_str().is_some_and(|h| h != "tftp.invalid") => {
            format!("{base}/cisco/directory/{phone}/{token}")
        }
        _ => String::new(),
    }
}

/// Comfort setting values from the database (non-strings are ignored).
pub fn setting_values(value: &serde_json::Value) -> SettingValues {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn configured_base(
    state: &AppState,
    tenant: TenantId,
    headers: &HeaderMap,
) -> ApiResult<String> {
    let options = provisioning_admin::options(state, tenant).await?;
    Ok(if options.endpoint.is_empty() {
        base_url(headers)
    } else {
        options.endpoint.trim_end_matches('/').into()
    })
}

/// Record only bounded configuration filenames; never headers, URL userinfo or queries.
pub async fn serve(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    req: Request,
) -> Response {
    let scheme = if is_https(req.headers()) {
        "https"
    } else {
        "http"
    };
    let mac = path
        .strip_suffix(".cfg")
        .or_else(|| path.strip_suffix(".boot"))
        .and_then(normalize_mac)
        .or_else(|| {
            path.strip_prefix("devices/")
                .and_then(|rest| rest.split('/').next())
                .and_then(normalize_mac)
        })
        .or_else(|| {
            path.strip_prefix("softkeys/")
                .and_then(|rest| rest.split('/').next())
                .and_then(normalize_mac)
        });
    let softkey = path
        .strip_prefix("softkeys/")
        .and_then(|p| p.split_once('/'))
        .is_some_and(|(_, file)| {
            matches!(
                file,
                "call_in.xml"
                    | "call_failed.xml"
                    | "connecting.xml"
                    | "dialing.xml"
                    | "ring_back.xml"
                    | "talking.xml"
            )
        });
    let record = path.len() <= 100
        && (is_common_cfg(&path)
            || (mac.is_some() && (softkey || !path.contains('/')))
            || path == "y000000000000.boot"
            || path.starts_with("devices/")
                && mac.is_some()
                && talkops_provisioning::vendors::valid_path(&path));
    let response = serve_inner(State(state.clone()), Path(path.clone()), Query(query), req).await;
    if record {
        let tenant = match mac.as_deref() {
            Some(mac) => phones::find_by_mac(&state.db, mac)
                .await
                .ok()
                .flatten()
                .map(|p| p.0)
                .unwrap_or(TenantId::DEFAULT),
            None => TenantId::DEFAULT,
        };
        if let Err(error) = talkops_core::phone_provisioning::record_request(
            &state.db,
            tenant,
            mac.as_deref(),
            &path,
            scheme,
            response.status().as_u16(),
        )
        .await
        {
            tracing::warn!(%error,"cannot record provisioning result");
        }
    }
    response
}

/// `GET /provisioning/{*path}`
async fn serve_inner(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    req: Request,
) -> Response {
    let (parts, _) = req.into_parts();
    let headers = &parts.headers;
    let ip = client_ip(&parts);
    let limiter_key = format!("prov:{}", ip.as_deref().unwrap_or("unknown"));
    if state.limiter.blocked(&limiter_key) {
        return ApiError::TooManyRequests.into_response();
    }
    // Cisco Enterprise phones send no credentials; a per-phone token admits them.
    if let Some(rest) = path.strip_prefix("cisco/directory/") {
        return cisco_directory(&state, rest, &query, headers, &limiter_key)
            .await
            .unwrap_or_else(|err| match err {
                ApiError::NotFound => not_found(),
                other => other.into_response(),
            });
    }

    let (prov, admin_password) = match provisioning(&state, headers).await {
        Ok(p) => p,
        Err(err) => return err.into_response(),
    };
    let expected = format!("{}:{}", prov.username, prov.password);
    let authorized = match basic_auth(headers) {
        Some(given) => constant_time_eq(&given, expected.as_bytes()),
        None => false,
    } || (path == "events"
        && query
            .get("key")
            .is_some_and(|k| constant_time_eq(k.as_bytes(), prov.password.as_bytes())));
    if !authorized {
        // Phones first ask without credentials; only wrong ones count.
        let wrong = headers.contains_key(header::AUTHORIZATION) || query.contains_key("key");
        if wrong && !state.limiter.allow(&limiter_key) {
            return ApiError::TooManyRequests.into_response();
        }
        tracing::debug!(path, ip = ?ip, "provisioning request without valid credentials");
        return (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                "Basic realm=\"talkops-provisioning\"",
            )],
        )
            .into_response();
    }

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let result = match path.as_str() {
        p if p.starts_with("devices/") => native_artifact(&state, p, headers, ip.as_deref()).await,
        p if p.ends_with(".boot") => boot_file(&state, p, user_agent).await,
        p if p.starts_with("softkeys/") => softkey_file(&state, p).await,
        p if is_common_cfg(p) => common_cfg(&state, &prov, &admin_password).await,
        "phonebook/internal.xml" => internal_phonebook(&state, query.get("search")).await,
        "phonebook/contacts.xml" => contacts_phonebook(&state, None, query.get("search")).await,
        p if p.starts_with("phonebook/section/") => {
            match p
                .strip_prefix("phonebook/section/")
                .and_then(|r| r.strip_suffix(".xml"))
                .and_then(|id| id.parse::<Uuid>().ok())
            {
                Some(id) => contacts_phonebook(&state, Some(id), query.get("search")).await,
                None => return not_found(),
            }
        }
        p if p.starts_with("media/") => return media(&state, p).await,
        "events" => events(&state, &query).await,
        p if p.starts_with("firmware/") => return firmware(&state, p).await,
        p => match p.strip_suffix(".cfg").and_then(normalize_mac) {
            Some(mac) if p.len() == 16 => {
                phone_cfg(&state, &mac, headers, ip.as_deref(), user_agent).await
            }
            _ => return not_found(),
        },
    };
    result.unwrap_or_else(|err| match err {
        ApiError::NotFound => not_found(),
        other => other.into_response(),
    })
}

async fn native_artifact(
    state: &AppState,
    path: &str,
    headers: &HeaderMap,
    ip: Option<&str>,
) -> ApiResult<Response> {
    let (identity, requested) = path
        .strip_prefix("devices/")
        .and_then(|p| p.split_once('/'))
        .ok_or(ApiError::NotFound)?;
    let mac = normalize_mac(identity).ok_or(ApiError::NotFound)?;
    let (tenant, phone) = phones::find_by_mac(&state.db, &mac)
        .await?
        .ok_or(ApiError::NotFound)?;
    if tenant != TenantId::DEFAULT {
        return Err(ApiError::NotFound);
    }
    let profile = talkops_provisioning::vendors::profile(&phone.model).ok_or(ApiError::NotFound)?;
    let artifact = talkops_provisioning::vendors::artifact(profile, requested, &mac)
        .ok_or(ApiError::NotFound)?;
    let body = render_phone_artifact(state, tenant, &phone, headers, &artifact.name).await?;
    phones::record_seen(&state.db, phone.id, ip, None).await?;
    Ok(text(
        if artifact.format.starts_with("xml") {
            "application/xml; charset=utf-8"
        } else if artifact.format == "csv" {
            "text/csv; charset=utf-8"
        } else if artifact.format == "json" {
            "application/json; charset=utf-8"
        } else {
            "text/plain; charset=utf-8"
        },
        body,
    ))
}

async fn boot_file(state: &AppState, path: &str, user_agent: &str) -> ApiResult<Response> {
    let (model, mac) = if path == "y000000000000.boot" {
        let model = state
            .phone_catalog
            .from_user_agent(user_agent)
            .ok_or(ApiError::NotFound)?;
        let mac = user_agent
            .split_whitespace()
            .last()
            .and_then(normalize_mac)
            .ok_or(ApiError::NotFound)?;
        let (_, phone) = phones::find_by_mac(&state.db, &mac)
            .await?
            .ok_or(ApiError::NotFound)?;
        if phone.model != model.id {
            return Err(ApiError::NotFound);
        }
        (model, mac)
    } else {
        let mac = path
            .strip_suffix(".boot")
            .and_then(normalize_mac)
            .ok_or(ApiError::NotFound)?;
        let (_, phone) = phones::find_by_mac(&state.db, &mac)
            .await?
            .ok_or(ApiError::NotFound)?;
        let model = state
            .phone_catalog
            .get(&phone.model)
            .ok_or(ApiError::NotFound)?;
        (model, mac)
    };
    if model.vendor != "yealink" {
        return Err(ApiError::NotFound);
    }
    let common = talkops_provisioning::library::sources()
        .iter()
        .filter_map(|(name, _)| name.strip_prefix(&format!("{}/", model.id)))
        .find(|name| name.starts_with('y') && name.ends_with(".cfg"))
        .ok_or(ApiError::NotFound)?;
    Ok(text(
        "text/plain; charset=utf-8",
        format!(
            "#!version:1.0.0.1\ninclude:config \"{common}\"\ninclude:config \"{mac}.cfg\"\noverwrite_mode = 0\n"
        ),
    ))
}
async fn softkey_file(state: &AppState, path: &str) -> ApiResult<Response> {
    let rest = path.strip_prefix("softkeys/").ok_or(ApiError::NotFound)?;
    let (mac, filename) = rest.split_once('/').ok_or(ApiError::NotFound)?;
    let mac = normalize_mac(mac).ok_or(ApiError::NotFound)?;
    let (_, phone) = phones::find_by_mac(&state.db, &mac)
        .await?
        .ok_or(ApiError::NotFound)?;
    let model = state
        .phone_catalog
        .get(&phone.model)
        .ok_or(ApiError::NotFound)?;
    let overrides: keys::Overrides =
        serde_json::from_value(phone.provisioning).map_err(|_| ApiError::NotFound)?;
    keys::validate(model, &overrides).map_err(ApiError::BadRequest)?;
    let name = filename.strip_suffix(".xml").ok_or(ApiError::NotFound)?;
    let caps = keys::capabilities(model);
    let allowed = caps.softkey_states.get(name).ok_or(ApiError::NotFound)?;
    let actions = overrides.softkeys.get(name).ok_or(ApiError::NotFound)?;
    Ok(text(
        "text/xml; charset=utf-8",
        keys::softkey_xml(name, actions, allowed),
    ))
}

/// `y<12-digit hardware id>.cfg`, e.g. `y000000000028.cfg` (T46) or
/// `y000000000108.cfg` (AX83H): the model-specific common file. All models
/// get the same content.
fn is_common_cfg(path: &str) -> bool {
    path.strip_prefix('y')
        .and_then(|rest| rest.strip_suffix(".cfg"))
        .is_some_and(|id| {
            matches!(id.len(), 12 | 13)
                && id.starts_with("000000000")
                && id.bytes().all(|b| b.is_ascii_digit())
        })
}

async fn common_cfg(state: &AppState, prov: &Provisioning, admin: &str) -> ApiResult<Response> {
    let s = settings::get(&state.db, TenantId::DEFAULT).await?;
    let options = provisioning_admin::options(state, TenantId::DEFAULT).await?;
    let language = if options.language.is_empty() {
        &s.default_language
    } else {
        &options.language
    };
    let time_zone = if options.time_zone.is_empty() {
        &s.timezone
    } else {
        &options.time_zone
    };
    let mut locale = Locale::new(time_zone, language);
    if !options.ntp_server.is_empty() {
        locale.ntp_server = options.ntp_server;
    }
    let body = yealink::render_common(prov, admin, &locale, language)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(text("text/plain; charset=utf-8", body))
}

async fn phone_cfg(
    state: &AppState,
    mac: &str,
    headers: &HeaderMap,
    ip: Option<&str>,
    user_agent: &str,
) -> ApiResult<Response> {
    let Some((tenant, phone)) = phones::find_by_mac(&state.db, mac).await? else {
        tracing::info!(mac, "provisioning request from unknown phone");
        return Err(ApiError::NotFound);
    };
    if tenant != TenantId::DEFAULT || talkops_provisioning::vendors::profile(&phone.model).is_some()
    {
        return Err(ApiError::NotFound);
    }
    let firmware = firmware_from_user_agent(user_agent);
    phones::record_seen(&state.db, phone.id, ip, firmware.as_deref()).await?;
    let body = render_phone_config(state, tenant, &phone, headers).await?;
    Ok(text("text/plain; charset=utf-8", body))
}

/// `cisco/directory/<phone id>/<token>`: the phone book of one Cisco
/// Enterprise phone as `CiscoIPPhoneDirectory` (`?page=`, `?search=`).
async fn cisco_directory(
    state: &AppState,
    rest: &str,
    query: &HashMap<String, String>,
    headers: &HeaderMap,
    limiter_key: &str,
) -> ApiResult<Response> {
    let (id, token) = rest.split_once('/').ok_or(ApiError::NotFound)?;
    let id = id.parse::<Uuid>().map_err(|_| ApiError::NotFound)?;
    let tenant = TenantId::DEFAULT;
    let secrets = phones::provisioning_secrets(&state.db, tenant, &state.secrets, false).await?;
    let expected = cisco_directory_token(&secrets.password, id);
    if !constant_time_eq(token.as_bytes(), expected.as_bytes()) {
        if !state.limiter.allow(limiter_key) {
            return Err(ApiError::TooManyRequests);
        }
        return Err(ApiError::NotFound);
    }
    let phone = phones::get(&state.db, tenant, id).await?;
    if !talkops_provisioning::vendors::cisco_enterprise(&phone.model) {
        return Err(ApiError::NotFound);
    }
    let endpoint = phone
        .provisioning
        .get("endpoint")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let base = match endpoint {
        Some(endpoint) => endpoint,
        None => configured_base(state, tenant, headers).await?,
    };
    let url = cisco_directory_url(&base, phone.id, token);
    if url.is_empty() {
        return Err(ApiError::NotFound);
    }
    let search = query.get("search").map(|s| s.trim());
    if search == Some("") {
        return Ok(text(
            "text/xml; charset=utf-8",
            phonebook::cisco_search(&url),
        ));
    }
    let entries = phonebook::search(phone_entries(state, tenant, &phone).await?, search);
    let encoded = search
        .map(|s| {
            percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
        })
        .unwrap_or_default();
    let page = query
        .get("page")
        .and_then(|p| p.parse::<usize>().ok())
        .unwrap_or(1);
    Ok(text(
        "text/xml; charset=utf-8",
        phonebook::cisco_directory("TalkOps", &entries, &url, &encoded, page),
    ))
}

async fn internal_phonebook(state: &AppState, search: Option<&String>) -> ApiResult<Response> {
    let entries = extensions::list(&state.db, TenantId::DEFAULT)
        .await?
        .into_iter()
        .filter(|e| e.enabled)
        .map(|e| Entry {
            name: if e.display_name.is_empty() {
                e.number.clone()
            } else {
                e.display_name.clone()
            },
            numbers: vec![("Extension".to_owned(), format!("**{}", e.number))],
        })
        .collect();
    let entries = phonebook::search(entries, search.map(String::as_str));
    Ok(text(
        "text/xml; charset=utf-8",
        adapter::phonebook("yealink", "TalkOps", &entries).map_err(ApiError::BadRequest)?,
    ))
}

/// The global contacts (`section` = `None`) or one phone book section.
async fn contacts_phonebook(
    state: &AppState,
    section: Option<Uuid>,
    search: Option<&String>,
) -> ApiResult<Response> {
    let title = match section {
        Some(id) => {
            phones::get_section(&state.db, TenantId::DEFAULT, id)
                .await?
                .name
        }
        None => "TalkOps".to_owned(),
    };
    let s = settings::get(&state.db, TenantId::DEFAULT).await?;
    let plan = s.dial_plan();
    let (work, mobile, other) = if s.default_language == "de" {
        ("Geschäftlich", "Mobil", "Sonstige")
    } else {
        ("Work", "Mobile", "Other")
    };
    let display = |n: &str| {
        if n.starts_with('+') {
            plan.for_display(n)
        } else {
            n.to_owned()
        }
    };
    let entries = phones::list_contacts_in(&state.db, TenantId::DEFAULT, section)
        .await?
        .into_iter()
        .map(|c| {
            let numbers = [
                (work, &c.phone_work),
                (mobile, &c.phone_mobile),
                (other, &c.phone_other),
            ]
            .into_iter()
            .filter(|(_, n)| !n.is_empty())
            .map(|(label, n)| (label.to_owned(), display(n)))
            .collect();
            let name = c.display_name();
            Entry { name, numbers }
        })
        .collect();
    let entries = phonebook::search(entries, search.map(String::as_str));
    Ok(text(
        "text/xml; charset=utf-8",
        adapter::phonebook("yealink", &title, &entries).map_err(ApiError::BadRequest)?,
    ))
}

/// Action URL events: DND toggled on the phone, setup completed.
async fn events(state: &AppState, query: &HashMap<String, String>) -> ApiResult<Response> {
    let Some(mac) = query.get("mac").and_then(|m| normalize_mac(m)) else {
        return Err(ApiError::BadRequest("mac required".into()));
    };
    let Some((tenant, phone)) = phones::find_by_mac(&state.db, &mac).await? else {
        return Err(ApiError::NotFound);
    };
    match query.get("event").map(String::as_str) {
        Some(ev @ ("dnd_on" | "dnd_off")) => {
            // The DND key acts on the phone's first account.
            if let Some(account) = phones::accounts(&state.db, phone.id).await?.first() {
                extensions::set_dnd(&state.db, tenant, account.extension_id, ev == "dnd_on")
                    .await?;
            }
        }
        Some("setup_completed") => {
            let ip = query
                .get("ip")
                .map(String::as_str)
                .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok());
            let fw = query
                .get("firmware")
                .map(String::as_str)
                .filter(|f| f.len() <= 32 && f.chars().all(|c| c.is_ascii_digit() || c == '.'));
            phones::record_seen(&state.db, phone.id, ip, fw).await?;
        }
        _ => return Err(ApiError::BadRequest("unknown event".into())),
    }
    Ok(StatusCode::OK.into_response())
}

/// `firmware/<id>/<filename>`: an uploaded firmware image.
async fn firmware(state: &AppState, path: &str) -> Response {
    let mut parts = path.splitn(3, '/').skip(1);
    let (Some(id), Some(name)) = (
        parts.next().and_then(|id| id.parse::<Uuid>().ok()),
        parts.next(),
    ) else {
        return not_found();
    };
    let Ok(fw) = phones::get_firmware(&state.db, TenantId::DEFAULT, id).await else {
        return not_found();
    };
    if fw.filename != name {
        return not_found();
    }
    let file_path = firmware_path(state, fw.id, &fw.filename);
    let Ok(file) = tokio::fs::File::open(&file_path).await else {
        tracing::warn!(path = %file_path.display(), "firmware file missing");
        return not_found();
    };
    let stream = tokio_util::io::ReaderStream::new(file);
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (header::CONTENT_LENGTH, fw.size_bytes.to_string()),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

/// `media/<id>/<filename>`: an uploaded ringtone or wallpaper.
async fn media(state: &AppState, path: &str) -> Response {
    let mut parts = path.splitn(3, '/').skip(1);
    let (Some(id), Some(name)) = (
        parts.next().and_then(|id| id.parse::<Uuid>().ok()),
        parts.next(),
    ) else {
        return not_found();
    };
    let Ok(m) = phones::get_media(&state.db, TenantId::DEFAULT, id).await else {
        return not_found();
    };
    if m.filename != name {
        return not_found();
    }
    let Ok(data) = tokio::fs::read(media_path(state, m.id, &m.filename)).await else {
        tracing::warn!(id = %m.id, "phone media file missing");
        return not_found();
    };
    (
        [(header::CONTENT_TYPE, media_type(&m.filename))],
        Body::from(data),
    )
        .into_response()
}

/// Content type of a ringtone or wallpaper file.
pub fn media_type(filename: &str) -> &'static str {
    match filename.rsplit('.').next() {
        Some("wav") => "audio/wav",
        Some("png") => "image/png",
        _ => "image/jpeg",
    }
}

/// Location of an uploaded ringtone or wallpaper on disk.
pub fn media_path(state: &AppState, id: Uuid, filename: &str) -> std::path::PathBuf {
    state
        .provisioning_dir
        .join("media")
        .join(id.to_string())
        .join(filename)
}

/// Location of an uploaded firmware image on disk.
pub fn firmware_path(state: &AppState, id: Uuid, filename: &str) -> std::path::PathBuf {
    state
        .provisioning_dir
        .join("firmware")
        .join(id.to_string())
        .join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_file_names() {
        assert!(is_common_cfg("y000000000028.cfg"));
        assert!(is_common_cfg("y000000000108.cfg"), "AX83H (three-digit id)");
        for name in [
            "y000000000036.cfg",
            "y0000000000183.cfg",
            "y0000000000196.cfg",
            "y0000000000197.cfg",
        ] {
            assert!(is_common_cfg(name), "{name}");
        }
        for name in [
            "y00000000000183.cfg",
            "y000000000018x.cfg",
            "y1000000000183.cfg",
            "y0000000000183.cfg/extra",
        ] {
            assert!(!is_common_cfg(name), "{name}");
        }
        assert!(!is_common_cfg("y00000000028.cfg"));
        assert!(!is_common_cfg("y00000000002x.cfg"));
        assert!(!is_common_cfg("y000000000000.boot"));
    }

    #[test]
    fn host_parsing() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "pbx.lan:8080".parse().unwrap());
        assert_eq!(base_url(&h), "http://pbx.lan:8080/provisioning");
        assert_eq!(sip_host(&h), "pbx.lan");
        h.insert(header::HOST, "[fd00::1]:8080".parse().unwrap());
        assert_eq!(sip_host(&h), "[fd00::1]");
        h.insert(header::HOST, "evil\"host".parse().unwrap());
        assert_eq!(sip_host(&h), "localhost");
    }
}
