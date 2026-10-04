//! Integration tests for the domain repositories (need Postgres via DATABASE_URL).

use std::path::Path;

use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::error::CoreError;
use talkops_core::extensions::{self, DeviceInput, DeviceKind, ExtensionInput};
use talkops_core::presets::PresetCatalog;
use talkops_core::tenant::TenantId;
use talkops_core::trunks::{self, AccountInput, NumberDestination, NumberInput, TrunkInput};
use talkops_core::users::{self, NewUser, Role};
use talkops_core::{audit, cdr, settings};
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

fn catalog() -> PresetCatalog {
    PresetCatalog::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets/trunks"))
        .unwrap()
}

fn ext(number: &str) -> ExtensionInput {
    ExtensionInput {
        number: number.into(),
        display_name: format!("Ext {number}"),
        user_id: None,
        outbound_number_id: None,
        hide_caller_id: false,
        ring_timeout_secs: 30,
        enabled: true,
    }
}

fn emergency() -> Vec<String> {
    vec!["110".into(), "112".into()]
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn first_admin_login_and_sessions(pool: PgPool) {
    let new = NewUser {
        username: "Admin".into(),
        display_name: "Admin".into(),
        email: None,
        role: Role::User, // forced to admin
        password: Some("supersecret1".into()),
    };
    let admin = users::create_first_admin(&pool, T, &new).await.unwrap();
    assert_eq!(admin.role, Role::Admin);
    assert!(matches!(
        users::create_first_admin(&pool, T, &new).await,
        Err(CoreError::Conflict(_))
    ));

    assert!(
        users::authenticate(&pool, T, "admin", "wrong-password")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        users::authenticate(&pool, T, "nobody", "supersecret1")
            .await
            .unwrap()
            .is_none()
    );
    let user = users::authenticate(&pool, T, "ADMIN", "supersecret1")
        .await
        .unwrap()
        .unwrap();

    let session = users::create_session(&pool, user.id, Some("127.0.0.1"), Some("test"))
        .await
        .unwrap();
    let su = users::session_user(&pool, &session.token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(su.user_id, user.id);
    assert_eq!(su.csrf_token, session.csrf_token);
    assert!(users::session_user(&pool, "bogus").await.unwrap().is_none());

    // Disabling the user invalidates the session.
    let upd = users::UserUpdate {
        display_name: "Admin".into(),
        email: None,
        role: Role::Admin,
        enabled: false,
    };
    users::update(&pool, T, user.id, &upd).await.unwrap();
    assert!(
        users::session_user(&pool, &session.token)
            .await
            .unwrap()
            .is_none()
    );

    assert!(matches!(
        users::create(
            &pool,
            T,
            &NewUser {
                username: "x".into(),
                display_name: "x".into(),
                email: None,
                role: Role::User,
                password: Some("short".into())
            }
        )
        .await,
        Err(CoreError::Validation(_))
    ));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn extensions_and_devices(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let e = extensions::create(&pool, T, &ext("20"), &emergency())
        .await
        .unwrap();
    assert!(matches!(
        extensions::create(&pool, T, &ext("20"), &emergency()).await,
        Err(CoreError::Conflict(_))
    ));

    let input = DeviceInput {
        name: "Desk".into(),
        kind: DeviceKind::Desk,
        sip_username: None,
        mac: Some("80:5E:C0:00:00:01".into()),
        model: Some("T54W".into()),
        enabled: true,
    };
    let (dev, password) = extensions::create_device(&pool, T, &sb, &e, &input)
        .await
        .unwrap();
    assert_eq!(dev.sip_username, "20-1");
    assert_eq!(dev.mac.as_deref(), Some("805ec0000001"));
    assert_eq!(sb.decrypt(&dev.sip_password_enc).unwrap(), password);
    let (dev2, _) = extensions::create_device(
        &pool,
        T,
        &sb,
        &e,
        &DeviceInput {
            mac: None,
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(dev2.sip_username, "20-2");

    let auth = extensions::device_auth(&pool, "20-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(auth.extension_number, "20");
    assert_eq!(
        extensions::ring_targets(&pool, e.id).await.unwrap(),
        vec!["20-1", "20-2"]
    );

    let new_pw = extensions::reset_device_password(&pool, T, &sb, dev.id)
        .await
        .unwrap();
    let auth = extensions::device_auth(&pool, "20-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sb.decrypt(&auth.sip_password_enc).unwrap(), new_pw);

    // Disabled extension: devices cannot authenticate.
    extensions::update(
        &pool,
        T,
        e.id,
        &ExtensionInput {
            enabled: false,
            ..ext("20")
        },
        &emergency(),
    )
    .await
    .unwrap();
    assert!(
        extensions::device_auth(&pool, "20-1")
            .await
            .unwrap()
            .is_none()
    );

    extensions::delete(&pool, T, e.id).await.unwrap();
    assert!(
        extensions::list_all_devices(&pool, T)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn trunks_accounts_numbers_routes(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let cat = catalog();

    // Presets without registrar need an override.
    let bad = TrunkInput {
        name: "VF".into(),
        preset: "vodafone-privat".into(),
        overrides: json!({}),
        enabled: true,
    };
    assert!(matches!(
        trunks::create(&pool, T, &cat, &bad).await,
        Err(CoreError::Validation(_))
    ));
    let unknown = TrunkInput {
        preset: "nope".into(),
        ..bad.clone()
    };
    assert!(trunks::create(&pool, T, &cat, &unknown).await.is_err());

    let trunk = trunks::create(
        &pool,
        T,
        &cat,
        &TrunkInput {
            name: "LEONET".into(),
            preset: "leonet".into(),
            overrides: json!({}),
            enabled: true,
        },
    )
    .await
    .unwrap();
    let acc = trunks::create_account(
        &pool,
        T,
        &sb,
        trunk.id,
        &AccountInput {
            username: "leo49891234567".into(),
            auth_username: String::new(),
            password: Some("pw".into()),
            enabled: true,
        },
    )
    .await
    .unwrap();
    assert!(
        trunks::create_account(
            &pool,
            T,
            &sb,
            Uuid::new_v4(),
            &AccountInput {
                username: "x".into(),
                auth_username: String::new(),
                password: Some("pw".into()),
                enabled: true
            }
        )
        .await
        .is_err()
    );

    let e = extensions::create(&pool, T, &ext("21"), &emergency())
        .await
        .unwrap();
    let num_input = NumberInput {
        trunk_id: trunk.id,
        account_id: Some(acc.id),
        e164: "+49891234567".into(),
        label: "Main".into(),
        destination_type: NumberDestination::Extension,
        destination_id: Some(e.id),
        enabled: true,
    };
    let num = trunks::create_number(&pool, T, &num_input).await.unwrap();
    assert!(
        trunks::create_number(
            &pool,
            T,
            &NumberInput {
                e164: "0891234".into(),
                ..num_input.clone()
            }
        )
        .await
        .is_err()
    );
    assert!(
        trunks::create_number(
            &pool,
            T,
            &NumberInput {
                destination_id: None,
                e164: "+49891234568".into(),
                ..num_input.clone()
            }
        )
        .await
        .is_err()
    );

    let (tenant, found) = trunks::find_number(&pool, "+49891234567")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tenant, T);
    assert_eq!(found.destination_id, Some(e.id));

    let route = trunks::outbound_route(&pool, T, num.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(route.account_id, acc.id);
    assert_eq!(route.preset, "leonet");

    let gws = trunks::active_gateways(&pool).await.unwrap();
    assert_eq!(gws.len(), 1);
    assert_eq!(gws[0].first_number.as_deref(), Some("+49891234567"));

    // Updating the account without password keeps the old secret.
    let acc2 = trunks::update_account(
        &pool,
        T,
        &sb,
        acc.id,
        &AccountInput {
            username: "leo49891234567".into(),
            auth_username: String::new(),
            password: None,
            enabled: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(sb.decrypt(&acc2.password_enc).unwrap(), "pw");

    // Disabling the trunk removes the route and the gateway.
    trunks::update(
        &pool,
        T,
        &cat,
        trunk.id,
        &TrunkInput {
            name: "LEONET".into(),
            preset: "leonet".into(),
            overrides: json!({}),
            enabled: false,
        },
    )
    .await
    .unwrap();
    assert!(
        trunks::outbound_route(&pool, T, num.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(trunks::active_gateways(&pool).await.unwrap().is_empty());

    // Settings: default number.
    let mut s = settings::get(&pool, T).await.unwrap();
    assert_eq!(s.country_code, "49");
    s.area_code = "89".into();
    s.default_number_id = Some(num.id);
    let s2 = settings::update(&pool, T, &s).await.unwrap();
    assert_eq!(s2, s);
    trunks::delete(&pool, T, trunk.id).await.unwrap();
    assert_eq!(
        settings::get(&pool, T).await.unwrap().default_number_id,
        None
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn cdr_and_audit(pool: PgPool) {
    let now = Utc::now();
    let c = cdr::Cdr {
        id: Uuid::nil(),
        call_uuid: "abc".into(),
        direction: cdr::Direction::Internal,
        caller_number: "20".into(),
        caller_name: "A".into(),
        destination: "21".into(),
        extension_id: Some(Uuid::new_v4()), // unknown -> stored as NULL
        trunk_id: None,
        number_id: None,
        started_at: now,
        answered_at: Some(now),
        ended_at: now,
        duration_secs: 5,
        billsec: 4,
        hangup_cause: "NORMAL_CLEARING".into(),
    };
    assert!(cdr::insert(&pool, T, &c).await.unwrap());
    assert!(
        !cdr::insert(&pool, T, &c).await.unwrap(),
        "duplicates are ignored"
    );
    let rows = cdr::list(
        &pool,
        T,
        &cdr::CdrQuery {
            search: Some("2".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].extension_id, None);
    assert!(
        cdr::list(
            &pool,
            T,
            &cdr::CdrQuery {
                search: Some("999".into()),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .is_empty()
    );

    let actor = audit::Actor {
        tenant: T,
        user_id: None,
        ip: Some("10.0.0.1".into()),
    };
    audit::record(
        &pool,
        &actor,
        "create",
        "trunk",
        Some("x".into()),
        json!({"name": "LEONET"}),
    )
    .await
    .unwrap();
    let log = audit::list(&pool, T, 10).await.unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].action, "create");
}
