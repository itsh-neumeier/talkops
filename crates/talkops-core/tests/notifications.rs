//! Integration tests for mail notifications (need Postgres via DATABASE_URL).

use serde_json::json;
use sqlx::PgPool;
use talkops_core::audit::{self, Actor};
use talkops_core::backups;
use talkops_core::error::CoreError;
use talkops_core::notifications::{self as n, AlertState, NewDelivery, RuleInput};
use talkops_core::tenant::TenantId;
use talkops_core::users::{self, NewUser, Role};

const T: TenantId = TenantId::DEFAULT;

fn user(name: &str, role: Role, email: Option<&str>) -> NewUser {
    NewUser {
        username: name.into(),
        display_name: name.into(),
        email: email.map(str::to_owned),
        role,
        role_id: None,
        password: Some("supersecret1".into()),
    }
}

fn input() -> RuleInput {
    serde_json::from_value(json!({"enabled": true})).unwrap()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn rules_defaults_and_recipients(pool: PgPool) {
    let admin = users::create(
        &pool,
        T,
        &user("admin", Role::Admin, Some("Admin@Example.com")),
    )
    .await
    .unwrap();
    users::create(
        &pool,
        T,
        &user("anna", Role::User, Some("anna@example.com")),
    )
    .await
    .unwrap();
    users::create(&pool, T, &user("noaddr", Role::Admin, None))
        .await
        .unwrap();

    let rules = n::rules(&pool, T).await.unwrap();
    assert!(rules.iter().all(|r| !r.custom));
    assert!(!rules.iter().any(|r| r.event_type == "trunk.up"));
    let down = rules.iter().find(|r| r.event_type == "trunk.down").unwrap();
    assert!(down.enabled);
    assert_eq!(down.delay_secs, 120);
    // Default recipients: the admin role, users with an address only.
    assert_eq!(
        n::recipients(&pool, T, down).await.unwrap(),
        ["admin@example.com"]
    );

    let d = n::def("backup.failed").unwrap();
    let mut i = input();
    i.recipient_roles = vec![];
    i.recipient_users = vec![admin.id];
    i.recipient_emails = vec![" Ops@Example.com ".into(), "ops@example.com".into()];
    i.delay_secs = 30;
    let saved = n::save_rule(&pool, T, d, &i).await.unwrap();
    assert!(saved.custom);
    assert_eq!(saved.recipient_emails, ["ops@example.com"]);
    assert_eq!(
        n::recipients(&pool, T, &saved).await.unwrap(),
        ["admin@example.com", "ops@example.com"]
    );

    let mut bad = input();
    bad.recipient_emails = vec!["not an address".into()];
    assert!(matches!(
        n::save_rule(&pool, T, d, &bad).await,
        Err(CoreError::Validation(_))
    ));
    let mut bad = input();
    bad.recipient_users = vec![uuid::Uuid::new_v4()];
    assert!(matches!(
        n::save_rule(&pool, T, d, &bad).await,
        Err(CoreError::Validation(_))
    ));
    let mut bad = input();
    bad.min_severity = "huge".into();
    assert!(n::save_rule(&pool, T, d, &bad).await.is_err());
    assert!(
        n::save_rule(&pool, T, n::def("trunk.up").unwrap(), &input())
            .await
            .is_err()
    );

    let reset = n::reset_rule(&pool, T, d).await.unwrap();
    assert!(!reset.custom);
    assert_eq!(reset.delay_secs, 0);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn producers_write_the_outbox(pool: PgPool) {
    let admin = users::create(&pool, T, &user("admin", Role::Admin, None))
        .await
        .unwrap();
    let actor = Actor {
        tenant: T,
        user_id: Some(admin.id),
        ip: Some("192.0.2.7".into()),
    };
    // Configuration change: mapped; door opener: not mapped.
    audit::record(
        &pool,
        &actor,
        "update",
        "smtp",
        None,
        json!({"secret": "x"}),
    )
    .await
    .unwrap();
    audit::record(
        &pool,
        &actor,
        "open",
        "door_station",
        Some("d1".into()),
        json!({}),
    )
    .await
    .unwrap();
    backups::record_run(&pool, T, Err("disk full"))
        .await
        .unwrap();
    n::emit(&pool, T, "trunk.down", "gw-1", json!({"name": "A"}))
        .await
        .unwrap();
    assert!(n::emit(&pool, T, "no.such", "", json!({})).await.is_err());

    let events = n::claim_events(&pool, 10).await.unwrap();
    let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(types, ["config.system", "backup.failed", "trunk.down"]);
    let cfg = &events[0];
    assert_eq!(cfg.params["user"], "admin");
    assert_eq!(cfg.params["entity"], "smtp");
    assert!(cfg.params.get("secret").is_none());
    assert_eq!(events[1].severity, "critical");
    // Claimed events are not handed out again.
    assert!(n::claim_events(&pool, 10).await.unwrap().is_empty());
    assert_eq!(n::recent_events(&pool, T, 10).await.unwrap().len(), 3);

    // Admin login addresses: the first address is not "new".
    assert!(
        !n::new_login_address(&pool, T, admin.id, "192.0.2.1")
            .await
            .unwrap()
    );
    assert!(
        !n::new_login_address(&pool, T, admin.id, "192.0.2.1")
            .await
            .unwrap()
    );
    assert!(
        n::new_login_address(&pool, T, admin.id, "192.0.2.2")
            .await
            .unwrap()
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn alerts_digests_and_log(pool: PgPool) {
    n::emit(&pool, T, "trunk.down", "gw-1", json!({}))
        .await
        .unwrap();
    n::emit(&pool, T, "config.system", "", json!({}))
        .await
        .unwrap();
    n::emit(&pool, T, "config.system", "", json!({}))
        .await
        .unwrap();
    let events = n::claim_events(&pool, 10).await.unwrap();

    // Pending problem with a delay; a second open is ignored.
    n::open_problem(&pool, &events[0], 60).await.unwrap();
    n::open_problem(&pool, &events[0], 0).await.unwrap();
    let alert = n::open_alert(&pool, T, "trunk.down", "gw-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(alert.state(), Some(AlertState::Pending));
    assert!(n::due_alerts(&pool).await.unwrap().is_empty());
    n::mark_alerted(&pool, alert.id).await.unwrap();
    let alert = n::open_alert(&pool, T, "trunk.down", "gw-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(alert.state(), Some(AlertState::Alerted));
    n::close_alert(&pool, alert.id).await.unwrap();
    assert!(
        n::open_alert(&pool, T, "trunk.down", "gw-1")
            .await
            .unwrap()
            .is_none()
    );

    // Digest collects the events; with time 0 it is due at once.
    n::add_to_digest(&pool, &events[1], 0).await.unwrap();
    n::add_to_digest(&pool, &events[2], 0).await.unwrap();
    let due = n::due_alerts(&pool).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].kind, "digest");
    assert_eq!(due[0].event_ids, [events[1].id, events[2].id]);
    assert_eq!(
        n::events_by_ids(&pool, &due[0].event_ids)
            .await
            .unwrap()
            .len(),
        2
    );

    // Rate limit counts sent mails of the last hour (not tests).
    for (kind, status) in [("alert", "sent"), ("test", "sent"), ("alert", "failed")] {
        n::log_delivery(
            &pool,
            T,
            &NewDelivery {
                event_type: "trunk.down",
                kind,
                subject: "s",
                recipients: &["a@example.com".to_owned()],
                status,
                error: None,
                event_count: 1,
            },
        )
        .await
        .unwrap();
    }
    assert_eq!(n::sent_last_hour(&pool, T, "trunk.down").await.unwrap(), 1);
    assert_eq!(n::deliveries(&pool, T, 10).await.unwrap().len(), 3);
    assert_eq!(n::purge(&pool, 30).await.unwrap(), 0);
}
