//! Ring groups: several extensions ringing at once or one after another,
//! with a fallback destination for unanswered calls.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::numbering;
use crate::tenant::TenantId;
use crate::trunks::NumberDestination;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct RingGroup {
    pub id: Uuid,
    pub number: Option<String>,
    pub name: String,
    /// `simultaneous` or `sequential`.
    pub strategy: String,
    pub ring_timeout_secs: i32,
    pub caller_id_prefix: String,
    pub fallback_type: NumberDestination,
    pub fallback_id: Option<Uuid>,
    pub enabled: bool,
    /// Member extensions in ring order.
    #[sqlx(skip)]
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct RingGroupInput {
    #[serde(default)]
    pub number: Option<String>,
    pub name: String,
    #[serde(default = "simultaneous")]
    pub strategy: String,
    #[serde(default = "default_timeout")]
    pub ring_timeout_secs: i32,
    #[serde(default)]
    pub caller_id_prefix: String,
    #[serde(default = "no_destination")]
    pub fallback_type: NumberDestination,
    #[serde(default)]
    pub fallback_id: Option<Uuid>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub members: Vec<Uuid>,
}

fn simultaneous() -> String {
    "simultaneous".into()
}
fn default_timeout() -> i32 {
    25
}
fn no_destination() -> NumberDestination {
    NumberDestination::None
}
fn yes() -> bool {
    true
}

const COLUMNS: &str = "id, number, name, strategy, ring_timeout_secs, caller_id_prefix, \
                       fallback_type, fallback_id, enabled";

async fn with_members(pool: &PgPool, mut groups: Vec<RingGroup>) -> CoreResult<Vec<RingGroup>> {
    let ids: Vec<Uuid> = groups.iter().map(|g| g.id).collect();
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT group_id, extension_id FROM ring_group_members
         WHERE group_id = ANY($1) ORDER BY group_id, position",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    for g in &mut groups {
        g.members = rows
            .iter()
            .filter(|(group, _)| *group == g.id)
            .map(|(_, ext)| *ext)
            .collect();
    }
    Ok(groups)
}

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<RingGroup>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM ring_groups WHERE tenant_id = $1 ORDER BY number NULLS LAST, name"
    );
    let groups = sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?;
    with_members(pool, groups).await
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<RingGroup> {
    let sql = format!("SELECT {COLUMNS} FROM ring_groups WHERE tenant_id = $1 AND id = $2");
    let group = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(with_members(pool, vec![group]).await?.remove(0))
}

async fn validate(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &RingGroupInput,
    emergency: &[String],
) -> CoreResult<()> {
    if input.name.trim().is_empty() || input.name.chars().count() > 64 {
        return Err(CoreError::Validation(
            "name is required (max. 64 characters)".into(),
        ));
    }
    if let Some(n) = input.number.as_deref().filter(|n| !n.is_empty()) {
        numbering::validate_number(n, emergency)?;
        numbering::ensure_free(pool, tenant, n, id).await?;
    }
    if !["simultaneous", "sequential"].contains(&input.strategy.as_str()) {
        return Err(CoreError::Validation("invalid ring strategy".into()));
    }
    if !(5..=300).contains(&input.ring_timeout_secs) {
        return Err(CoreError::Validation(
            "ring time must be 5 to 300 seconds".into(),
        ));
    }
    if input.caller_id_prefix.chars().count() > 20 {
        return Err(CoreError::Validation("caller ID prefix is too long".into()));
    }
    if input.members.is_empty() {
        return Err(CoreError::Validation(
            "a ring group needs at least one member".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for m in &input.members {
        if !seen.insert(*m) {
            return Err(CoreError::Validation("an extension is listed twice".into()));
        }
        numbering::check_destination(pool, tenant, NumberDestination::Extension, Some(*m))
            .await
            .map_err(|_| CoreError::Validation("unknown member extension".into()))?;
    }
    if input.fallback_type == NumberDestination::RingGroup
        && id.is_some()
        && input.fallback_id == id
    {
        return Err(CoreError::Validation(
            "a group cannot fall back to itself".into(),
        ));
    }
    numbering::check_destination(pool, tenant, input.fallback_type, input.fallback_id).await
}

async fn set_members(db: &mut sqlx::PgConnection, group: Uuid, members: &[Uuid]) -> CoreResult<()> {
    sqlx::query("DELETE FROM ring_group_members WHERE group_id = $1")
        .bind(group)
        .execute(&mut *db)
        .await?;
    sqlx::query(
        "INSERT INTO ring_group_members (group_id, extension_id, position)
         SELECT $1, m, ord FROM unnest($2::uuid[]) WITH ORDINALITY AS t(m, ord)",
    )
    .bind(group)
    .bind(members)
    .execute(db)
    .await?;
    Ok(())
}

fn number(input: &RingGroupInput) -> Option<&str> {
    input.number.as_deref().filter(|n| !n.is_empty())
}

pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    input: &RingGroupInput,
    emergency: &[String],
) -> CoreResult<RingGroup> {
    validate(pool, tenant, None, input, emergency).await?;
    let mut tx = pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO ring_groups (tenant_id, number, name, strategy, ring_timeout_secs,
                                  caller_id_prefix, fallback_type, fallback_id, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(tenant)
    .bind(number(input))
    .bind(input.name.trim())
    .bind(&input.strategy)
    .bind(input.ring_timeout_secs)
    .bind(&input.caller_id_prefix)
    .bind(input.fallback_type)
    .bind(
        input
            .fallback_id
            .filter(|_| input.fallback_type != NumberDestination::None),
    )
    .bind(input.enabled)
    .fetch_one(&mut *tx)
    .await?;
    set_members(&mut tx, id, &input.members).await?;
    tx.commit().await?;
    get(pool, tenant, id).await
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &RingGroupInput,
    emergency: &[String],
) -> CoreResult<RingGroup> {
    get(pool, tenant, id).await?;
    validate(pool, tenant, Some(id), input, emergency).await?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE ring_groups SET number = $3, name = $4, strategy = $5, ring_timeout_secs = $6,
             caller_id_prefix = $7, fallback_type = $8, fallback_id = $9, enabled = $10,
             updated_at = now()
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(number(input))
    .bind(input.name.trim())
    .bind(&input.strategy)
    .bind(input.ring_timeout_secs)
    .bind(&input.caller_id_prefix)
    .bind(input.fallback_type)
    .bind(
        input
            .fallback_id
            .filter(|_| input.fallback_type != NumberDestination::None),
    )
    .bind(input.enabled)
    .execute(&mut *tx)
    .await?;
    set_members(&mut tx, id, &input.members).await?;
    tx.commit().await?;
    get(pool, tenant, id).await
}

pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM ring_groups WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}
