//! Audit log (ADR 0037): filtered and paged list, CSV/JSON download, the
//! automatic export to a storage target and the retention setting.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderName, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::audit::{self, AuditEntry, AuditFacets, AuditQuery, ExportFormat};
use talkops_core::audit_export::{self, AuditSettings, AuditSettingsInput, ExportFile};
use talkops_core::roles::Permission;
use talkops_core::storage_targets;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::audit_export as runner;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_audit))
        .routes(routes!(audit_facets))
        .routes(routes!(download))
        .routes(routes!(get_settings, put_settings))
        .routes(routes!(run_export))
}

/// Audit entries, filtered, sorted and paged. `X-Total-Count` has the
/// number of matching entries.
#[utoipa::path(get, path = "/api/v1/audit", tag = "settings", params(AuditQuery), responses((status = 200, body = [AuditEntry], headers(("X-Total-Count" = i64, description = "Number of matching entries")))))]
pub async fn list_audit(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<AuditQuery>,
) -> ApiResult<([(HeaderName, String); 1], Json<Vec<AuditEntry>>)> {
    auth.require_perm(Permission::AuditView)?;
    let (rows, total) = audit::search(&state.db, auth.tenant, &query).await?;
    Ok((
        [(HeaderName::from_static("x-total-count"), total.to_string())],
        Json(rows),
    ))
}

/// Actions, object types and users that occur in the audit log.
#[utoipa::path(get, path = "/api/v1/audit/facets", tag = "settings", responses((status = 200, body = AuditFacets)))]
pub async fn audit_facets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<AuditFacets>> {
    auth.require_perm(Permission::AuditView)?;
    Ok(Json(audit::facets(&state.db, auth.tenant).await?))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DownloadFormat {
    /// `csv` or `json`.
    pub format: ExportFormat,
}

/// The matching entries as a CSV or JSON file (at most 100,000 entries,
/// without paging). The download itself is recorded in the audit log.
#[utoipa::path(get, path = "/api/v1/audit/export", tag = "settings", params(DownloadFormat, AuditQuery), responses((status = 200, content_type = "text/csv"), (status = 422)))]
pub async fn download(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(format): Query<DownloadFormat>,
    Query(query): Query<AuditQuery>,
) -> ApiResult<Response> {
    auth.require_perm(Permission::AuditView)?;
    let rows = audit::export(&state.db, auth.tenant, &query).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "export",
        "audit_log",
        None,
        json!({
            "format": format.format.key(),
            "entries": rows.len(),
            "since": query.since,
            "until": query.until,
            "user_id": query.user_id,
            "action": query.action,
            "entity_type": query.entity_type,
            "search": query.search,
        }),
    )
    .await?;
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let (body, content_type, ext) = match format.format {
        ExportFormat::Csv => (audit::to_csv(&rows), "text/csv; charset=utf-8", "csv"),
        ExportFormat::Json => (
            audit::to_json(&rows),
            "application/json; charset=utf-8",
            "json",
        ),
    };
    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"talkops-audit-{stamp}.{ext}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        body,
    )
        .into_response())
}

#[derive(Serialize, ToSchema)]
pub struct AuditSettingsView {
    #[serde(flatten)]
    pub settings: AuditSettings,
    /// An export run is active.
    pub running: bool,
    /// Next planned export; `None` when the export is off.
    pub next_run_at: Option<chrono::DateTime<Utc>>,
    /// The newest export files with their SHA-256 checksums.
    pub files: Vec<ExportFile>,
    /// The server can transfer files (backup configuration with rclone).
    pub transfers_available: bool,
}

async fn view(state: &AppState, auth: &AuthUser) -> ApiResult<AuditSettingsView> {
    let settings = audit_export::get(&state.db, auth.tenant).await?;
    let next_run_at = settings
        .export_enabled
        .then(|| settings.schedule.next(chrono::Local::now().naive_local()))
        .flatten()
        .and_then(|n| n.and_local_timezone(chrono::Local).earliest())
        .map(|n| n.with_timezone(&Utc));
    Ok(AuditSettingsView {
        files: audit_export::files(&state.db, auth.tenant, 20).await?,
        running: runner::is_running(),
        next_run_at,
        transfers_available: state.backup.is_some(),
        settings,
    })
}

/// Retention and automatic export settings with the newest export files.
#[utoipa::path(get, path = "/api/v1/audit/settings", tag = "settings", responses((status = 200, body = AuditSettingsView)))]
pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<AuditSettingsView>> {
    auth.require_perm(Permission::AuditManage)?;
    Ok(Json(view(&state, &auth).await?))
}

/// Changes the retention and the automatic export.
#[utoipa::path(put, path = "/api/v1/audit/settings", tag = "settings", request_body = AuditSettingsInput, responses((status = 200, body = AuditSettingsView)))]
pub async fn put_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<AuditSettingsInput>,
) -> ApiResult<Json<AuditSettingsView>> {
    auth.require_perm(Permission::AuditManage)?;
    let folder = audit_export::check(&input)?;
    if let Some(id) = input.storage_target_id {
        let target = storage_targets::get(&state.db, auth.tenant, id)
            .await
            .map_err(|e| match e {
                talkops_core::error::CoreError::NotFound => {
                    ApiError::BadRequest("unknown storage target".into())
                }
                other => other.into(),
            })?;
        if input.export_enabled {
            let cfg = state.backup.as_ref().ok_or_else(|| {
                ApiError::Conflict("file transfers need the backup configuration".into())
            })?;
            let conn = talkops_core::storage_targets::TargetConnection {
                config: target.config.clone(),
                path: storage_targets::join_path(&target.path, &folder),
                allow_insecure: target.allow_insecure,
                secret: Default::default(),
            };
            crate::backup_targets::check_connection(cfg, &conn)?;
        }
    }
    let saved = audit_export::update(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "audit_settings",
        None,
        json!({
            "retention_days": saved.retention_days,
            "export_enabled": saved.export_enabled,
            "storage_target_id": saved.storage_target_id,
            "folder": saved.folder,
            "format": saved.format.key(),
            "schedule": saved.schedule,
            "retention": saved.retention,
        }),
    )
    .await?;
    Ok(Json(view(&state, &auth).await?))
}

/// Starts an export run now (`202`). The settings show the result.
#[utoipa::path(post, path = "/api/v1/audit/export/run", tag = "settings", responses((status = 202), (status = 409)))]
pub async fn run_export(State(state): State<AppState>, auth: AuthUser) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::AuditManage)?;
    let cfg = state
        .backup
        .clone()
        .ok_or_else(|| ApiError::Conflict("file transfers need the backup configuration".into()))?;
    let settings = audit_export::get(&state.db, auth.tenant).await?;
    if settings.storage_target_id.is_none() || settings.created_at.is_none() {
        return Err(ApiError::Conflict("select a storage target first".into()));
    }
    if !runner::start(cfg, state.db.clone(), state.secrets.clone(), auth.tenant) {
        return Err(ApiError::Conflict(
            "an audit export is already running".into(),
        ));
    }
    audit::record(
        &state.db,
        &auth.actor(),
        "run",
        "audit_export",
        None,
        json!({}),
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}
