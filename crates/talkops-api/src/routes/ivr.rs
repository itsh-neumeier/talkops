//! IVR menus.

use axum::Json;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use talkops_core::ivr::{self, IvrMenu, IvrMenuInput};
use talkops_core::users::Role;
use talkops_core::{audit, settings};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

/// Largest greeting upload (about 10 minutes of 8 kHz audio).
const GREETING_LIMIT: usize = 20 * 1024 * 1024;

pub fn router() -> OpenApiRouter<AppState> {
    let upload = OpenApiRouter::new()
        .routes(routes!(greeting_audio, upload_greeting))
        .layer(DefaultBodyLimit::max(GREETING_LIMIT));
    OpenApiRouter::new()
        .routes(routes!(list_menus, create_menu))
        .routes(routes!(get_menu, update_menu, delete_menu))
        .merge(upload)
}

/// Lists IVR menus (operator or admin).
#[utoipa::path(get, path = "/api/v1/ivr-menus", tag = "routing", responses((status = 200, body = [IvrMenu])))]
pub async fn list_menus(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<IvrMenu>>> {
    auth.require(Role::Operator)?;
    Ok(Json(ivr::list(&state.db, auth.tenant).await?))
}

async fn save(
    state: &AppState,
    auth: &AuthUser,
    id: Option<Uuid>,
    input: &IvrMenuInput,
) -> ApiResult<IvrMenu> {
    let s = settings::get(&state.db, auth.tenant).await?;
    let menu = ivr::save(
        &state.db,
        auth.tenant,
        id,
        input,
        &s.emergency_numbers,
        &s.default_language,
    )
    .await?;
    audit::record(
        &state.db,
        &auth.actor(),
        if id.is_some() { "update" } else { "create" },
        "ivr_menu",
        Some(menu.id.to_string()),
        json!({"name": menu.name, "number": menu.number, "options": menu.options.len()}),
    )
    .await?;
    Ok(menu)
}

/// Creates an IVR menu (admin). A TTS greeting is rendered in the background.
#[utoipa::path(post, path = "/api/v1/ivr-menus", tag = "routing", request_body = IvrMenuInput, responses((status = 200, body = IvrMenu)))]
pub async fn create_menu(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<IvrMenuInput>,
) -> ApiResult<Json<IvrMenu>> {
    auth.require(Role::Admin)?;
    Ok(Json(save(&state, &auth, None, &input).await?))
}

/// Returns an IVR menu (operator or admin).
#[utoipa::path(get, path = "/api/v1/ivr-menus/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 200, body = IvrMenu)))]
pub async fn get_menu(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<IvrMenu>> {
    auth.require(Role::Operator)?;
    Ok(Json(ivr::get(&state.db, auth.tenant, id).await?))
}

/// Updates an IVR menu (admin).
#[utoipa::path(put, path = "/api/v1/ivr-menus/{id}", tag = "routing", params(("id" = Uuid, Path)), request_body = IvrMenuInput, responses((status = 200, body = IvrMenu)))]
pub async fn update_menu(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<IvrMenuInput>,
) -> ApiResult<Json<IvrMenu>> {
    auth.require(Role::Admin)?;
    Ok(Json(save(&state, &auth, Some(id), &input).await?))
}

/// Deletes an IVR menu and its greeting (admin).
#[utoipa::path(delete, path = "/api/v1/ivr-menus/{id}", tag = "routing", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_menu(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let menu = ivr::get(&state.db, auth.tenant, id).await?;
    ivr::delete(&state.db, auth.tenant, id).await?;
    let _ =
        tokio::fs::remove_file(state.media.sounds.join(ivr::greeting_file(auth.tenant, id))).await;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "ivr_menu",
        Some(id.to_string()),
        json!({"name": menu.name}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The menu greeting as WAV (operator or admin).
#[utoipa::path(get, path = "/api/v1/ivr-menus/{id}/greeting", tag = "routing", params(("id" = Uuid, Path)), responses((status = 200, content_type = "audio/wav", body = Vec<u8>)))]
pub async fn greeting_audio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    auth.require(Role::Operator)?;
    ivr::get(&state.db, auth.tenant, id).await?;
    let path = state.media.sounds.join(ivr::greeting_file(auth.tenant, id));
    let data = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, "audio/wav"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from(data),
    )
        .into_response())
}

/// Uploads a WAV greeting (multipart field `file`) and makes it the
/// menu's greeting (admin). Mono PCM WAV, 8–48 kHz.
#[utoipa::path(post, path = "/api/v1/ivr-menus/{id}/greeting", tag = "routing", params(("id" = Uuid, Path)), request_body(content_type = "multipart/form-data", content = String), responses((status = 200, body = IvrMenu)))]
pub async fn upload_greeting(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<Json<IvrMenu>> {
    auth.require(Role::Admin)?;
    ivr::get(&state.db, auth.tenant, id).await?;
    let mut data = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
    {
        if field.name() == Some("file") {
            data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?,
            );
        }
    }
    let data = data.ok_or_else(|| ApiError::BadRequest("field `file` is required".into()))?;
    let spec = hound::WavReader::new(std::io::Cursor::new(&data))
        .map(|r| r.spec())
        .map_err(|_| ApiError::BadRequest("not a WAV file".into()))?;
    if spec.sample_format != hound::SampleFormat::Int
        || !(8000..=48000).contains(&spec.sample_rate)
        || spec.channels > 2
    {
        return Err(ApiError::BadRequest(
            "unsupported WAV format (PCM, 8–48 kHz, mono or stereo)".into(),
        ));
    }
    let path = state.media.sounds.join(ivr::greeting_file(auth.tenant, id));
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    let tmp = path.with_extension("upload.wav");
    tokio::fs::write(&tmp, &data)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ivr::set_uploaded_greeting(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "upload_greeting",
        "ivr_menu",
        Some(id.to_string()),
        json!({"bytes": data.len()}),
    )
    .await?;
    Ok(Json(ivr::get(&state.db, auth.tenant, id).await?))
}
