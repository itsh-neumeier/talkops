//! Smart Attendant: a call flow for incoming calls, built from steps like in
//! UniFi Talk – play audio, keypress menu, ring phones, schedule, voicemail,
//! park, forward, go to another step, hang up.
//!
//! A flow is a tree of [`Node`]s stored as JSON (`ivr_menus.flow`); a
//! missing `next` ends the call. [`Node::Goto`] jumps to any step of the same
//! flow (e.g. "back to the main menu"), so flows can loop; the call engine
//! bounds the number of steps per call. The destination kind stays `ivr`.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::numbering;
use crate::tenant::TenantId;
use crate::trunks::NumberDestination;

/// Upper bounds that keep flows reasonable.
pub const MAX_NODES: usize = 200;
const MAX_DEPTH: usize = 30;
const MAX_TARGETS: usize = 20;

/// The step after this one; `None` hangs up.
pub type Next = Option<Box<Node>>;

/// One step of a flow. Every step has an `id` unique within the flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    /// Plays an audio clip, then continues.
    Play {
        id: String,
        clip_id: Option<Uuid>,
        #[serde(default)]
        #[schema(no_recursion)]
        next: Next,
    },
    /// Plays a prompt and waits for a key.
    Menu {
        id: String,
        clip_id: Option<Uuid>,
        #[serde(default = "five")]
        timeout_secs: i32,
        #[serde(default = "three")]
        max_tries: i32,
        /// Callers may also dial internal numbers.
        #[serde(default)]
        direct_dial: bool,
        #[serde(default)]
        #[schema(no_recursion)]
        options: Vec<Choice>,
        /// No (valid) key after all tries.
        #[serde(default)]
        #[schema(no_recursion)]
        timeout: Next,
    },
    /// Rings extensions; continues with `next` when nobody answers.
    Ring {
        id: String,
        extensions: Vec<Uuid>,
        #[serde(default)]
        strategy: RingStrategy,
        #[serde(default = "thirty")]
        ring_secs: i32,
        #[serde(default)]
        #[schema(no_recursion)]
        next: Next,
    },
    /// Branches on a schedule (time condition: hours, holidays, closures).
    Schedule {
        id: String,
        time_condition_id: Option<Uuid>,
        #[serde(default)]
        #[schema(no_recursion)]
        open: Next,
        #[serde(default)]
        #[schema(no_recursion)]
        closed: Next,
    },
    /// Records a message for one or more voicemail boxes.
    Voicemail {
        id: String,
        recipients: Vec<Uuid>,
        /// Greeting; none: the system greeting.
        clip_id: Option<Uuid>,
        #[serde(default = "two_minutes")]
        max_message_secs: i32,
    },
    /// Parks the call in a free slot (*51 … *59) for pickup.
    Park {
        id: String,
    },
    /// Sends the call to another destination (extension, group, queue …).
    Transfer {
        id: String,
        destination_type: NumberDestination,
        destination_id: Option<Uuid>,
    },
    /// Continues at another step of this flow.
    Goto {
        id: String,
        target: String,
    },
    Hangup {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Choice {
    /// `0`–`9`, `*` or `#`.
    pub digit: String,
    #[serde(default)]
    #[schema(no_recursion)]
    pub next: Next,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RingStrategy {
    /// All phones at once.
    #[default]
    Simultaneous,
    /// One after the other, `ring_secs` each.
    Sequential,
}

fn five() -> i32 {
    5
}
fn three() -> i32 {
    3
}
fn thirty() -> i32 {
    30
}
fn two_minutes() -> i32 {
    120
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Node::Play { id, .. }
            | Node::Menu { id, .. }
            | Node::Ring { id, .. }
            | Node::Schedule { id, .. }
            | Node::Voicemail { id, .. }
            | Node::Park { id }
            | Node::Transfer { id, .. }
            | Node::Goto { id, .. }
            | Node::Hangup { id } => id,
        }
    }

    /// The steps directly below this one.
    pub fn children(&self) -> Vec<&Node> {
        match self {
            Node::Play { next, .. } | Node::Ring { next, .. } => {
                next.as_deref().into_iter().collect()
            }
            Node::Menu {
                options, timeout, ..
            } => options
                .iter()
                .filter_map(|o| o.next.as_deref())
                .chain(timeout.as_deref())
                .collect(),
            Node::Schedule { open, closed, .. } => open
                .as_deref()
                .into_iter()
                .chain(closed.as_deref())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// This step and everything below it.
    pub fn walk(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(node) = stack.pop() {
            out.push(node);
            stack.extend(node.children().into_iter().rev());
        }
        out
    }

    /// Steps by id.
    pub fn index(&self) -> HashMap<&str, &Node> {
        self.walk().into_iter().map(|n| (n.id(), n)).collect()
    }

    /// Audio clips the flow plays.
    pub fn clip_ids(&self) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = self
            .walk()
            .into_iter()
            .filter_map(|n| match n {
                Node::Play { clip_id, .. }
                | Node::Menu { clip_id, .. }
                | Node::Voicemail { clip_id, .. } => *clip_id,
                _ => None,
            })
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    fn depth(&self) -> usize {
        1 + self.children().iter().map(|c| c.depth()).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Attendant {
    pub id: Uuid,
    pub number: Option<String>,
    pub name: String,
    /// Prompt language; `None`: the tenant's default.
    pub language: Option<String>,
    #[schema(value_type = Node)]
    pub flow: Json<Node>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct AttendantInput {
    #[serde(default)]
    pub number: Option<String>,
    pub name: String,
    #[serde(default)]
    pub language: Option<String>,
    pub flow: Node,
}

/// The flow of a new attendant: a menu without options.
pub fn starter_flow() -> Node {
    Node::Menu {
        id: "start".into(),
        clip_id: None,
        timeout_secs: 5,
        max_tries: 3,
        direct_dial: false,
        options: Vec::new(),
        timeout: None,
    }
}

const COLUMNS: &str = "id, number, name, language, flow";

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<Attendant>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM ivr_menus WHERE tenant_id = $1 ORDER BY number NULLS LAST, name"
    );
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?)
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Attendant> {
    let sql = format!("SELECT {COLUMNS} FROM ivr_menus WHERE tenant_id = $1 AND id = $2");
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(CoreError::NotFound)
}

fn invalid(msg: impl Into<String>) -> CoreError {
    CoreError::Validation(msg.into())
}

/// Checks the shape of a flow (ids, keys, limits); [`validate`] also checks
/// the referenced extensions, clips and destinations.
pub fn check_structure(flow: &Node) -> CoreResult<()> {
    let nodes = flow.walk();
    if nodes.len() > MAX_NODES {
        return Err(invalid(format!("at most {MAX_NODES} steps")));
    }
    if flow.depth() > MAX_DEPTH {
        return Err(invalid(format!("at most {MAX_DEPTH} nested steps")));
    }
    let mut ids = HashSet::new();
    for node in &nodes {
        let id = node.id();
        let valid = !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !valid {
            return Err(invalid(format!("invalid step id `{id}`")));
        }
        if !ids.insert(id) {
            return Err(invalid(format!("step id `{id}` is used twice")));
        }
    }
    for node in &nodes {
        match node {
            Node::Menu {
                timeout_secs,
                max_tries,
                direct_dial,
                options,
                ..
            } => {
                if !(1..=30).contains(timeout_secs) || !(1..=10).contains(max_tries) {
                    return Err(invalid("invalid timeout or number of tries"));
                }
                let mut seen = HashSet::new();
                for o in options {
                    let valid = o.digit.len() == 1
                        && o.digit
                            .chars()
                            .all(|c| c.is_ascii_digit() || c == '*' || c == '#');
                    if !valid {
                        return Err(invalid(format!("invalid key `{}`", o.digit)));
                    }
                    if *direct_dial && o.digit == "#" {
                        return Err(invalid("# ends direct dialing and cannot be a menu option"));
                    }
                    if !seen.insert(o.digit.as_str()) {
                        return Err(invalid(format!("key {} is used twice", o.digit)));
                    }
                }
            }
            Node::Ring {
                extensions,
                ring_secs,
                ..
            } => {
                if extensions.is_empty() || extensions.len() > MAX_TARGETS {
                    return Err(invalid(format!("ring 1 to {MAX_TARGETS} extensions")));
                }
                if !(5..=300).contains(ring_secs) {
                    return Err(invalid("ring time: 5 to 300 seconds"));
                }
            }
            Node::Schedule {
                time_condition_id, ..
            } if time_condition_id.is_none() => {
                return Err(invalid("choose a schedule"));
            }
            Node::Voicemail {
                recipients,
                max_message_secs,
                ..
            } => {
                if recipients.is_empty() || recipients.len() > MAX_TARGETS {
                    return Err(invalid(format!(
                        "voicemail needs 1 to {MAX_TARGETS} recipients"
                    )));
                }
                if !(10..=600).contains(max_message_secs) {
                    return Err(invalid("message length: 10 to 600 seconds"));
                }
            }
            Node::Transfer {
                destination_type, ..
            } if *destination_type == NumberDestination::None => {
                return Err(invalid("choose where to forward the call"));
            }
            Node::Goto { target, .. } if !ids.contains(target.as_str()) => {
                return Err(invalid(format!("unknown step `{target}`")));
            }
            _ => {}
        }
    }
    Ok(())
}

async fn validate(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &AttendantInput,
    emergency: &[String],
) -> CoreResult<()> {
    if input.name.trim().is_empty() || input.name.chars().count() > 64 {
        return Err(invalid("name is required (max. 64 characters)"));
    }
    if let Some(n) = number(input) {
        numbering::validate_number(n, emergency)?;
        numbering::ensure_free(pool, tenant, n, id).await?;
    }
    if let Some(lang) = input.language.as_deref() {
        if !crate::prompts::LANGUAGES.contains(&lang) {
            return Err(invalid(format!("unsupported language `{lang}`")));
        }
    }
    check_structure(&input.flow)?;
    for clip in input.flow.clip_ids() {
        crate::audio::ensure_exists(pool, tenant, Some(clip)).await?;
    }
    for node in input.flow.walk() {
        match node {
            Node::Ring { extensions, .. }
            | Node::Voicemail {
                recipients: extensions,
                ..
            } => {
                for ext in extensions {
                    numbering::check_destination(
                        pool,
                        tenant,
                        NumberDestination::Extension,
                        Some(*ext),
                    )
                    .await?;
                }
            }
            Node::Schedule {
                time_condition_id, ..
            } => {
                numbering::check_destination(
                    pool,
                    tenant,
                    NumberDestination::TimeCondition,
                    *time_condition_id,
                )
                .await?;
            }
            Node::Transfer {
                destination_type,
                destination_id,
                ..
            } => {
                if *destination_type == NumberDestination::Ivr && *destination_id == id {
                    return Err(invalid(
                        "use “go to” to return to a step of the same attendant",
                    ));
                }
                numbering::check_destination(pool, tenant, *destination_type, *destination_id)
                    .await?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn number(input: &AttendantInput) -> Option<&str> {
    input.number.as_deref().filter(|n| !n.is_empty())
}

/// Creates (`id` = None) or updates an attendant.
pub async fn save(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &AttendantInput,
    emergency: &[String],
) -> CoreResult<Attendant> {
    if let Some(id) = id {
        get(pool, tenant, id).await?;
    }
    validate(pool, tenant, id, input, emergency).await?;
    let clips = input.flow.clip_ids();
    let flow = Json(&input.flow);
    let saved = match id {
        None => {
            let sql = format!(
                "INSERT INTO ivr_menus (tenant_id, number, name, language, flow, clip_ids)
                 VALUES ($1, $2, $3, $4, $5, $6) RETURNING {COLUMNS}"
            );
            sqlx::query_as(&sql)
                .bind(tenant)
                .bind(number(input))
                .bind(input.name.trim())
                .bind(&input.language)
                .bind(flow)
                .bind(&clips)
                .fetch_one(pool)
                .await?
        }
        Some(id) => {
            let sql = format!(
                "UPDATE ivr_menus SET number = $3, name = $4, language = $5, flow = $6,
                     clip_ids = $7, updated_at = now()
                 WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
            );
            sqlx::query_as(&sql)
                .bind(tenant)
                .bind(id)
                .bind(number(input))
                .bind(input.name.trim())
                .bind(&input.language)
                .bind(flow)
                .bind(&clips)
                .fetch_one(pool)
                .await?
        }
    };
    Ok(saved)
}

pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM ivr_menus WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Before 1.1, menu greetings lived at `ivr/<tenant>/<menu>.wav`; the
/// migration turned them into clips with the menu's id. Moves files whose
/// clip has no audio yet (idempotent; run on start).
pub async fn adopt_legacy_greetings(pool: &PgPool, sounds: &std::path::Path) -> CoreResult<usize> {
    let rows: Vec<(TenantId, Uuid)> = sqlx::query_as(
        "SELECT c.tenant_id, c.id FROM audio_clips c JOIN ivr_menus m ON m.id = c.id",
    )
    .fetch_all(pool)
    .await?;
    let mut moved = 0;
    for (tenant, id) in rows {
        let old = sounds.join(format!("ivr/{tenant}/{id}.wav"));
        let new = sounds.join(crate::audio::clip_file(tenant, id));
        if new.is_file() || !old.is_file() {
            continue;
        }
        let result = new
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::copy(&old, &new));
        match result {
            Ok(_) => {
                let _ = std::fs::remove_file(&old);
                moved += 1;
            }
            Err(err) => tracing::warn!(file = %old.display(), error = %err, "cannot move greeting"),
        }
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transfer(id: &str) -> Next {
        Some(Box::new(Node::Transfer {
            id: id.into(),
            destination_type: NumberDestination::Extension,
            destination_id: Some(Uuid::nil()),
        }))
    }

    fn sample() -> Node {
        serde_json::from_value(serde_json::json!({
            "type": "schedule", "id": "start", "time_condition_id": Uuid::nil(),
            "open": {
                "type": "menu", "id": "main", "clip_id": null,
                "options": [
                    {"digit": "1", "next": {"type": "ring", "id": "sales",
                        "extensions": [Uuid::nil()],
                        "next": {"type": "voicemail", "id": "vm", "recipients": [Uuid::nil()],
                                 "clip_id": null}}},
                    {"digit": "9", "next": {"type": "goto", "id": "again", "target": "main"}}
                ],
                "timeout": {"type": "hangup", "id": "bye"}
            },
            "closed": {"type": "play", "id": "closed", "clip_id": Uuid::nil(),
                       "next": {"type": "park", "id": "park"}}
        }))
        .unwrap()
    }

    #[test]
    fn parses_walks_and_validates() {
        let flow = sample();
        let ids: Vec<_> = flow.walk().iter().map(|n| n.id()).collect();
        assert_eq!(
            ids,
            [
                "start", "main", "sales", "vm", "again", "bye", "closed", "park"
            ]
        );
        assert_eq!(flow.clip_ids(), vec![Uuid::nil()]);
        assert!(flow.index().contains_key("vm"));
        check_structure(&flow).unwrap();
        // Defaults.
        let Node::Menu {
            timeout_secs,
            max_tries,
            ..
        } = flow.index()["main"]
        else {
            panic!()
        };
        assert_eq!((*timeout_secs, *max_tries), (5, 3));
        // Round trip.
        let json = serde_json::to_value(&flow).unwrap();
        assert_eq!(serde_json::from_value::<Node>(json).unwrap(), flow);
    }

    #[test]
    fn reads_flows_converted_by_the_migration() {
        // As written by migrations/20261013000002_v11_smart_attendant.sql.
        let flow: Node = serde_json::from_str(
            r#"{"id": "start", "type": "menu", "clip_id": "010ebab6-e902-4773-a93e-90e6ccc8c76e",
                "options": [{"next": {"id": "key-1", "type": "transfer",
                    "destination_id": "d57c3f4c-4561-4acb-af80-5132c4a61dd7",
                    "destination_type": "extension"}, "digit": "1"}],
                "timeout": null, "max_tries": 2, "direct_dial": true, "timeout_secs": 4}"#,
        )
        .unwrap();
        check_structure(&flow).unwrap();
        assert_eq!(flow.walk().len(), 2);
    }

    #[test]
    fn rejects_broken_flows() {
        let dup = Node::Play {
            id: "a".into(),
            clip_id: None,
            next: Some(Box::new(Node::Hangup { id: "a".into() })),
        };
        assert!(check_structure(&dup).is_err(), "duplicate id");
        let goto = Node::Goto {
            id: "a".into(),
            target: "nowhere".into(),
        };
        assert!(check_structure(&goto).is_err(), "unknown target");
        let keys = Node::Menu {
            id: "m".into(),
            clip_id: None,
            timeout_secs: 5,
            max_tries: 3,
            direct_dial: true,
            options: vec![Choice {
                digit: "#".into(),
                next: transfer("t"),
            }],
            timeout: None,
        };
        assert!(check_structure(&keys).is_err(), "# with direct dial");
        let ring = Node::Ring {
            id: "r".into(),
            extensions: vec![],
            strategy: RingStrategy::Simultaneous,
            ring_secs: 30,
            next: None,
        };
        assert!(check_structure(&ring).is_err(), "nobody to ring");
        let bad_id = Node::Hangup { id: "a b".into() };
        assert!(check_structure(&bad_id).is_err());
        let mut deep = Node::Hangup { id: "x0".into() };
        for i in 1..40 {
            deep = Node::Play {
                id: format!("x{i}"),
                clip_id: None,
                next: Some(Box::new(deep)),
            };
        }
        assert!(check_structure(&deep).is_err(), "too deep");
    }
}
