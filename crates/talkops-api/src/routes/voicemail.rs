//! Voicemail boxes and messages (owner or admin) and SMTP settings (admin).

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::extensions::{self, Extension};
use talkops_core::mail::{self, SmtpInput, SmtpSettings};
use talkops_core::users::Role;
use talkops_core::voicemail::{self, Message, VoicemailBox, VoicemailBoxInput};
use talkops_core::{audit, settings};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::voicemail::{VmContext, delete_message as remove_message, update_mwi};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_box, update_box))
        .routes(routes!(greeting_audio))
        .routes(routes!(list_messages))
        .routes(routes!(update_message, delete_message))
        .routes(routes!(message_audio))
        .routes(routes!(get_smtp, update_smtp))
        .routes(routes!(test_smtp))
}

/// Voicemail is private: the extension's user, or an admin.
async fn owned_extension(state: &AppState, auth: &AuthUser, id: Uuid) -> ApiResult<Extension> {
    let ext = extensions::get(&state.db, auth.tenant, id).await?;
    if ext.user_id == Some(auth.id) || auth.role >= Role::Admin {
        Ok(ext)
    } else {
        Err(ApiError::Forbidden)
    }
}

#[derive(Serialize, ToSchema)]
pub struct VoicemailBoxView {
    #[serde(flatten)]
    pub settings: VoicemailBox,
    pub has_pin: bool,
    pub new_messages: u32,
    pub saved_messages: u32,
}

/// Voicemail settings of an extension.
#[utoipa::path(get, path = "/api/v1/extensions/{id}/voicemail", tag = "voicemail", params(("id" = Uuid, Path)), responses((status = 200, body = VoicemailBoxView)))]
pub async fn get_box(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<VoicemailBoxView>> {
    owned_extension(&state, &auth, id).await?;
    let vbox = voicemail::get_box(&state.db, auth.tenant, id).await?;
    let (new_messages, saved_messages) = voicemail::counts(&state.db, id).await?;
    Ok(Json(VoicemailBoxView {
        has_pin: vbox.has_pin(),
        settings: vbox,
        new_messages,
        saved_messages,
    }))
}

/// Changes voicemail settings. A TTS greeting is rendered in the background.
#[utoipa::path(put, path = "/api/v1/extensions/{id}/voicemail", tag = "voicemail", params(("id" = Uuid, Path)), request_body = VoicemailBoxInput, responses((status = 200, body = VoicemailBoxView)))]
pub async fn update_box(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<VoicemailBoxInput>,
) -> ApiResult<Json<VoicemailBoxView>> {
    let ext = owned_extension(&state, &auth, id).await?;
    let lang = settings::get(&state.db, auth.tenant)
        .await?
        .default_language;
    let vbox = voicemail::update_box(&state.db, auth.tenant, id, &input, &lang).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update_voicemail",
        "extension",
        Some(id.to_string()),
        json!({
            "number": ext.number,
            "enabled": vbox.enabled,
            "pin_changed": input.pin.is_some(),
            "greeting": vbox.greeting,
            "email_notify": vbox.email_notify,
        }),
    )
    .await?;
    let (new_messages, saved_messages) = voicemail::counts(&state.db, id).await?;
    Ok(Json(VoicemailBoxView {
        has_pin: vbox.has_pin(),
        settings: vbox,
        new_messages,
        saved_messages,
    }))
}

async fn wav(path: std::path::PathBuf) -> ApiResult<Response> {
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| ApiError::NotFound)?;
    let len = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .len();
    Ok((
        [
            (header::CONTENT_TYPE, "audio/wav".to_owned()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(file)),
    )
        .into_response())
}

/// The custom greeting (recorded or TTS) as WAV.
#[utoipa::path(get, path = "/api/v1/extensions/{id}/voicemail/greeting", tag = "voicemail", params(("id" = Uuid, Path)), responses((status = 200, content_type = "audio/wav", body = Vec<u8>)))]
pub async fn greeting_audio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    owned_extension(&state, &auth, id).await?;
    wav(state
        .media
        .voicemail
        .join(voicemail::greeting_file(auth.tenant, id)))
    .await
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct MessageQuery {
    pub extension_id: Uuid,
}

/// Messages of a box, new ones first.
#[utoipa::path(get, path = "/api/v1/voicemail/messages", tag = "voicemail", params(MessageQuery), responses((status = 200, body = [Message])))]
pub async fn list_messages(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<MessageQuery>,
) -> ApiResult<Json<Vec<Message>>> {
    owned_extension(&state, &auth, q.extension_id).await?;
    Ok(Json(
        voicemail::list_messages(&state.db, auth.tenant, q.extension_id).await?,
    ))
}

async fn owned_message(
    state: &AppState,
    auth: &AuthUser,
    id: Uuid,
) -> ApiResult<(Message, Extension)> {
    let msg = voicemail::get_message(&state.db, auth.tenant, id)
        .await
        .map_err(|_| ApiError::NotFound)?;
    let ext = owned_extension(state, auth, msg.extension_id).await?;
    Ok((msg, ext))
}

/// Audit an admin listening to someone else's voicemail.
async fn audit_foreign(
    state: &AppState,
    auth: &AuthUser,
    ext: &Extension,
    action: &str,
    id: Uuid,
) -> ApiResult<()> {
    if ext.user_id != Some(auth.id) {
        audit::record(
            &state.db,
            &auth.actor(),
            action,
            "voicemail",
            Some(id.to_string()),
            json!({"extension": ext.number}),
        )
        .await?;
    }
    Ok(())
}

/// A message recording as WAV.
#[utoipa::path(get, path = "/api/v1/voicemail/messages/{id}/audio", tag = "voicemail", params(("id" = Uuid, Path)), responses((status = 200, content_type = "audio/wav", body = Vec<u8>)))]
pub async fn message_audio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let (msg, ext) = owned_message(&state, &auth, id).await?;
    audit_foreign(&state, &auth, &ext, "listen", id).await?;
    wav(state.media.voicemail.join(&msg.file)).await
}

#[derive(Deserialize, ToSchema)]
pub struct MessageUpdate {
    /// `true` = heard (saved), `false` = new again.
    pub heard: bool,
}

/// Marks a message as heard or new; updates the phone's MWI lamp.
#[utoipa::path(put, path = "/api/v1/voicemail/messages/{id}", tag = "voicemail", params(("id" = Uuid, Path)), request_body = MessageUpdate, responses((status = 200, body = Message)))]
pub async fn update_message(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<MessageUpdate>,
) -> ApiResult<Json<Message>> {
    let (_, ext) = owned_message(&state, &auth, id).await?;
    let msg = voicemail::set_message_status(&state.db, auth.tenant, id, input.heard).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "voicemail",
        Some(id.to_string()),
        json!({"extension": ext.number, "heard": input.heard}),
    )
    .await?;
    update_mwi(&VmContext::from(&state), ext.id).await;
    Ok(Json(msg))
}

/// Deletes a message and its recording.
#[utoipa::path(delete, path = "/api/v1/voicemail/messages/{id}", tag = "voicemail", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_message(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let (_, ext) = owned_message(&state, &auth, id).await?;
    let ctx = VmContext::from(&state);
    remove_message(&ctx, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "voicemail",
        Some(id.to_string()),
        json!({"extension": ext.number}),
    )
    .await?;
    update_mwi(&ctx, ext.id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// SMTP settings for notification mails (admin).
#[utoipa::path(get, path = "/api/v1/settings/smtp", tag = "settings", responses((status = 200, body = SmtpSettings)))]
pub async fn get_smtp(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<SmtpSettings>> {
    auth.require(Role::Admin)?;
    Ok(Json(mail::get(&state.db, auth.tenant).await?))
}

/// Changes the SMTP settings (admin). The password is stored encrypted.
#[utoipa::path(put, path = "/api/v1/settings/smtp", tag = "settings", request_body = SmtpInput, responses((status = 200, body = SmtpSettings)))]
pub async fn update_smtp(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<SmtpInput>,
) -> ApiResult<Json<SmtpSettings>> {
    auth.require(Role::Admin)?;
    let s = mail::update(&state.db, auth.tenant, &state.secrets, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "smtp",
        None,
        json!({"host": s.host, "port": s.port, "security": s.security,
               "password_changed": input.password.is_some()}),
    )
    .await?;
    Ok(Json(s))
}

#[derive(Deserialize, ToSchema)]
pub struct SmtpTest {
    /// Recipient of the test mail.
    pub to: String,
}

/// Sends a test mail with the saved settings (admin).
#[utoipa::path(post, path = "/api/v1/settings/smtp/test", tag = "settings", request_body = SmtpTest, responses((status = 204)))]
pub async fn test_smtp(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<SmtpTest>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let cfg = mail::config(&state.db, auth.tenant, &state.secrets)
        .await?
        .ok_or_else(|| ApiError::BadRequest("SMTP is not configured".into()))?;
    crate::mailer::send_test(&cfg, &input.to)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    audit::record(&state.db, &auth.actor(), "test", "smtp", None, json!({})).await?;
    Ok(StatusCode::NO_CONTENT)
}
