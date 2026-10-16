//! Tenant-scoped provisioning profiles, immutable revisions and request diagnostics.
use crate::{error::CoreResult, tenant::TenantId};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize, utoipa::ToSchema)]
pub struct Template {
    pub id: Uuid,
    pub name: String,
    pub vendor: String,
    pub model: String,
    pub active_revision: Option<i32>,
}
#[derive(Debug, FromRow, Serialize, utoipa::ToSchema)]
pub struct Revision {
    pub revision: i32,
    #[schema(value_type = Object)]
    pub files: Value,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, FromRow, Serialize, utoipa::ToSchema)]
pub struct RequestRecord {
    pub mac: Option<String>,
    pub filename: String,
    pub scheme: String,
    pub status: i16,
    pub created_at: DateTime<Utc>,
}
pub async fn options(db: &PgPool, tenant: TenantId) -> CoreResult<Value> {
    Ok(sqlx::query_scalar::<_, Value>(
        "SELECT provisioning_options FROM tenant_settings WHERE tenant_id=$1",
    )
    .bind(tenant)
    .fetch_optional(db)
    .await?
    .unwrap_or_else(|| serde_json::json!({})))
}
pub async fn set_options(db: &PgPool, tenant: TenantId, value: &Value) -> CoreResult<()> {
    sqlx::query("INSERT INTO tenant_settings (tenant_id, provisioning_options) VALUES ($1,$2) ON CONFLICT (tenant_id) DO UPDATE SET provisioning_options=EXCLUDED.provisioning_options")
        .bind(tenant).bind(value).execute(db).await?;
    Ok(())
}
pub async fn templates(db: &PgPool, tenant: TenantId) -> CoreResult<Vec<Template>> {
    Ok(sqlx::query_as("SELECT id,name,vendor,model,active_revision FROM provisioning_templates WHERE tenant_id=$1 ORDER BY name,id")
        .bind(tenant).fetch_all(db).await?)
}
pub async fn template(db: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Template> {
    Ok(sqlx::query_as("SELECT id,name,vendor,model,active_revision FROM provisioning_templates WHERE tenant_id=$1 AND id=$2")
        .bind(tenant).bind(id).fetch_one(db).await?)
}
pub async fn revisions(db: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Vec<Revision>> {
    template(db, tenant, id).await?;
    Ok(sqlx::query_as("SELECT revision,files,created_at FROM provisioning_template_revisions WHERE tenant_id=$1 AND template_id=$2 ORDER BY revision DESC")
        .bind(tenant).bind(id).fetch_all(db).await?)
}
pub async fn create_template(
    db: &PgPool,
    tenant: TenantId,
    name: &str,
    vendor: &str,
    model: &str,
    files: &Value,
) -> CoreResult<Template> {
    let mut tx = db.begin().await?;
    let row: Template=sqlx::query_as("INSERT INTO provisioning_templates (tenant_id,name,vendor,model) VALUES ($1,$2,$3,$4) RETURNING id,name,vendor,model,active_revision")
        .bind(tenant).bind(name).bind(vendor).bind(model).fetch_one(&mut *tx).await?;
    sqlx::query("INSERT INTO provisioning_template_revisions (tenant_id,template_id,revision,files) VALUES ($1,$2,1,$3)")
        .bind(tenant).bind(row.id).bind(files).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn add_revision(
    db: &PgPool,
    tenant: TenantId,
    id: Uuid,
    files: &Value,
) -> CoreResult<i32> {
    let mut tx = db.begin().await?;
    sqlx::query("SELECT id FROM provisioning_templates WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
        .bind(tenant)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let revision: i32=sqlx::query_scalar("INSERT INTO provisioning_template_revisions (tenant_id,template_id,revision,files) SELECT $1,$2,COALESCE(MAX(revision),0)+1,$3 FROM provisioning_template_revisions WHERE tenant_id=$1 AND template_id=$2 RETURNING revision")
        .bind(tenant).bind(id).bind(files).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(revision)
}
pub async fn activate(db: &PgPool, tenant: TenantId, id: Uuid, revision: i32) -> CoreResult<()> {
    sqlx::query(
        "UPDATE provisioning_templates SET active_revision=$3 WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(id)
    .bind(revision)
    .execute(db)
    .await?;
    Ok(())
}
pub async fn requests(
    db: &PgPool,
    tenant: TenantId,
    mac: Option<&str>,
) -> CoreResult<Vec<RequestRecord>> {
    Ok(sqlx::query_as("SELECT mac,filename,scheme,status,created_at FROM provisioning_requests WHERE tenant_id=$1 AND ($2::text IS NULL OR mac=$2) ORDER BY id DESC LIMIT 100")
        .bind(tenant).bind(mac).fetch_all(db).await?)
}
pub async fn record_request(
    db: &PgPool,
    tenant: TenantId,
    mac: Option<&str>,
    filename: &str,
    scheme: &str,
    status: u16,
) -> CoreResult<()> {
    // Store no headers, credentials, query strings or response bodies. Bound retention.
    let mut tx = db.begin().await?;
    sqlx::query("INSERT INTO provisioning_requests (tenant_id,mac,filename,scheme,status) VALUES ($1,$2,$3,$4,$5)")
        .bind(tenant).bind(mac).bind(filename).bind(scheme).bind(status as i16).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM provisioning_requests WHERE tenant_id=$1 AND id < COALESCE((SELECT id FROM provisioning_requests WHERE tenant_id=$1 ORDER BY id DESC OFFSET 1999 LIMIT 1),0)")
        .bind(tenant).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
