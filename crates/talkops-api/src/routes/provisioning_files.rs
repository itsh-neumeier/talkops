//! Provisioning files that an admin edits or adds (ADR 0052). A file with
//! the name of a generated file replaces it; another name adds a file.

use axum::Json;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::provisioning_files::{self as store, FileMeta, Transport};
use talkops_core::roles::Permission;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_provisioning_files, put_provisioning_file))
        .routes(routes!(get_provisioning_file, delete_provisioning_file))
        .layer(DefaultBodyLimit::max(2 * store::MAX_SIZE))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FileInput {
    pub transport: Transport,
    /// TFTP only: the phone; empty for every Cisco Enterprise phone.
    pub phone_id: Option<Uuid>,
    /// HTTP: path below `/provisioning/`; TFTP: file name.
    pub path: String,
    /// Text content (UTF-8). Give this or `content_base64`.
    pub content: Option<String>,
    /// Binary content.
    pub content_base64: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct FileView {
    #[serde(flatten)]
    pub meta: FileMeta,
    /// The content if it is UTF-8 text.
    pub content: Option<String>,
    pub content_base64: String,
}

/// Stored provisioning files (admin).
#[utoipa::path(get, path = "/api/v1/provisioning-files", tag = "phones", responses((status = 200, body = Vec<FileMeta>)))]
pub async fn list_provisioning_files(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<FileMeta>>> {
    auth.require_perm(Permission::PhonesManage)?;
    Ok(Json(store::list(&state.db, auth.tenant).await?))
}

/// Creates or replaces a provisioning file. Admin.
#[utoipa::path(put, path = "/api/v1/provisioning-files", tag = "phones", request_body = FileInput, responses((status = 200, body = FileMeta)))]
pub async fn put_provisioning_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<FileInput>,
) -> ApiResult<Json<FileMeta>> {
    auth.require_perm(Permission::PhonesManage)?;
    let content = match (input.content, input.content_base64) {
        (Some(text), None) => text.into_bytes(),
        (None, Some(data)) => STANDARD
            .decode(data.trim())
            .map_err(|_| ApiError::BadRequest("content_base64 is not base64".into()))?,
        _ => {
            return Err(ApiError::BadRequest(
                "give exactly one of content and content_base64".into(),
            ));
        }
    };
    let meta = store::put(
        &state.db,
        &state.secrets,
        auth.tenant,
        input.transport,
        input.phone_id,
        &input.path,
        &content,
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "provisioning_file",
        Some(meta.id.to_string()),
        json!({"transport": meta.transport, "phone_id": meta.phone_id, "path": meta.path,
            "size_bytes": meta.size_bytes}),
    )
    .await?;
    if input.transport == Transport::Tftp {
        crate::cisco_tftp::schedule(&state);
    }
    Ok(Json(meta))
}

/// One provisioning file with its content. Admin.
#[utoipa::path(get, path = "/api/v1/provisioning-files/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 200, body = FileView)))]
pub async fn get_provisioning_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<FileView>> {
    auth.require_perm(Permission::PhonesManage)?;
    let (meta, content) = store::content(&state.db, &state.secrets, auth.tenant, id).await?;
    Ok(Json(FileView {
        meta,
        content_base64: STANDARD.encode(&content),
        content: String::from_utf8(content).ok(),
    }))
}

/// Removes a provisioning file; a replaced generated file returns. Admin.
#[utoipa::path(delete, path = "/api/v1/provisioning-files/{id}", tag = "phones", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_provisioning_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::PhonesManage)?;
    let meta = store::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "provisioning_file",
        Some(meta.id.to_string()),
        json!({"transport": meta.transport, "phone_id": meta.phone_id, "path": meta.path}),
    )
    .await?;
    if meta.transport == "tftp" {
        crate::cisco_tftp::schedule(&state);
    }
    Ok(StatusCode::NO_CONTENT)
}
