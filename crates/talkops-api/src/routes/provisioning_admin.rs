//! Provisioning transport, source library, template revisions and diagnostics.
use crate::{
    AppState,
    auth::AuthUser,
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use talkops_core::{audit, phone_provisioning as store, roles::Permission, tenant::TenantId};
use talkops_provisioning::{keys, library};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

#[derive(Debug, Default, Clone, Deserialize, Serialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub language: String,
    pub time_zone: String,
    pub ntp_server: String,
    pub endpoint: String,
    pub sip_host: String,
}
pub async fn options(state: &AppState, tenant: TenantId) -> ApiResult<Options> {
    serde_json::from_value(store::options(&state.db, tenant).await?)
        .map_err(|_| ApiError::Internal("invalid stored provisioning options".into()))
}
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_options, put_options))
        .routes(routes!(get_requests))
        .routes(routes!(get_library))
        .routes(routes!(get_source))
        .routes(routes!(native_files))
        .routes(routes!(list_templates, create_template))
        .routes(routes!(get_revisions, save_revision))
        .routes(routes!(activate_revision))
}
#[utoipa::path(get,path="/api/v1/provisioning/options",tag="phones",responses((status=200,body=Options)))]
async fn get_options(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Options>> {
    auth.require_perm(Permission::PhonesManage)?;
    Ok(Json(options(&state, auth.tenant).await?))
}
#[utoipa::path(put,path="/api/v1/provisioning/options",tag="phones",request_body=Options,responses((status=200,body=Options)))]
async fn put_options(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<Options>,
) -> ApiResult<Json<Options>> {
    auth.require_perm(Permission::PhonesManage)?;
    keys::validate_locale(&input.language, &input.time_zone, &input.ntp_server)
        .map_err(ApiError::BadRequest)?;
    if !input.time_zone.is_empty() && input.time_zone.parse::<chrono_tz::Tz>().is_err() {
        return Err(ApiError::BadRequest("unknown device time zone".into()));
    }
    if !input.endpoint.is_empty() {
        keys::validate_endpoint(&input.endpoint, true).map_err(ApiError::BadRequest)?;
    }
    keys::validate_sip_host(&input.sip_host).map_err(ApiError::BadRequest)?;
    store::set_options(&state.db, auth.tenant, &json!(input)).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "provisioning_options",
        None,
        json!({"endpoint":input.endpoint,"sip_host":input.sip_host,"language":input.language,"time_zone":input.time_zone,"ntp_server":input.ntp_server}),
    )
    .await?;
    Ok(Json(input))
}
#[utoipa::path(get,path="/api/v1/provisioning/requests",tag="phones",responses((status=200,body=[store::RequestRecord])))]
async fn get_requests(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<BTreeMap<String, String>>,
) -> ApiResult<Json<Vec<store::RequestRecord>>> {
    auth.require_perm(Permission::PhonesManage)?;
    let mac = query
        .get("mac")
        .map(|mac| {
            talkops_provisioning::normalize_mac(mac)
                .ok_or_else(|| ApiError::BadRequest("invalid MAC".into()))
        })
        .transpose()?;
    Ok(Json(
        store::requests(&state.db, auth.tenant, mac.as_deref()).await?,
    ))
}
#[utoipa::path(get,path="/api/v1/provisioning/library",tag="phones",responses((status=200,body=Value)))]
async fn get_library(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Value>> {
    auth.require_perm(Permission::PhonesManage)?;
    let mut profiles=talkops_provisioning::vendors::profiles().iter().filter(|p|p.template_selector.is_none()).map(|p|{
        let model=if p.vendor=="yealink"{p.source_model.clone()}else{format!("{}/{}",p.vendor,p.source_model)};
        let adapted=state.phone_catalog.get(&p.id).is_some() || (p.vendor=="yealink"&&state.phone_catalog.get(&p.source_model).is_some());
        let prefix=format!("{}/{}/",p.vendor,p.source_model);
        let mut source_paths=talkops_provisioning::vendors::sources().iter().filter_map(|(name,_)|name.strip_prefix(&prefix).map(|file|(file.to_owned(),name.to_string()))).collect::<BTreeMap<_,_>>();
        for artifact in &p.artifacts {let path=artifact.template.trim_end_matches(".jinja");if talkops_provisioning::vendors::source(path).is_some(){source_paths.entry(artifact.name.clone()).or_insert_with(||path.into());}}
        json!({"model":model,"vendor":p.vendor,"adapted":adapted,"adaptation":p.adaptation,"device_validation":p.device_validation,"reason":p.reason,"files":source_paths.keys().collect::<Vec<_>>(),"source_paths":source_paths})
    }).collect::<Vec<_>>();
    profiles.push(json!({"model":"spectralink","vendor":"spectralink","adapted":false,"adaptation":"blocked","device_validation":"not_tested","reason":"no_source_template","files":["empty.txt"],"source_paths":{"empty.txt":"spectralink/empty.txt"}}));
    Ok(Json(
        json!({"revision":"20143aec4366afc2755f336d3058243479d49d44","license":"MPL-1.1","profiles":profiles}),
    ))
}
#[utoipa::path(get,path="/api/v1/provisioning/library/source/{path}",tag="phones",params(("path"=String,Path)),responses((status=200,body=String)))]
async fn get_source(auth: AuthUser, Path(path): Path<String>) -> ApiResult<Response> {
    auth.require_perm(Permission::PhonesManage)?;
    let source = talkops_provisioning::vendors::source(&path)
        .or_else(|| library::source(&path).map(str::as_bytes))
        .ok_or(ApiError::NotFound)?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                if std::str::from_utf8(source).is_ok() {
                    "text/plain; charset=utf-8"
                } else {
                    "application/octet-stream"
                },
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        source,
    )
        .into_response())
}
#[utoipa::path(get,path="/api/v1/provisioning/native",tag="phones",responses((status=200,body=Value)))]
async fn native_files(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<BTreeMap<String, String>>,
) -> ApiResult<Json<Value>> {
    auth.require_perm(Permission::PhonesManage)?;
    let files = match query.get("model") {
        Some(id) => {
            library::builtin_for_model(state.phone_catalog.get(id).ok_or(ApiError::NotFound)?)
                .map_err(ApiError::BadRequest)?
        }
        None => library::builtin(),
    };
    Ok(Json(json!(files)))
}
#[utoipa::path(get,path="/api/v1/provisioning/templates",tag="phones",responses((status=200,body=[store::Template])))]
async fn list_templates(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<store::Template>>> {
    auth.require_perm(Permission::PhonesManage)?;
    Ok(Json(store::templates(&state.db, auth.tenant).await?))
}
#[derive(Deserialize, ToSchema)]
struct NewTemplate {
    name: String,
    model: String,
}
#[utoipa::path(post,path="/api/v1/provisioning/templates",tag="phones",request_body=NewTemplate,responses((status=200,body=store::Template)))]
async fn create_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<NewTemplate>,
) -> ApiResult<Json<store::Template>> {
    auth.require_perm(Permission::PhonesManage)?;
    let model = state
        .phone_catalog
        .get(&input.model)
        .ok_or_else(|| ApiError::BadRequest("unknown model".into()))?;
    if input.name.trim().is_empty() || input.name.len() > 100 {
        return Err(ApiError::BadRequest(
            "invalid template name or unsupported vendor".into(),
        ));
    }
    let template = store::create_template(
        &state.db,
        auth.tenant,
        input.name.trim(),
        &model.vendor,
        &model.id,
        &json!(library::builtin_for_model(model).map_err(ApiError::BadRequest)?),
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "provisioning_template",
        Some(template.id.to_string()),
        json!({"model":model.id}),
    )
    .await?;
    Ok(Json(template))
}
#[utoipa::path(get,path="/api/v1/provisioning/templates/{id}/revisions",tag="phones",params(("id"=Uuid,Path)),responses((status=200,body=[store::Revision])))]
async fn get_revisions(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<store::Revision>>> {
    auth.require_perm(Permission::PhonesManage)?;
    Ok(Json(store::revisions(&state.db, auth.tenant, id).await?))
}
#[derive(Deserialize, ToSchema)]
struct RevisionInput {
    files: BTreeMap<String, String>,
}
#[utoipa::path(post,path="/api/v1/provisioning/templates/{id}/revisions",tag="phones",params(("id"=Uuid,Path)),request_body=RevisionInput,responses((status=200,body=Value)))]
async fn save_revision(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<RevisionInput>,
) -> ApiResult<Json<Value>> {
    auth.require_perm(Permission::PhonesManage)?;
    let template = store::template(&state.db, auth.tenant, id).await?;
    let model = state
        .phone_catalog
        .get(&template.model)
        .ok_or(ApiError::NotFound)?;
    library::validate_manifest(&input.files, model).map_err(ApiError::BadRequest)?;
    let revision = store::add_revision(&state.db, auth.tenant, id, &json!(input.files)).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "provisioning_template_revision",
        Some(id.to_string()),
        json!({"revision":revision}),
    )
    .await?;
    Ok(Json(json!({"revision":revision})))
}
#[derive(Deserialize, ToSchema)]
struct ActivateInput {
    revision: i32,
}
#[utoipa::path(post,path="/api/v1/provisioning/templates/{id}/activate",tag="phones",params(("id"=Uuid,Path)),request_body=ActivateInput,responses((status=204)))]
async fn activate_revision(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<ActivateInput>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::PhonesManage)?;
    let template = store::template(&state.db, auth.tenant, id).await?;
    let revisions = store::revisions(&state.db, auth.tenant, id).await?;
    let revision = revisions
        .iter()
        .find(|r| r.revision == input.revision)
        .ok_or(ApiError::NotFound)?;
    let files: BTreeMap<String, String> = serde_json::from_value(revision.files.clone())
        .map_err(|_| ApiError::BadRequest("invalid template files".into()))?;
    let model = state
        .phone_catalog
        .get(&template.model)
        .ok_or_else(|| ApiError::BadRequest("unknown template model".into()))?;
    library::validate_for_model(&files, model).map_err(ApiError::BadRequest)?;
    // Validate saved phones too; a failed candidate never replaces the active revision.
    for phone in talkops_core::phones::list(&state.db, auth.tenant).await? {
        if phone
            .provisioning
            .get("template_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            == Some(id)
        {
            super::provisioning::preview_with_files(&state, auth.tenant, &phone, &files).await?;
        }
    }
    store::activate(&state.db, auth.tenant, id, input.revision).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "activate",
        "provisioning_template",
        Some(id.to_string()),
        json!({"revision":input.revision}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn template_files(
    state: &AppState,
    tenant: TenantId,
    model: &str,
    id: Option<&str>,
) -> ApiResult<BTreeMap<String, String>> {
    let Some(id) = id else {
        return Ok(BTreeMap::new());
    };
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid template ID".into()))?;
    let template = store::template(&state.db, tenant, id).await?;
    if template.model != model {
        return Err(ApiError::BadRequest(
            "template belongs to another model".into(),
        ));
    }
    let revision = store::revisions(&state.db, tenant, id)
        .await?
        .into_iter()
        .find(|r| Some(r.revision) == template.active_revision)
        .ok_or_else(|| ApiError::BadRequest("template has no active revision".into()))?;
    serde_json::from_value(revision.files)
        .map_err(|_| ApiError::Internal("invalid active template".into()))
}
