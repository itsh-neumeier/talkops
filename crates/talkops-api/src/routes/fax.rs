//! Authorized fax submission, preview, history and administration.
use crate::{
    AppState,
    auth::AuthUser,
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::{
    audit,
    fax::{self, Endpoint, Fax, InputFile, Settings, Template, Whitelist},
    roles::Permission,
    trunks,
};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(history, upload))
        .routes(routes!(document))
        .routes(routes!(submit))
        .routes(routes!(cancel))
        .routes(routes!(resend))
        .routes(routes!(remove))
        .routes(routes!(get_settings, put_settings))
        .routes(routes!(get_endpoints, put_endpoint))
        .routes(routes!(get_whitelist, put_whitelist))
        .routes(routes!(get_template, put_template, reset_template))
        .routes(routes!(input_log))
        .routes(routes!(options))
        .routes(routes!(report))
        .layer(DefaultBodyLimit::max(30 * 1024 * 1024))
}
async fn accessible(state: &AppState, auth: &AuthUser, id: Uuid) -> ApiResult<Fax> {
    let f = fax::get(&state.db, auth.tenant, id).await?;
    let shared = if let Some(endpoint) = f.endpoint_id {
        f.direction == "inbound"
            && fax::endpoints(&state.db, auth.tenant)
                .await?
                .iter()
                .any(|e| e.id == endpoint && e.enabled && e.receive_users.contains(&auth.id))
    } else {
        false
    };
    if f.owner_id != auth.id && !auth.can(Permission::FaxAccessAll) && !shared {
        return Err(ApiError::NotFound);
    }
    Ok(f)
}
async fn record(
    state: &AppState,
    auth: &AuthUser,
    action: &str,
    id: Option<Uuid>,
) -> ApiResult<()> {
    audit::record(
        &state.db,
        &auth.actor(),
        action,
        "fax",
        id.map(|i| i.to_string()),
        json!({}),
    )
    .await?;
    Ok(())
}
#[utoipa::path(get,path="/api/v1/faxes",tag="fax",responses((status=200,body=[Fax])))]
async fn history(State(s): State<AppState>, a: AuthUser) -> ApiResult<Json<Vec<Fax>>> {
    Ok(Json(
        fax::list(&s.db, a.tenant, a.id, a.can(Permission::FaxAccessAll)).await?,
    ))
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UploadFile {
    pub name: String,
    pub data: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct Upload {
    pub destination: String,
    pub number_id: Option<Uuid>,
    pub files: Vec<UploadFile>,
    #[serde(skip)]
    pub receipt: Option<fax::Receipt>,
}
#[utoipa::path(post,path="/api/v1/faxes",tag="fax",request_body=Upload,responses((status=200,body=Fax)))]
async fn upload(
    State(s): State<AppState>,
    a: AuthUser,
    Json(input): Json<Upload>,
) -> ApiResult<Json<Fax>> {
    a.require_perm(Permission::FaxSend)?;
    let f = store_upload(&s, a.tenant, a.id, input, "", false).await?;
    record(&s, &a, "upload", Some(f.id)).await?;
    Ok(Json(f))
}
pub async fn store_upload(
    s: &AppState,
    tenant: talkops_core::tenant::TenantId,
    user: Uuid,
    input: Upload,
    sender: &str,
    automatic: bool,
) -> ApiResult<Fax> {
    if input.files.is_empty() || input.files.len() > fax::MAX_FILES {
        return Err(ApiError::BadRequest(
            "attach 1-10 PDF or image files".into(),
        ));
    }
    fax::resolve_number(&s.db, tenant, user, input.number_id).await?;
    if !fax::number(&input.destination) {
        return Err(ApiError::BadRequest(
            "Subject must contain only an international number, e.g. +49123456.".into(),
        ));
    }
    let id = Uuid::new_v4();
    let dir = crate::fax::directory(s, tenant, id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if fs2::available_space(&dir).map_err(|e| ApiError::Internal(e.to_string()))?
        < 100 * 1024 * 1024
    {
        return Err(ApiError::Conflict("Insufficient fax storage".into()));
    }
    let result = async {
        let mut files = Vec::new();
        let mut total = 0;
        for (i, file) in input.files.into_iter().enumerate() {
            let bytes = STANDARD
                .decode(&file.data)
                .map_err(|_| ApiError::BadRequest("invalid attachment encoding".into()))?;
            total += bytes.len();
            if total > fax::MAX_BYTES || bytes.is_empty() {
                return Err(ApiError::BadRequest(
                    "attachments exceed 20 MiB or are empty".into(),
                ));
            }
            let name = format!("input-{i}.bin");
            tokio::fs::write(dir.join(&name), bytes)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            files.push(InputFile {
                file: name,
                name: file
                    .name
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(200)
                    .collect(),
            });
        }
        Ok::<_, ApiError>(
            fax::create(
                &s.db,
                tenant,
                user,
                id,
                &input.destination,
                input.number_id,
                files,
                sender,
                automatic,
                input.receipt,
            )
            .await?,
        )
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
    result
}
#[utoipa::path(get,path="/api/v1/faxes/{id}/document",tag="fax",params(("id"=Uuid,Path)),responses((status=200,content_type="application/pdf",body=Vec<u8>)))]
async fn document(
    State(s): State<AppState>,
    a: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let f = accessible(&s, &a, id).await?;
    let path = crate::fax::directory(&s, f.tenant_id, id).join("document.pdf");
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| ApiError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (header::CONTENT_DISPOSITION, "inline; filename=\"fax.pdf\""),
            (header::CACHE_CONTROL, "no-store"),
            (header::X_FRAME_OPTIONS, "SAMEORIGIN"),
            (header::CONTENT_SECURITY_POLICY, "frame-ancestors 'self'"),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(file)),
    )
        .into_response())
}
#[utoipa::path(post,path="/api/v1/faxes/{id}/submit",tag="fax",params(("id"=Uuid,Path)),responses((status=200,body=Fax)))]
async fn submit(
    State(s): State<AppState>,
    a: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Fax>> {
    a.require_perm(Permission::FaxSend)?;
    let f = accessible(&s, &a, id).await?;
    if f.owner_id != a.id {
        return Err(ApiError::Forbidden);
    }
    fax::resolve_number(&s.db, a.tenant, a.id, f.number_id).await?;
    let changed=sqlx::query("UPDATE faxes SET state='queued',updated_at=now() WHERE tenant_id=$1 AND id=$2 AND state='draft'").bind(a.tenant).bind(id).execute(&s.db).await?.rows_affected();
    if changed != 1 {
        return Err(ApiError::Conflict("fax is not a prepared draft".into()));
    }
    record(&s, &a, "submit", Some(id)).await?;
    Ok(Json(fax::get(&s.db, a.tenant, id).await?))
}
#[utoipa::path(post,path="/api/v1/faxes/{id}/cancel",tag="fax",params(("id"=Uuid,Path)),responses((status=204)))]
async fn cancel(
    State(s): State<AppState>,
    a: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    a.require_perm(Permission::FaxSend)?;
    let f = accessible(&s, &a, id).await?;
    if f.owner_id != a.id && !a.can(Permission::FaxManage) {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("UPDATE faxes SET state='cancelled',updated_at=now(),result='Cancelled by user' WHERE tenant_id=$1 AND id=$2 AND state IN ('preparing','draft','queued','dialing','sending')").bind(a.tenant).bind(id).execute(&s.db).await?;
    if let (Some(uuid), Some(esl)) = (f.call_uuid, s.telephony.esl.get().await) {
        let _ = esl
            .api(&format!("uuid_kill {uuid} ORIGINATOR_CANCEL"))
            .await;
    }
    record(&s, &a, "cancel", Some(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/api/v1/faxes/{id}/resend",tag="fax",params(("id"=Uuid,Path)),responses((status=200,body=Fax)))]
async fn resend(
    State(s): State<AppState>,
    a: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Fax>> {
    a.require_perm(Permission::FaxSend)?;
    let f = accessible(&s, &a, id).await?;
    if f.direction != "outbound"
        || ["preparing", "queued", "dialing", "sending"].contains(&f.state.as_str())
    {
        return Err(ApiError::Conflict(
            "fax cannot be resent in this state".into(),
        ));
    }
    let inputs: Vec<InputFile> = serde_json::from_value(f.inputs)
        .map_err(|_| ApiError::Internal("invalid fax files".into()))?;
    let mut files = Vec::new();
    for input in inputs {
        if input.file.contains(['/', '\\']) {
            return Err(ApiError::Internal("invalid file".into()));
        }
        let bytes = tokio::fs::read(crate::fax::directory(&s, a.tenant, id).join(input.file))
            .await
            .map_err(|_| ApiError::NotFound)?;
        files.push(UploadFile {
            name: input.name,
            data: STANDARD.encode(bytes),
        });
    }
    let new = store_upload(
        &s,
        a.tenant,
        a.id,
        Upload {
            destination: f.destination,
            number_id: f.number_id,
            files,
            receipt: None,
        },
        "",
        false,
    )
    .await?;
    audit::record(
        &s.db,
        &a.actor(),
        "resend",
        "fax",
        Some(new.id.to_string()),
        json!({"previous_fax":id}),
    )
    .await?;
    Ok(Json(new))
}
#[utoipa::path(delete,path="/api/v1/faxes/{id}",tag="fax",params(("id"=Uuid,Path)),responses((status=204)))]
async fn remove(
    State(s): State<AppState>,
    a: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    a.require_perm(Permission::FaxDelete)?;
    let f = accessible(&s, &a, id).await?;
    if ["preparing", "queued", "dialing", "sending"].contains(&f.state.as_str()) {
        return Err(ApiError::Conflict(
            "cancel active fax before deletion".into(),
        ));
    }
    sqlx::query("DELETE FROM faxes WHERE tenant_id=$1 AND id=$2")
        .bind(a.tenant)
        .bind(id)
        .execute(&s.db)
        .await?;
    let _ = tokio::fs::remove_dir_all(crate::fax::directory(&s, a.tenant, id)).await;
    record(&s, &a, "delete", Some(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct SettingsView {
    pub config: Settings,
    pub has_password: bool,
    pub numbers: Vec<trunks::PhoneNumber>,
    pub users: Vec<FaxUser>,
}
#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct FaxUser {
    pub id: Uuid,
    pub name: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SettingsInput {
    pub config: Settings,
    pub password: Option<String>,
}
#[utoipa::path(get,path="/api/v1/settings/fax",tag="fax",responses((status=200,body=SettingsView)))]
async fn get_settings(State(s): State<AppState>, a: AuthUser) -> ApiResult<Json<SettingsView>> {
    a.require_perm(Permission::FaxManage)?;
    Ok(Json(SettingsView{config:fax::settings(&s.db,a.tenant).await?,has_password:fax::password(&s.db,a.tenant,&s.secrets).await?.is_some(),numbers:trunks::list_numbers(&s.db,a.tenant).await?,users:sqlx::query_as("SELECT id,display_name AS name FROM users WHERE tenant_id=$1 AND enabled ORDER BY display_name").bind(a.tenant).fetch_all(&s.db).await?}))
}
#[utoipa::path(put,path="/api/v1/settings/fax",tag="fax",request_body=SettingsInput,responses((status=204)))]
async fn put_settings(
    State(s): State<AppState>,
    a: AuthUser,
    Json(v): Json<SettingsInput>,
) -> ApiResult<StatusCode> {
    a.require_perm(Permission::FaxManage)?;
    fax::save_settings(
        &s.db,
        a.tenant,
        &v.config,
        v.password.as_deref(),
        &s.secrets,
    )
    .await?;
    record(&s, &a, "settings", None).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/v1/fax-endpoints",tag="fax",responses((status=200,body=[Endpoint])))]
async fn get_endpoints(State(s): State<AppState>, a: AuthUser) -> ApiResult<Json<Vec<Endpoint>>> {
    let mut entries = fax::endpoints(&s.db, a.tenant).await?;
    if !a.can(Permission::FaxManage)
        && !a.can(Permission::RoutingView)
        && !a.can(Permission::TrunksView)
    {
        entries.retain(|e| {
            e.owner_id == a.id || e.send_users.contains(&a.id) || e.receive_users.contains(&a.id)
        });
    }
    Ok(Json(entries))
}
#[utoipa::path(put,path="/api/v1/fax-endpoints",tag="fax",request_body=Endpoint,responses((status=204)))]
async fn put_endpoint(
    State(s): State<AppState>,
    a: AuthUser,
    Json(e): Json<Endpoint>,
) -> ApiResult<StatusCode> {
    a.require_perm(Permission::FaxManage)?;
    fax::save_endpoint(&s.db, a.tenant, &e).await?;
    record(&s, &a, "endpoint", Some(e.id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/v1/settings/fax/whitelist",tag="fax",responses((status=200,body=[Whitelist])))]
async fn get_whitelist(State(s): State<AppState>, a: AuthUser) -> ApiResult<Json<Vec<Whitelist>>> {
    a.require_perm(Permission::FaxManage)?;
    Ok(Json(fax::whitelist(&s.db, a.tenant).await?))
}
#[utoipa::path(put,path="/api/v1/settings/fax/whitelist",tag="fax",request_body=[Whitelist],responses((status=204)))]
async fn put_whitelist(
    State(s): State<AppState>,
    a: AuthUser,
    Json(v): Json<Vec<Whitelist>>,
) -> ApiResult<StatusCode> {
    a.require_perm(Permission::FaxManage)?;
    fax::save_whitelist(&s.db, a.tenant, &v).await?;
    record(&s, &a, "whitelist", None).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/v1/faxes/template",tag="fax",responses((status=200,body=Template)))]
async fn get_template(State(s): State<AppState>, a: AuthUser) -> ApiResult<Json<Template>> {
    Ok(Json(fax::template_for(&s.db, a.tenant, a.id).await?))
}
#[utoipa::path(put,path="/api/v1/faxes/template",tag="fax",request_body=Template,responses((status=204)))]
async fn put_template(
    State(s): State<AppState>,
    a: AuthUser,
    Json(t): Json<Template>,
) -> ApiResult<StatusCode> {
    a.require_perm(Permission::FaxTemplatesEdit)?;
    fax::save_template(&s.db, a.tenant, a.id, &t).await?;
    record(&s, &a, "template", None).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(delete,path="/api/v1/faxes/template",tag="fax",responses((status=204)))]
async fn reset_template(State(s): State<AppState>, a: AuthUser) -> ApiResult<StatusCode> {
    a.require_perm(Permission::FaxTemplatesEdit)?;
    sqlx::query("DELETE FROM fax_templates WHERE tenant_id=$1 AND user_id=$2")
        .bind(a.tenant)
        .bind(a.id)
        .execute(&s.db)
        .await?;
    record(&s, &a, "reset_template", None).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct MailLogEntry {
    pub sender: String,
    pub result: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct MailDelivery {
    pub recipient: String,
    pub kind: String,
    pub sent: bool,
    pub attempts: i32,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct MailLog {
    pub receipts: Vec<MailLogEntry>,
    pub deliveries: Vec<MailDelivery>,
}
#[utoipa::path(get,path="/api/v1/settings/fax/input-log",tag="fax",responses((status=200,body=MailLog)))]
async fn input_log(State(s): State<AppState>, a: AuthUser) -> ApiResult<Json<MailLog>> {
    a.require_perm(Permission::FaxManage)?;
    Ok(Json(MailLog{
        receipts:sqlx::query_as("SELECT sender,result,created_at FROM fax_mail_receipts WHERE tenant_id=$1 ORDER BY created_at DESC LIMIT 100").bind(a.tenant).fetch_all(&s.db).await?,
        deliveries:sqlx::query_as("SELECT recipient,kind,sent,attempts FROM fax_mail_outbox WHERE tenant_id=$1 ORDER BY run_at DESC LIMIT 100").bind(a.tenant).fetch_all(&s.db).await?,
    }))
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct Options {
    pub enabled: bool,
    pub numbers: Vec<trunks::PhoneNumber>,
}
#[utoipa::path(get,path="/api/v1/faxes/options",tag="fax",responses((status=200,body=Options)))]
async fn options(State(s): State<AppState>, a: AuthUser) -> ApiResult<Json<Options>> {
    let cfg = fax::settings(&s.db, a.tenant).await?;
    let own = fax::sender_numbers(&s.db, a.tenant, a.id).await?;
    let mut numbers = trunks::list_numbers(&s.db, a.tenant).await?;
    numbers.retain(|n| {
        n.enabled
            && ((!cfg.always_global && own.contains(&n.id))
                || (Some(n.id) == cfg.global_number_id && a.can(Permission::FaxUseGlobal)))
    });
    Ok(Json(Options {
        enabled: cfg.enabled,
        numbers,
    }))
}
#[utoipa::path(get,path="/api/v1/faxes/{id}/report",tag="fax",params(("id"=Uuid,Path)),responses((status=200,body=Fax)))]
async fn report(
    State(s): State<AppState>,
    a: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Fax>> {
    Ok(Json(accessible(&s, &a, id).await?))
}
