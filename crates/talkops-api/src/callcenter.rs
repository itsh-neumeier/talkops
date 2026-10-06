//! Keeps mod_callcenter's agents, tiers and queues in line with the
//! database. mod_callcenter reads agents and tiers from `callcenter.conf`
//! only when it loads; afterwards they live in its own database, so changes
//! are applied with `callcenter_config` commands. The sync is diff-based (no
//! interruption of running queue calls) and runs after changes and every
//! 30 seconds (DND, devices).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use talkops_core::queues::{self, Agent, Queue, Tier};
use tokio::sync::Notify;

use crate::esl::EslHandle;
use crate::fsxml::SIP_DOMAIN;
use crate::fsxml::callcenter::contact;

const INTERVAL: Duration = Duration::from_secs(30);

/// Parses `callcenter_config … list` output (`a|b|c` with a header line,
/// ending in `+OK`) into maps keyed by column name.
fn parse_list(output: &str) -> Vec<HashMap<String, String>> {
    let mut lines = output
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("+OK"));
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let columns: Vec<&str> = header.split('|').collect();
    lines
        .map(|line| {
            columns
                .iter()
                .zip(line.split('|'))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .collect()
}

/// Commands that turn the current mod_callcenter state into the desired one.
pub fn plan(
    queues: &[Queue],
    agents: &[Agent],
    tiers: &[Tier],
    current_queues: &str,
    current_agents: &str,
    current_tiers: &str,
) -> Vec<String> {
    let mut cmds = Vec::new();

    let loaded: Vec<String> = parse_list(current_queues)
        .into_iter()
        .filter_map(|q| q.get("name").cloned())
        .collect();
    for q in queues {
        let name = q.cc_name();
        match (q.enabled, loaded.contains(&name)) {
            (true, true) => cmds.push(format!("callcenter_config queue reload {name}")),
            (true, false) => cmds.push(format!("callcenter_config queue load {name}")),
            (false, true) => cmds.push(format!("callcenter_config queue unload {name}")),
            (false, false) => {}
        }
    }
    for name in loaded.iter().filter(|n| n.starts_with("q-")) {
        if !queues.iter().any(|q| &q.cc_name() == name) {
            cmds.push(format!("callcenter_config queue unload {name}"));
        }
    }

    let current: HashMap<String, HashMap<String, String>> = parse_list(current_agents)
        .into_iter()
        .filter_map(|a| Some((a.get("name")?.clone(), a)))
        .collect();
    for a in agents {
        let contact = contact(a);
        let wrap_up = a.wrap_up_secs.to_string();
        let existing = current.get(&a.name);
        if existing.is_none() {
            cmds.push(format!("callcenter_config agent add {} callback", a.name));
            cmds.push(format!(
                "callcenter_config agent set max_no_answer {} 0",
                a.name
            ));
        }
        let field = |k: &str| existing.and_then(|e| e.get(k)).map(String::as_str);
        if field("contact") != Some(contact.as_str()) {
            cmds.push(format!(
                "callcenter_config agent set contact {} {contact}",
                a.name
            ));
        }
        if field("status") != Some(a.status.as_str()) {
            cmds.push(format!(
                "callcenter_config agent set status {} '{}'",
                a.name, a.status
            ));
        }
        if field("wrap_up_time") != Some(wrap_up.as_str()) {
            cmds.push(format!(
                "callcenter_config agent set wrap_up_time {} {wrap_up}",
                a.name
            ));
        }
    }
    for name in current.keys().filter(|n| n.starts_with("a-")) {
        if !agents.iter().any(|a| &a.name == name) {
            cmds.push(format!("callcenter_config agent del {name}"));
        }
    }

    let current_tiers = parse_list(current_tiers);
    for t in tiers {
        let existing = current_tiers
            .iter()
            .find(|c| c.get("queue") == Some(&t.queue) && c.get("agent") == Some(&t.agent));
        let position = t.position.to_string();
        match existing {
            None => cmds.push(format!(
                "callcenter_config tier add {} {} 1 {position}",
                t.queue, t.agent
            )),
            Some(c) if c.get("position") != Some(&position) => cmds.push(format!(
                "callcenter_config tier set position {} {} {position}",
                t.queue, t.agent
            )),
            Some(_) => {}
        }
    }
    for c in &current_tiers {
        let (Some(q), Some(a)) = (c.get("queue"), c.get("agent")) else {
            continue;
        };
        if a.starts_with("a-") && !tiers.iter().any(|t| &t.queue == q && &t.agent == a) {
            cmds.push(format!("callcenter_config tier del {q} {a}"));
        }
    }
    cmds
}

async fn sync(db: &PgPool, esl: &EslHandle) -> anyhow::Result<usize> {
    let Some(client) = esl.get().await else {
        return Ok(0);
    };
    let queues = queues::list_all(db).await?;
    let (agents, tiers) = queues::desired_agents(db, SIP_DOMAIN).await?;
    if queues.is_empty() && agents.is_empty() {
        // Nothing configured: do not touch (or require) mod_callcenter.
        return Ok(0);
    }
    let current_queues = client
        .api("callcenter_config queue list")
        .await
        .unwrap_or_default();
    let current_agents = client
        .api("callcenter_config agent list")
        .await
        .unwrap_or_default();
    let current_tiers = client
        .api("callcenter_config tier list")
        .await
        .unwrap_or_default();
    let cmds = plan(
        &queues,
        &agents,
        &tiers,
        &current_queues,
        &current_agents,
        &current_tiers,
    );
    for cmd in &cmds {
        if let Err(err) = client.api(cmd).await {
            tracing::warn!(command = %cmd, error = %err, "callcenter sync command failed");
        }
    }
    Ok(cmds.len())
}

/// Starts the background sync; `trigger` requests an immediate run.
pub fn spawn(db: PgPool, esl: EslHandle) -> Arc<Notify> {
    let trigger = Arc::new(Notify::new());
    let wake = trigger.clone();
    tokio::spawn(async move {
        loop {
            match sync(&db, &esl).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(commands = n, "queues synchronized"),
                Err(err) => tracing::warn!(error = %err, "queue sync failed"),
            }
            tokio::select! {
                _ = wake.notified() => {}
                _ = tokio::time::sleep(INTERVAL) => {}
            }
        }
    });
    trigger
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkops_core::trunks::NumberDestination;
    use uuid::Uuid;

    fn queue(id: u128, enabled: bool) -> Queue {
        Queue {
            id: Uuid::from_u128(id),
            number: None,
            name: "Q".into(),
            strategy: "ring-all".into(),
            max_wait_secs: 60,
            agent_timeout_secs: 15,
            wrap_up_secs: 5,
            timeout_type: NumberDestination::None,
            timeout_id: None,
            enabled,
            members: vec![],
        }
    }

    fn agent(name: &str, status: &str) -> Agent {
        Agent {
            name: name.into(),
            contact: "user/20-1@talkops.local".into(),
            status: status.into(),
            wrap_up_secs: 5,
            timeout_secs: 15,
        }
    }

    #[test]
    fn plans_minimal_changes() {
        let q1 = queue(1, true);
        let tiers = vec![Tier {
            queue: q1.cc_name(),
            agent: "a-1".into(),
            position: 1,
        }];
        // Empty mod_callcenter: everything is added.
        let cmds = plan(
            &[q1.clone(), queue(2, false)],
            &[agent("a-1", "Available")],
            &tiers,
            "name|strategy\n+OK\n",
            "name|instance_id|uuid|type|contact|status|state|max_no_answer|wrap_up_time\n+OK\n",
            "queue|agent|state|level|position\n+OK\n",
        );
        assert_eq!(
            cmds,
            [
                format!("callcenter_config queue load {}", q1.cc_name()),
                "callcenter_config agent add a-1 callback".to_owned(),
                "callcenter_config agent set max_no_answer a-1 0".to_owned(),
                "callcenter_config agent set contact a-1 [leg_timeout=15]user/20-1@talkops.local"
                    .to_owned(),
                "callcenter_config agent set status a-1 'Available'".to_owned(),
                "callcenter_config agent set wrap_up_time a-1 5".to_owned(),
                format!("callcenter_config tier add {} a-1 1 1", q1.cc_name()),
            ]
        );

        // In sync except for DND; stale agents, tiers and queues are removed.
        let agents_out = "name|instance_id|uuid|type|contact|status|state|max_no_answer|wrap_up_time\n\
             a-1|single_box||callback|[leg_timeout=15]user/20-1@talkops.local|Available|Waiting|0|5\n\
             a-9|single_box||callback|x|Available|Waiting|0|5\n\
             manual|single_box||callback|x|Available|Waiting|0|5\n+OK\n";
        let tiers_out = format!(
            "queue|agent|state|level|position\n{q}|a-1|Ready|1|1\n{q}|a-9|Ready|1|2\n+OK\n",
            q = q1.cc_name()
        );
        let queues_out = format!(
            "name|strategy\n{}|ring-all\nq-gone|ring-all\n+OK\n",
            q1.cc_name()
        );
        let cmds = plan(
            std::slice::from_ref(&q1),
            &[agent("a-1", "On Break")],
            &tiers,
            &queues_out,
            agents_out,
            &tiers_out,
        );
        assert_eq!(
            cmds,
            [
                format!("callcenter_config queue reload {}", q1.cc_name()),
                "callcenter_config queue unload q-gone".to_owned(),
                "callcenter_config agent set status a-1 'On Break'".to_owned(),
                "callcenter_config agent del a-9".to_owned(),
                format!("callcenter_config tier del {} a-9", q1.cc_name()),
            ]
        );
    }
}
