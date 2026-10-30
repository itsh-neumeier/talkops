//! Cisco Enterprise user locales from uploaded Cisco locale installer
//! packages. The phones get the files of their language over TFTP.

use axum::Json;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;
use talkops_core::audit;
use talkops_core::roles::Permission;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::AuthUser;
use crate::cisco_locale::{self, LocaleError};
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_cisco_locales, upload_cisco_locale))
        .layer(DefaultBodyLimit::max(cisco_locale::MAX_PACKAGE + 64 * 1024))
        .routes(routes!(delete_cisco_locale))
}

#[derive(Serialize, ToSchema)]
pub struct CiscoLocaleView {
    /// Cisco name, such as `German_Germany`.
    pub name: String,
    /// Language code, such as `de_DE`. Phones with this TalkOps language use it.
    pub lang_code: String,
    pub version: String,
}

impl From<talkops_provisioning::vendors::CiscoLocale> for CiscoLocaleView {
    fn from(l: talkops_provisioning::vendors::CiscoLocale) -> Self {
        Self {
            name: l.name,
            lang_code: l.lang_code,
            version: l.version,
        }
    }
}

/// Installed Cisco Enterprise user locales (admin).
#[utoipa::path(get, path = "/api/v1/phone-locales/cisco", tag = "phones", responses((status = 200, body = Vec<CiscoLocaleView>)))]
pub async fn list_cisco_locales(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<CiscoLocaleView>>> {
    auth.require_perm(Permission::PhonesManage)?;
    let locales = cisco_locale::list(&state.provisioning_dir).await;
    Ok(Json(locales.into_iter().map(Into::into).collect()))
}

/// Installs a Cisco locale installer package (`po-locale-*.cop.sha512`,
/// multipart field `file`). TalkOps reads only its phone TFTP files and
/// never runs the package scripts. Admin.
#[utoipa::path(post, path = "/api/v1/phone-locales/cisco", tag = "phones", responses((status = 200, body = CiscoLocaleView)))]
pub async fn upload_cisco_locale(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<CiscoLocaleView>> {
    auth.require_perm(Permission::PhonesManage)?;
    let mut data = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
            data = Some(bytes);
        }
    }
    let data = data.ok_or_else(|| ApiError::BadRequest("file is required".into()))?;
    let package = tokio::task::spawn_blocking(move || cisco_locale::parse(&data))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| match e {
            LocaleError::Io(e) => ApiError::Internal(e.to_string()),
            other => ApiError::BadRequest(other.to_string()),
        })?;
    cisco_locale::store(&state.provisioning_dir, &package)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "cisco_locale",
        Some(package.locale.name.clone()),
        json!({"lang_code": package.locale.lang_code, "version": package.locale.version,
            "files": package.files.len()}),
    )
    .await?;
    crate::cisco_tftp::schedule(&state);
    Ok(Json(package.locale.into()))
}

/// Removes an installed Cisco locale. Admin.
#[utoipa::path(delete, path = "/api/v1/phone-locales/cisco/{name}", tag = "phones", params(("name" = String, Path)), responses((status = 204)))]
pub async fn delete_cisco_locale(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(name): Path<String>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::PhonesManage)?;
    if !cisco_locale::remove(&state.provisioning_dir, &name)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        return Err(ApiError::NotFound);
    }
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "cisco_locale",
        Some(name),
        json!({}),
    )
    .await?;
    crate::cisco_tftp::schedule(&state);
    Ok(StatusCode::NO_CONTENT)
}
