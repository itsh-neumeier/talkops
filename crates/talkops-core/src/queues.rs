//! Call queues: callers wait with music while member extensions (agents)
//! are offered the calls one by one. Executed by FreeSWITCH's
//! mod_callcenter; TalkOps owns the configuration.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::numbering;
use crate::tenant::TenantId;
use crate::trunks::NumberDestination;

pub const STRATEGIES: &[&str] = &[
    "ring-all",
    "longest-idle-agent",
    "round-robin",
    "top-down",
    "agent-with-fewest-calls",
    "random",
];

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Queue {
    pub id: Uuid,
    #[serde(skip)]
    pub tenant_id: TenantId,
    pub number: Option<String>,
    pub name: String,
    pub strategy: String,
    pub max_wait_secs: i32,
    pub agent_timeout_secs: i32,
    pub wrap_up_secs: i32,
    pub timeout_type: NumberDestination,
    pub timeout_id: Option<Uuid>,
    pub enabled: bool,
    /// Played once before the caller waits.
    pub greeting_clip_id: Option<Uuid>,
    /// Music on hold while waiting (default: the system music).
    pub moh_clip_id: Option<Uuid>,
    /// Callers waiting at most; 0 = no limit.
    pub max_callers: i32,
    /// Where callers go when the queue is full.
    pub overflow_type: NumberDestination,
    pub overflow_id: Option<Uuid>,
    /// Business hours; outside them calls go to `closed_*`.
    pub time_condition_id: Option<Uuid>,
    pub closed_type: NumberDestination,
    pub closed_id: Option<Uuid>,
    /// Not answered in time: a message for these extensions (instead of the
    /// timeout destination).
    pub voicemail_recipients: Vec<Uuid>,
    pub voicemail_clip_id: Option<Uuid>,
    /// Member extensions (agents) in order.
    #[sqlx(skip)]
    pub members: Vec<Uuid>,
}

impl Queue {
    /// A queue with default settings (tests).
    pub fn example() -> Self {
        Queue {
            id: Uuid::nil(),
            tenant_id: TenantId::DEFAULT,
            number: None,
            name: "Queue".into(),
            strategy: default_strategy(),
            max_wait_secs: default_wait(),
            agent_timeout_secs: default_agent_timeout(),
            wrap_up_secs: default_wrap_up(),
            timeout_type: NumberDestination::None,
            timeout_id: None,
            enabled: true,
            greeting_clip_id: None,
            moh_clip_id: None,
            max_callers: 0,
            overflow_type: NumberDestination::None,
            overflow_id: None,
            time_condition_id: None,
            closed_type: NumberDestination::None,
            closed_id: None,
            voicemail_recipients: Vec::new(),
            voicemail_clip_id: None,
            members: Vec::new(),
        }
    }

    /// Queue name inside mod_callcenter.
    pub fn cc_name(&self) -> String {
        cc_queue(self.id)
    }
}

pub fn cc_queue(id: Uuid) -> String {
    format!("q-{}", id.simple())
}

/// Agent name inside mod_callcenter: one agent per extension.
pub fn cc_agent(extension: Uuid) -> String {
    format!("a-{}", extension.simple())
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct QueueInput {
    #[serde(default)]
    pub number: Option<String>,
    pub name: String,
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default = "default_wait")]
    pub max_wait_secs: i32,
    #[serde(default = "default_agent_timeout")]
    pub agent_timeout_secs: i32,
    #[serde(default = "default_wrap_up")]
    pub wrap_up_secs: i32,
    #[serde(default = "no_destination")]
    pub timeout_type: NumberDestination,
    #[serde(default)]
    pub timeout_id: Option<Uuid>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub members: Vec<Uuid>,
    #[serde(default)]
    pub greeting_clip_id: Option<Uuid>,
    #[serde(default)]
    pub moh_clip_id: Option<Uuid>,
    #[serde(default)]
    pub max_callers: i32,
    #[serde(default = "no_destination")]
    pub overflow_type: NumberDestination,
    #[serde(default)]
    pub overflow_id: Option<Uuid>,
    #[serde(default)]
    pub time_condition_id: Option<Uuid>,
    #[serde(default = "no_destination")]
    pub closed_type: NumberDestination,
    #[serde(default)]
    pub closed_id: Option<Uuid>,
    #[serde(default)]
    pub voicemail_recipients: Vec<Uuid>,
    #[serde(default)]
    pub voicemail_clip_id: Option<Uuid>,
}

fn default_strategy() -> String {
    "longest-idle-agent".into()
}
fn default_wait() -> i32 {
    300
}
fn default_agent_timeout() -> i32 {
    20
}
fn default_wrap_up() -> i32 {
    5
}
fn no_destination() -> NumberDestination {
    NumberDestination::None
}
fn yes() -> bool {
    true
}

const COLUMNS: &str = "id, tenant_id, number, name, strategy, max_wait_secs, agent_timeout_secs, \
                       wrap_up_secs, timeout_type, timeout_id, enabled, greeting_clip_id, \
                       moh_clip_id, max_callers, overflow_type, overflow_id, time_condition_id, \
                       closed_type, closed_id, voicemail_recipients, voicemail_clip_id";

async fn with_members(pool: &PgPool, mut queues: Vec<Queue>) -> CoreResult<Vec<Queue>> {
    let ids: Vec<Uuid> = queues.iter().map(|q| q.id).collect();
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT queue_id, extension_id FROM queue_members
         WHERE queue_id = ANY($1) ORDER BY queue_id, position",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    for q in &mut queues {
        q.members = rows
            .iter()
            .filter(|(queue, _)| *queue == q.id)
            .map(|(_, ext)| *ext)
            .collect();
    }
    Ok(queues)
}

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<Queue>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM queues WHERE tenant_id = $1 ORDER BY number NULLS LAST, name"
    );
    let queues = sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?;
    with_members(pool, queues).await
}

/// All queues of all tenants (mod_callcenter configuration).
pub async fn list_all(pool: &PgPool) -> CoreResult<Vec<Queue>> {
    let sql = format!("SELECT {COLUMNS} FROM queues ORDER BY id");
    let queues = sqlx::query_as(&sql).fetch_all(pool).await?;
    with_members(pool, queues).await
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Queue> {
    let sql = format!("SELECT {COLUMNS} FROM queues WHERE tenant_id = $1 AND id = $2");
    let queue = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(with_members(pool, vec![queue]).await?.remove(0))
}

async fn validate(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &QueueInput,
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
    if !STRATEGIES.contains(&input.strategy.as_str()) {
        return Err(CoreError::Validation("invalid queue strategy".into()));
    }
    if !(0..=7200).contains(&input.max_wait_secs)
        || !(5..=120).contains(&input.agent_timeout_secs)
        || !(0..=600).contains(&input.wrap_up_secs)
    {
        return Err(CoreError::Validation("invalid queue timing".into()));
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
    if input.timeout_type == NumberDestination::Queue && id.is_some() && input.timeout_id == id {
        return Err(CoreError::Validation(
            "a queue cannot overflow into itself".into(),
        ));
    }
    numbering::check_destination(pool, tenant, input.timeout_type, input.timeout_id).await?;
    if !(0..=500).contains(&input.max_callers) {
        return Err(CoreError::Validation("at most 500 waiting callers".into()));
    }
    for (kind, target) in [
        (input.overflow_type, input.overflow_id),
        (input.closed_type, input.closed_id),
    ] {
        if kind == NumberDestination::Queue && id.is_some() && target == id {
            return Err(CoreError::Validation(
                "a queue cannot overflow into itself".into(),
            ));
        }
        numbering::check_destination(pool, tenant, kind, target).await?;
    }
    if input.time_condition_id.is_some() {
        numbering::check_destination(
            pool,
            tenant,
            NumberDestination::TimeCondition,
            input.time_condition_id,
        )
        .await?;
    }
    if input.voicemail_recipients.len() > 20 {
        return Err(CoreError::Validation(
            "at most 20 voicemail recipients".into(),
        ));
    }
    for ext in &input.voicemail_recipients {
        numbering::check_destination(pool, tenant, NumberDestination::Extension, Some(*ext))
            .await?;
    }
    for clip in [
        input.greeting_clip_id,
        input.moh_clip_id,
        input.voicemail_clip_id,
    ] {
        crate::audio::ensure_exists(pool, tenant, clip).await?;
    }
    Ok(())
}

async fn set_members(db: &mut sqlx::PgConnection, queue: Uuid, members: &[Uuid]) -> CoreResult<()> {
    sqlx::query("DELETE FROM queue_members WHERE queue_id = $1")
        .bind(queue)
        .execute(&mut *db)
        .await?;
    sqlx::query(
        "INSERT INTO queue_members (queue_id, extension_id, position)
         SELECT $1, m, ord FROM unnest($2::uuid[]) WITH ORDINALITY AS t(m, ord)",
    )
    .bind(queue)
    .bind(members)
    .execute(db)
    .await?;
    Ok(())
}

/// Creates (`id = None`) or updates a queue.
pub async fn save(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &QueueInput,
    emergency: &[String],
) -> CoreResult<Queue> {
    if let Some(id) = id {
        get(pool, tenant, id).await?;
    }
    validate(pool, tenant, id, input, emergency).await?;
    let number = input.number.as_deref().filter(|n| !n.is_empty());
    let timeout_id = input
        .timeout_id
        .filter(|_| input.timeout_type != NumberDestination::None);
    let mut tx = pool.begin().await?;
    let id: Uuid = match id {
        None => {
            sqlx::query_scalar(
                "INSERT INTO queues (tenant_id, number, name, strategy, max_wait_secs,
                     agent_timeout_secs, wrap_up_secs, timeout_type, timeout_id, enabled)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
            )
            .bind(tenant)
            .bind(number)
            .bind(input.name.trim())
            .bind(&input.strategy)
            .bind(input.max_wait_secs)
            .bind(input.agent_timeout_secs)
            .bind(input.wrap_up_secs)
            .bind(input.timeout_type)
            .bind(timeout_id)
            .bind(input.enabled)
            .fetch_one(&mut *tx)
            .await?
        }
        Some(id) => {
            sqlx::query(
                "UPDATE queues SET number = $3, name = $4, strategy = $5, max_wait_secs = $6,
                     agent_timeout_secs = $7, wrap_up_secs = $8, timeout_type = $9,
                     timeout_id = $10, enabled = $11, updated_at = now()
                 WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant)
            .bind(id)
            .bind(number)
            .bind(input.name.trim())
            .bind(&input.strategy)
            .bind(input.max_wait_secs)
            .bind(input.agent_timeout_secs)
            .bind(input.wrap_up_secs)
            .bind(input.timeout_type)
            .bind(timeout_id)
            .bind(input.enabled)
            .execute(&mut *tx)
            .await?;
            id
        }
    };
    let destination = |kind: NumberDestination, target: Option<Uuid>| {
        target.filter(|_| kind != NumberDestination::None)
    };
    sqlx::query(
        "UPDATE queues SET greeting_clip_id = $2, moh_clip_id = $3, max_callers = $4,
             overflow_type = $5, overflow_id = $6, time_condition_id = $7, closed_type = $8,
             closed_id = $9, voicemail_recipients = $10, voicemail_clip_id = $11
         WHERE id = $1",
    )
    .bind(id)
    .bind(input.greeting_clip_id)
    .bind(input.moh_clip_id)
    .bind(input.max_callers)
    .bind(input.overflow_type)
    .bind(destination(input.overflow_type, input.overflow_id))
    .bind(input.time_condition_id)
    .bind(input.closed_type)
    .bind(destination(input.closed_type, input.closed_id))
    .bind(&input.voicemail_recipients)
    .bind(input.voicemail_clip_id)
    .execute(&mut *tx)
    .await?;
    set_members(&mut tx, id, &input.members).await?;
    tx.commit().await?;
    get(pool, tenant, id).await
}

pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM queues WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// A queue agent as mod_callcenter should know it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    /// Originate string ringing all devices of the extension.
    pub contact: String,
    /// `Available` or `On Break` (DND, disabled, no devices).
    pub status: String,
    pub wrap_up_secs: i32,
    pub timeout_secs: i32,
}

/// A queue/agent assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub queue: String,
    pub agent: String,
    pub position: i32,
}

/// The complete agent and tier set mod_callcenter should have.
pub async fn desired_agents(pool: &PgPool, domain: &str) -> CoreResult<(Vec<Agent>, Vec<Tier>)> {
    #[derive(FromRow)]
    struct Row {
        queue_id: Uuid,
        position: i32,
        extension_id: Uuid,
        enabled: bool,
        dnd: bool,
        agent_timeout_secs: i32,
        wrap_up_secs: i32,
        devices: Vec<String>,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT m.queue_id, m.position, e.id AS extension_id, e.enabled, e.dnd,
                q.agent_timeout_secs, q.wrap_up_secs,
                COALESCE(array_agg(d.sip_username ORDER BY d.sip_username)
                         FILTER (WHERE d.enabled), '{}') AS devices
         FROM queue_members m
         JOIN queues q ON q.id = m.queue_id AND q.enabled
         JOIN extensions e ON e.id = m.extension_id
         LEFT JOIN devices d ON d.extension_id = e.id
         GROUP BY m.queue_id, m.position, e.id, q.agent_timeout_secs, q.wrap_up_secs
         ORDER BY e.id, m.queue_id",
    )
    .fetch_all(pool)
    .await?;
    let mut agents: Vec<Agent> = Vec::new();
    let mut tiers = Vec::new();
    for r in rows {
        let name = cc_agent(r.extension_id);
        tiers.push(Tier {
            queue: cc_queue(r.queue_id),
            agent: name.clone(),
            position: r.position,
        });
        if agents.iter().any(|a| a.name == name) {
            continue;
        }
        let contact = r
            .devices
            .iter()
            .filter(|d| {
                d.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            })
            .map(|d| format!("user/{d}@{domain}"))
            .collect::<Vec<_>>()
            .join(",");
        let available = r.enabled && !r.dnd && !contact.is_empty();
        agents.push(Agent {
            name,
            contact: if contact.is_empty() {
                "error/user_not_registered".into()
            } else {
                contact
            },
            status: if available { "Available" } else { "On Break" }.into(),
            wrap_up_secs: r.wrap_up_secs,
            timeout_secs: r.agent_timeout_secs,
        });
    }
    Ok((agents, tiers))
}
