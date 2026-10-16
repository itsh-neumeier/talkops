//! Mail design: branding, logo and the texts of each mail type (ADR 0034).

use axum::Json;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit;
use talkops_core::mail;
use talkops_core::mail_render::{self, Design, InstanceInfo, MailKind, RenderedMail};
use talkops_core::mail_templates::{
    self, BrandingInput, MailBranding, TemplateEntry, TemplateText,
};
use talkops_core::roles::Permission;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::mailer;

pub fn router() -> OpenApiRouter<AppState> {
    let upload = OpenApiRouter::new()
        .routes(routes!(upload_logo, delete_logo, get_logo))
        .layer(DefaultBodyLimit::max(
            mail_templates::MAX_LOGO_BYTES + 64 * 1024,
        ));
    OpenApiRouter::new()
        .routes(routes!(get_mail_design))
        .routes(routes!(update_branding))
        .routes(routes!(save_template, reset_template))
        .routes(routes!(preview))
        .routes(routes!(send_template_test))
        .merge(upload)
}

/// A mail type with the placeholders its texts may use.
#[derive(Debug, Serialize, ToSchema)]
pub struct KindView {
    pub key: &'static str,
    /// Placeholder names, e.g. `caller.name`; the common ones first.
    pub placeholders: Vec<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MailDesignView {
    pub branding: MailBranding,
    /// Instance information shown in every mail (URL from the sign-in
    /// settings, host name and version of this server).
    pub instance: InstanceInfo,
    pub kinds: Vec<KindView>,
    pub templates: Vec<TemplateEntry>,
    /// Placeholders allowed in the header and footer text.
    pub common_placeholders: Vec<&'static str>,
    pub max_logo_bytes: usize,
}

fn kind_or_404(key: &str) -> ApiResult<&'static MailKind> {
    mail_render::kind(key).ok_or(ApiError::NotFound)
}

fn lang_or_400(lang: &str) -> ApiResult<&'static str> {
    mail_render::LANGUAGES
        .iter()
        .copied()
        .find(|l| *l == lang)
        .ok_or_else(|| ApiError::BadRequest("language must be de or en".into()))
}

/// Branding, instance information, mail types and texts (admin).
#[utoipa::path(get, path = "/api/v1/settings/mail", tag = "settings", responses((status = 200, body = MailDesignView)))]
pub async fn get_mail_design(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<MailDesignView>> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let (design, _) = mail_templates::design(&state.db, auth.tenant).await?;
    Ok(Json(MailDesignView {
        branding: mail_templates::branding(&state.db, auth.tenant).await?,
        instance: design.instance,
        kinds: mail_render::KINDS
            .iter()
            .map(|k| KindView {
                key: k.key,
                placeholders: k.all_placeholders().map(|p| p.name).collect(),
            })
            .collect(),
        templates: mail_templates::list_templates(&state.db, auth.tenant).await?,
        common_placeholders: mail_render::COMMON.iter().map(|p| p.name).collect(),
        max_logo_bytes: mail_templates::MAX_LOGO_BYTES,
    }))
}

/// Changes instance name, sender name, accent color, header and footer
/// text (admin).
#[utoipa::path(put, path = "/api/v1/settings/mail/branding", tag = "settings", request_body = BrandingInput, responses((status = 200, body = MailBranding)))]
pub async fn update_branding(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<BrandingInput>,
) -> ApiResult<Json<MailBranding>> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let b = mail_templates::update_branding(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "mail_branding",
        None,
        json!({"instance_name": b.instance_name, "sender_name": b.sender_name,
               "accent_color": b.accent_color, "header_chars": b.header_text.chars().count(),
               "footer_chars": b.footer_text.chars().count()}),
    )
    .await?;
    Ok(Json(b))
}

/// Uploads the mail logo (multipart field `file`; PNG, JPEG or GIF, at most
/// 256 KB and 4096 × 4096 pixels).
#[utoipa::path(post, path = "/api/v1/settings/mail/logo", tag = "settings", responses((status = 200, body = MailBranding)))]
pub async fn upload_logo(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<MailBranding>> {
    auth.require_perm(Permission::VoicemailSettings)?;
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
    let b = mail_templates::set_logo(&state.db, auth.tenant, &data).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "upload",
        "mail_logo",
        None,
        json!({"type": b.logo_type, "bytes": data.len(), "width": b.logo_width, "height": b.logo_height}),
    )
    .await?;
    Ok(Json(b))
}

/// Removes the mail logo (admin).
#[utoipa::path(delete, path = "/api/v1/settings/mail/logo", tag = "settings", responses((status = 200, body = MailBranding)))]
pub async fn delete_logo(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<MailBranding>> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let b = mail_templates::delete_logo(&state.db, auth.tenant).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "mail_logo",
        None,
        json!({}),
    )
    .await?;
    Ok(Json(b))
}

/// The mail logo image (admin).
#[utoipa::path(get, path = "/api/v1/settings/mail/logo", tag = "settings", responses((status = 200, content_type = "image/png")))]
pub async fn get_logo(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Response> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let logo = mail_templates::logo(&state.db, auth.tenant)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, logo.content_type),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        logo.bytes,
    )
        .into_response())
}

/// Saves an own subject and body for a mail type and language. Unknown
/// placeholders and syntax errors are rejected (admin).
#[utoipa::path(put, path = "/api/v1/settings/mail/templates/{kind}/{language}", tag = "settings", params(("kind" = String, Path), ("language" = String, Path)), request_body = TemplateText, responses((status = 200, body = [TemplateEntry])))]
pub async fn save_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((kind, language)): Path<(String, String)>,
    Json(input): Json<TemplateText>,
) -> ApiResult<Json<Vec<TemplateEntry>>> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let kind = kind_or_404(&kind)?;
    let lang = lang_or_400(&language)?;
    mail_templates::save_template(&state.db, auth.tenant, kind, lang, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "mail_template",
        Some(format!("{}/{lang}", kind.key)),
        json!({"kind": kind.key, "language": lang}),
    )
    .await?;
    Ok(Json(
        mail_templates::list_templates(&state.db, auth.tenant).await?,
    ))
}

/// Removes an own text; the default text applies again (admin).
#[utoipa::path(delete, path = "/api/v1/settings/mail/templates/{kind}/{language}", tag = "settings", params(("kind" = String, Path), ("language" = String, Path)), responses((status = 200, body = [TemplateEntry])))]
pub async fn reset_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((kind, language)): Path<(String, String)>,
) -> ApiResult<Json<Vec<TemplateEntry>>> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let kind = kind_or_404(&kind)?;
    let lang = lang_or_400(&language)?;
    mail_templates::reset_template(&state.db, auth.tenant, kind, lang).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "reset",
        "mail_template",
        Some(format!("{}/{lang}", kind.key)),
        json!({"kind": kind.key, "language": lang}),
    )
    .await?;
    Ok(Json(
        mail_templates::list_templates(&state.db, auth.tenant).await?,
    ))
}

/// A draft text, optionally with unsaved branding.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DraftInput {
    pub kind: String,
    pub language: String,
    pub subject: String,
    pub body: String,
    /// Unsaved branding for the preview; `null` uses the saved branding.
    #[serde(default)]
    pub branding: Option<BrandingInput>,
}

/// Design for a draft: saved branding, optionally replaced by the draft
/// branding (validated like a save).
async fn draft_design(
    state: &AppState,
    auth: &AuthUser,
    draft: &DraftInput,
) -> ApiResult<(Design, Option<mail_templates::Logo>)> {
    let (mut design, logo) = mail_templates::design(&state.db, auth.tenant).await?;
    if let Some(b) = &draft.branding {
        let accent = mail_render::normalize_color(&b.accent_color)
            .ok_or_else(|| ApiError::BadRequest("accent color must be #rrggbb".into()))?;
        for (field, text, max) in [
            ("header text", &b.header_text, mail_templates::MAX_HEADER),
            ("footer text", &b.footer_text, mail_templates::MAX_FOOTER),
        ] {
            mail_render::validate_fragment(field, text, max)
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        }
        design.sender_name = b.sender_name.clone();
        design.accent_color = accent;
        design.header_text = b.header_text.clone();
        design.footer_text = b.footer_text.clone();
        design.instance.name = b
            .instance_name
            .trim()
            .chars()
            .take(mail_templates::MAX_INSTANCE_NAME)
            .collect();
    }
    Ok((design, logo))
}

fn render_draft(
    design: &Design,
    kind: &MailKind,
    lang: &str,
    draft: &DraftInput,
) -> ApiResult<RenderedMail> {
    mail_render::validate(kind, &draft.subject, &draft.body)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let values = mail_render::sample_values(kind, lang);
    mail_render::render(design, kind, lang, &draft.subject, &draft.body, &values)
        .map_err(|e| ApiError::BadRequest(e.to_string()))
}

/// Renders a draft text with sample values. The HTML embeds the logo as a
/// `data:` URL; show it in a sandboxed frame (admin).
#[utoipa::path(post, path = "/api/v1/settings/mail/preview", tag = "settings", request_body = DraftInput, responses((status = 200, body = RenderedMail)))]
pub async fn preview(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(draft): Json<DraftInput>,
) -> ApiResult<Json<RenderedMail>> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let kind = kind_or_404(&draft.kind)?;
    let lang = lang_or_400(&draft.language)?;
    let (mut design, logo) = draft_design(&state, &auth, &draft).await?;
    if let Some(logo) = &logo {
        design.logo_src = format!(
            "data:{};base64,{}",
            logo.content_type,
            base64::engine::general_purpose::STANDARD.encode(&logo.bytes)
        );
    }
    Ok(Json(render_draft(&design, kind, lang, &draft)?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DraftTest {
    #[serde(flatten)]
    pub draft: DraftInput,
    /// Recipient of the test mail.
    pub to: String,
}

/// Sends a draft text with sample values to one address (admin).
#[utoipa::path(post, path = "/api/v1/settings/mail/test", tag = "settings", request_body = DraftTest, responses((status = 204)))]
pub async fn send_template_test(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<DraftTest>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::VoicemailSettings)?;
    let kind = kind_or_404(&input.draft.kind)?;
    let lang = lang_or_400(&input.draft.language)?;
    let cfg = mail::config(&state.db, auth.tenant, &state.secrets)
        .await?
        .ok_or_else(|| ApiError::BadRequest("SMTP is not configured".into()))?;
    let (design, logo) = draft_design(&state, &auth, &input.draft).await?;
    let rendered = render_draft(&design, kind, lang, &input.draft)?;
    let bad = |e: mailer::MailError| ApiError::BadRequest(e.to_string());
    let msg = mailer::build_message(
        mailer::sender(&cfg.from, &design.sender_name).map_err(bad)?,
        &input.to,
        rendered,
        logo.as_ref(),
        Vec::new(),
    )
    .map_err(bad)?;
    mailer::send(&cfg, msg).await.map_err(bad)?;
    audit::record(
        &state.db,
        &auth.actor(),
        "test",
        "mail_template",
        Some(format!("{}/{lang}", kind.key)),
        json!({"kind": kind.key, "language": lang}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
