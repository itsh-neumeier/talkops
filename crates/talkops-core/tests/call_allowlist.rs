use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::{audit, blocking, cdr, stats, tenant::TenantId};
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn migration_preserves_existing_records_without_inventing_a_reason(db: PgPool) {
    // Recreate the pre-upgrade schema inside this isolated database.
    sqlx::raw_sql("DROP TABLE call_allowlist; ALTER TABLE cdr DROP COLUMN blocked_reason;
        INSERT INTO cdr(tenant_id,call_uuid,direction,caller_number,destination,started_at,ended_at,hangup_cause)
        VALUES('00000000-0000-4000-8000-000000000001','legacy','inbound','+4930123456','+4930999999',now(),now(),'CALL_REJECTED');")
        .execute(&db).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261106000003_call_allowlist_and_block_reason.sql"
    ))
    .execute(&db)
    .await
    .unwrap();
    let calls = cdr::list(&db, T, &cdr::CdrQuery::default()).await.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].hangup_cause, "CALL_REJECTED");
    assert_eq!(calls[0].blocked_reason, None);
    assert!(blocking::allowlist(&db, T).await.unwrap().is_empty());
}

fn actor(tenant: TenantId) -> audit::Actor {
    audit::Actor {
        tenant,
        user_id: None,
        ip: None,
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn whitelist_is_exact_tenant_scoped_idempotent_and_audited(db: PgPool) {
    let input = blocking::CallAllowInput {
        number: "+4930123456".into(),
        label: " Trusted ".into(),
    };
    let a = actor(T);
    let (first, second) = tokio::join!(
        blocking::allow(&db, &a, &input),
        blocking::allow(&db, &a, &input)
    );
    let (first, created_a) = first.unwrap();
    let (second, created_b) = second.unwrap();
    assert_eq!(first.id, second.id);
    assert_ne!(created_a, created_b);
    assert_eq!(first.label, "Trusted");
    assert_eq!(blocking::allowlist(&db, T).await.unwrap().len(), 1);
    assert_eq!(
        blocking::allowed_numbers(&db, T, &[input.number.clone(), "+49301234567".into()])
            .await
            .unwrap(),
        [input.number.clone()]
    );
    let other: Uuid = sqlx::query_scalar("INSERT INTO tenants(name) VALUES('Other') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(
        blocking::allowlist(&db, TenantId(other))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        blocking::allowed_numbers(&db, TenantId(other), &[input.number.clone()])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        blocking::disallow(&db, &actor(TenantId(other)), first.id)
            .await
            .is_err()
    );
    let entries = audit::list(&db, T, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].entity_type, "call_allowlist");
    assert_eq!(
        entries[0].details,
        json!({"number": input.number, "label": "Trusted"})
    );
    blocking::disallow(&db, &a, first.id).await.unwrap();
    assert!(blocking::allowlist(&db, T).await.unwrap().is_empty());
    let entries = audit::list(&db, T, 10).await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].action, "delete");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn failed_audit_rolls_back_whitelist_writes(db: PgPool) {
    let bad_actor = audit::Actor {
        user_id: Some(Uuid::new_v4()),
        ..actor(T)
    };
    let input = blocking::CallAllowInput {
        number: "+4930123456".into(),
        label: "".into(),
    };
    assert!(blocking::allow(&db, &bad_actor, &input).await.is_err());
    assert!(blocking::allowlist(&db, T).await.unwrap().is_empty());
    let (entry, _) = blocking::allow(&db, &actor(T), &input).await.unwrap();
    assert!(blocking::disallow(&db, &bad_actor, entry.id).await.is_err());
    assert_eq!(blocking::allowlist(&db, T).await.unwrap().len(), 1);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn blocked_filters_pages_scopes_and_statistics_agree(db: PgPool) {
    let now = Utc::now();
    for (id, reason, answered, cause) in [
        ("answered", None, true, "NORMAL_CLEARING"),
        ("missed", None, false, "NO_ANSWER"),
        ("declined", None, false, "CALL_REJECTED"),
        ("list", Some("list:+4930*"), false, "CALL_REJECTED"),
        ("phoneblock", Some("phoneblock:12"), false, "CALL_REJECTED"),
        ("anonymous", Some("anonymous"), false, "CALL_REJECTED"),
    ] {
        let call = cdr::Cdr {
            id: Uuid::nil(),
            call_uuid: id.into(),
            direction: cdr::Direction::Inbound,
            caller_number: "+4930123456".into(),
            caller_name: "".into(),
            destination: "+4930999999".into(),
            extension_id: None,
            dest_extension_id: None,
            trunk_id: None,
            number_id: None,
            started_at: now,
            answered_at: answered.then_some(now),
            ended_at: now,
            duration_secs: 0,
            billsec: if answered { 10 } else { 0 },
            hangup_cause: cause.into(),
            blocked_reason: reason.map(str::to_owned),
            recording_id: None,
        };
        assert!(cdr::insert(&db, T, &call).await.unwrap());
        assert!(!cdr::insert(&db, T, &call).await.unwrap());
    }
    for (result, expected) in [
        ("blocked", 3),
        ("busy", 1),
        ("missed", 1),
        ("answered", 1),
        ("busy,blocked", 4),
    ] {
        let q = cdr::CdrQuery {
            result: Some(result.into()),
            ..Default::default()
        };
        assert_eq!(
            cdr::count(&db, T, &q, &cdr::CdrScope::default())
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            cdr::list(&db, T, &q).await.unwrap().len(),
            expected as usize
        );
        let scoped = cdr::CdrScope {
            extensions: Some(vec![]),
            ..Default::default()
        };
        assert!(
            cdr::list_scoped(&db, T, &q, &scoped)
                .await
                .unwrap()
                .is_empty()
        );
    }
    let q = cdr::CdrQuery {
        result: Some("blocked".into()),
        direction: Some("inbound".into()),
        from: Some("4930".into()),
        limit: Some(1),
        offset: Some(1),
        ..Default::default()
    };
    let page = cdr::list(&db, T, &q).await.unwrap();
    assert_eq!(page.len(), 1);
    assert!(page[0].blocked_reason.is_some());
    assert_eq!(
        cdr::count(&db, T, &q, &cdr::CdrScope::default())
            .await
            .unwrap(),
        3
    );
    let s = stats::calls(&db, T, stats::Range::Hour, now, "Europe/Berlin")
        .await
        .unwrap();
    assert_eq!((s.total, s.inbound, s.blocked, s.missed), (6, 6, 3, 2));
    assert_eq!(s.answer_rate, Some(33.3));
    sqlx::query("DELETE FROM cdr WHERE blocked_reason IS NULL")
        .execute(&db)
        .await
        .unwrap();
    let s = stats::calls(&db, T, stats::Range::Hour, now, "Europe/Berlin")
        .await
        .unwrap();
    assert_eq!((s.total, s.blocked, s.missed), (3, 3, 0));
    assert_eq!(s.answer_rate, None);
}
