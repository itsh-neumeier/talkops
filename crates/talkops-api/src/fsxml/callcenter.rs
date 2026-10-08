//! `callcenter.conf` for mod_callcenter: queues, agents and tiers from the
//! database. Agents and tiers are only read when the module loads; later
//! changes are applied by [`crate::callcenter`] over the event socket.

use std::path::Path;

use talkops_core::audio;
use talkops_core::queues::{Agent, Queue, Tier};

use super::XmlWriter;

/// Music on hold for waiting callers.
pub const MOH: &str = "local_stream://default";

pub fn render(queues: &[Queue], agents: &[Agent], tiers: &[Tier], sounds: &Path) -> String {
    let mut w = XmlWriter::document();
    w.open("section", &[("name", "configuration")]);
    w.open(
        "configuration",
        &[
            ("name", "callcenter.conf"),
            ("description", "TalkOps queues"),
        ],
    );
    w.open("settings", &[]);
    w.close("settings");
    w.open("queues", &[]);
    for q in queues.iter().filter(|q| q.enabled) {
        w.open("queue", &[("name", &q.cc_name())]);
        for (name, value) in queue_params(q, sounds) {
            w.param(name, &value);
        }
        w.close("queue");
    }
    w.close("queues");
    w.open("agents", &[]);
    for a in agents {
        w.empty(
            "agent",
            &[
                ("name", &a.name),
                ("type", "callback"),
                ("contact", &contact(a)),
                ("status", &a.status),
                ("max-no-answer", "0"),
                ("wrap-up-time", &a.wrap_up_secs.to_string()),
                ("reject-delay-time", "5"),
                ("busy-delay-time", "5"),
            ],
        );
    }
    w.close("agents");
    w.open("tiers", &[]);
    for t in tiers {
        w.empty(
            "tier",
            &[
                ("agent", &t.agent),
                ("queue", &t.queue),
                ("level", "1"),
                ("position", &t.position.to_string()),
            ],
        );
    }
    w.close("tiers");
    w.close("configuration");
    w.close("section");
    w.finish()
}

/// Originate string for an agent: every device rings for the queue's
/// agent ring time.
pub fn contact(a: &Agent) -> String {
    a.contact
        .split(',')
        .map(|leg| format!("[leg_timeout={}]{leg}", a.timeout_secs))
        .collect::<Vec<_>>()
        .join(",")
}

/// The queue's own music on hold (an audio clip, looped), or the system's.
fn moh(q: &Queue, sounds: &Path) -> String {
    q.moh_clip_id
        .map(|id| sounds.join(audio::clip_file(q.tenant_id, id)))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.contains([' ', '$', '{', '}']))
        .unwrap_or_else(|| MOH.to_owned())
}

pub fn queue_params(q: &Queue, sounds: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("strategy", q.strategy.clone()),
        ("moh-sound", moh(q, sounds)),
        ("time-base-score", "system".to_owned()),
        ("max-wait-time", q.max_wait_secs.to_string()),
        // Nobody available: leave the queue after 30 s.
        ("max-wait-time-with-no-agent", "30".to_owned()),
        ("max-wait-time-with-no-agent-time-reached", "5".to_owned()),
        ("tier-rules-apply", "false".to_owned()),
        ("discard-abandoned-after", "60".to_owned()),
        ("abandoned-resume-allowed", "false".to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkops_core::trunks::NumberDestination;
    use uuid::Uuid;

    #[test]
    fn renders_queues_agents_tiers() {
        let q = Queue {
            id: Uuid::nil(),
            number: Some("80".into()),
            name: "Support".into(),
            strategy: "ring-all".into(),
            max_wait_secs: 120,
            agent_timeout_secs: 15,
            wrap_up_secs: 5,
            timeout_type: NumberDestination::None,
            timeout_id: None,
            enabled: true,
            members: vec![],
            ..Queue::example()
        };
        let a = Agent {
            name: "a-1".into(),
            contact: "user/20-1@talkops.local,user/20-2@talkops.local".into(),
            status: "Available".into(),
            wrap_up_secs: 5,
            timeout_secs: 15,
        };
        let t = Tier {
            queue: q.cc_name(),
            agent: "a-1".into(),
            position: 1,
        };
        let xml = render(&[q], &[a], &[t], Path::new("/nonexistent"));
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let queue = doc.descendants().find(|n| n.has_tag_name("queue")).unwrap();
        assert_eq!(
            queue.attribute("name"),
            Some("q-00000000000000000000000000000000")
        );
        let agent = doc.descendants().find(|n| n.has_tag_name("agent")).unwrap();
        assert_eq!(
            agent.attribute("contact"),
            Some("[leg_timeout=15]user/20-1@talkops.local,[leg_timeout=15]user/20-2@talkops.local")
        );
        assert!(xml.contains("name=\"max-wait-time\" value=\"120\""));
        assert!(xml.contains("name=\"moh-sound\" value=\"local_stream://default\""));
        assert!(doc.descendants().any(|n| n.has_tag_name("tier")));
    }
}
