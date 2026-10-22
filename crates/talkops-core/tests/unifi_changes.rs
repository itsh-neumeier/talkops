//! Voice SSID selection, write switch and change history (ADR 0042).
//! Needs Postgres via DATABASE_URL.

use serde_json::json;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::error::CoreError;
use talkops_core::tenant::TenantId;
use talkops_core::unifi::{self, UnifiInput};
use talkops_core::unifi_changes::{self as changes, NewChange, Outcome};
use talkops_core::wifi_survey as ws;
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const WIFI: &str = "6f1c2a3b-0000-4000-8000-000000000001";

async fn connect(pool: &PgPool, url: &str) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    unifi::save(
        pool,
        T,
        &sb,
        &UnifiInput {
            url: url.into(),
            site: "default".into(),
            cert_sha256: String::new(),
            api_key: Some("key-123".into()),
        },
    )
    .await
    .unwrap();
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn voice_ssid_and_write_switch(pool: PgPool) {
    // No connection: nothing to change.
    assert!(matches!(
        unifi::set_voice_wifi(&pool, T, Some((WIFI, "Voice")), true).await,
        Err(CoreError::NotFound)
    ));
    connect(&pool, "https://192.168.1.1").await;
    let c = unifi::get(&pool, T).await.unwrap().unwrap();
    assert!(!c.write_enabled, "off by default");
    assert_eq!(c.voice_wifi_id, None);

    let c = unifi::set_voice_wifi(&pool, T, Some((WIFI, "Voice")), true)
        .await
        .unwrap();
    assert_eq!(c.voice_wifi_id.as_deref(), Some(WIFI));
    assert_eq!(c.voice_wifi_name.as_deref(), Some("Voice"));
    assert!(c.write_enabled);
    assert!(
        unifi::set_voice_wifi(&pool, T, Some(("../x", "Voice")), false)
            .await
            .is_err()
    );

    // Saving the same console keeps the selection; the version check is new.
    connect(&pool, "https://192.168.1.1").await;
    let c = unifi::get(&pool, T).await.unwrap().unwrap();
    assert!(c.write_enabled && c.voice_wifi_id.is_some());
    unifi::record_check(&pool, T, Ok(("11.0.89", &json!({"site_id": "88f7"}))))
        .await
        .unwrap();
    assert_eq!(
        unifi::get(&pool, T).await.unwrap().unwrap().site_id(),
        Some("88f7")
    );

    // Another console clears the SSID and switches writes off.
    connect(&pool, "https://192.168.1.2").await;
    let c = unifi::get(&pool, T).await.unwrap().unwrap();
    assert!(!c.write_enabled);
    assert_eq!(c.voice_wifi_id, None);

    // Without a voice SSID, writes stay off.
    let c = unifi::set_voice_wifi(&pool, T, None, true).await.unwrap();
    assert!(!c.write_enabled);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn change_history(pool: PgPool) {
    let user: Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, username, display_name) VALUES ($1, 'ada', 'Ada') RETURNING id",
    )
    .bind(T)
    .fetch_one(&pool)
    .await
    .unwrap();
    let survey: Uuid = sqlx::query_scalar(
        "INSERT INTO wifi_surveys (tenant_id, mode, state, expires_at)
         VALUES ($1, 'originate', 'analysed', now() + interval '1 day') RETURNING id",
    )
    .bind(T)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!ws::any_active(&pool, T).await.unwrap());

    let new = NewChange {
        site_id: "88f7",
        wifi_id: WIFI,
        wifi_name: "Voice",
        kind: "apply",
        rollback_of: None,
        survey_id: Some(survey),
        fields: vec!["uapsdEnabled".into()],
        network_version: Some("11.0.89"),
        created_by: Some(user),
    };
    let id = changes::begin(&pool, T, &new).await.unwrap();
    assert!(changes::applying(&pool, T).await.unwrap());
    // One change at a time.
    assert!(matches!(
        changes::begin(&pool, T, &new).await,
        Err(CoreError::Conflict(m)) if m == "change_in_progress"
    ));
    let before = json!({"uapsdEnabled": {"value": false}});
    let after = json!({"uapsdEnabled": {"value": true}});
    changes::finish(
        &pool,
        T,
        id,
        &Outcome {
            state: changes::state::APPLIED,
            wifi_name: Some("Voice 2"),
            before: Some(&before),
            after: Some(&after),
            error: None,
        },
    )
    .await
    .unwrap();
    assert!(!changes::applying(&pool, T).await.unwrap());
    let c = changes::get(&pool, T, id).await.unwrap();
    assert_eq!(c.state, "applied");
    assert_eq!(c.wifi_name, "Voice 2");
    assert_eq!(c.created_by_name.as_deref(), Some("Ada"));
    assert_eq!(c.after, Some(after.clone()));
    assert!(c.can_roll_back());
    assert!(c.finished_at.is_some());

    // A rollback row reverts the change.
    let rb = changes::begin(
        &pool,
        T,
        &NewChange {
            kind: "rollback",
            rollback_of: Some(id),
            fields: new.fields.clone(),
            ..new
        },
    )
    .await
    .unwrap();
    changes::finish(
        &pool,
        T,
        rb,
        &Outcome {
            state: changes::state::FAILED,
            wifi_name: None,
            before: None,
            after: None,
            error: Some("forbidden"),
        },
    )
    .await
    .unwrap();
    changes::mark_rolled_back(&pool, T, id).await.unwrap();
    let c = changes::get(&pool, T, id).await.unwrap();
    assert_eq!(c.state, "rolled_back");
    assert!(!c.can_roll_back());
    let list = changes::list(&pool, T, 50).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, rb);
    assert_eq!(list[0].error.as_deref(), Some("forbidden"));
    assert!(!list[0].can_roll_back());

    // A stale "applying" row does not block forever.
    let stale = changes::begin(&pool, T, &new).await.unwrap();
    sqlx::query(
        "UPDATE unifi_wifi_changes SET created_at = now() - interval '10 minutes' WHERE id = $1",
    )
    .bind(stale)
    .execute(&pool)
    .await
    .unwrap();
    assert!(!changes::applying(&pool, T).await.unwrap());
    changes::begin(&pool, T, &new).await.unwrap();
    assert_eq!(
        changes::get(&pool, T, stale)
            .await
            .unwrap()
            .error
            .as_deref(),
        Some("interrupted")
    );

    // The history stays when the survey is deleted.
    sqlx::query("DELETE FROM wifi_surveys WHERE id = $1")
        .bind(survey)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(changes::get(&pool, T, id).await.unwrap().survey_id, None);
    assert!(matches!(
        changes::get(&pool, T, Uuid::new_v4()).await,
        Err(CoreError::NotFound)
    ));

    sqlx::query("UPDATE wifi_surveys SET state = 'running'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO wifi_surveys (tenant_id, mode, state, expires_at)
         VALUES ($1, 'self_dial', 'armed', now() + interval '1 day')",
    )
    .bind(T)
    .execute(&pool)
    .await
    .unwrap();
    assert!(ws::any_active(&pool, T).await.unwrap());
}
