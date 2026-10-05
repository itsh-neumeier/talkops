//! Integration tests for the domain repositories (need Postgres via DATABASE_URL).

use std::path::Path;

use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::error::CoreError;
use talkops_core::extensions::{self, DeviceInput, DeviceKind, ExtensionInput};
use talkops_core::phones::{self, ContactInput, PhoneInput};
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
        dnd: false,
        forward_all: None,
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
        phone_id: None,
        account_index: None,
        enabled: true,
    };
    let (dev, password) = extensions::create_device(&pool, T, &sb, &e, &input)
        .await
        .unwrap();
    assert_eq!(dev.sip_username, "20-1");
    assert_eq!(sb.decrypt(&dev.sip_password_enc).unwrap(), password);
    let (dev2, _) = extensions::create_device(&pool, T, &sb, &e, &input)
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
        dest_extension_id: None,
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

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn phones_accounts_firmware_contacts(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let phone = phones::create(
        &pool,
        T,
        &PhoneInput {
            mac: "80:5E:C0:AA:BB:CC".into(),
            model: "t54w".into(),
            name: "Desk".into(),
            line_keys: json!([]),
        },
    )
    .await
    .unwrap();
    assert_eq!(phone.mac, "805ec0aabbcc");
    assert!(
        phones::create(
            &pool,
            T,
            &PhoneInput {
                mac: "805ec0aabbcc".into(),
                model: "t54w".into(),
                name: "Dup".into(),
                line_keys: json!([])
            }
        )
        .await
        .is_err()
    );
    assert!(
        phones::create(
            &pool,
            T,
            &PhoneInput {
                mac: "xyz".into(),
                model: "t54w".into(),
                name: "Bad".into(),
                line_keys: json!([])
            }
        )
        .await
        .is_err()
    );

    let e20 = extensions::create(&pool, T, &ext("20"), &emergency())
        .await
        .unwrap();
    let e21 = extensions::create(&pool, T, &ext("21"), &emergency())
        .await
        .unwrap();
    let on_phone = |name: &str| DeviceInput {
        name: name.into(),
        kind: DeviceKind::Desk,
        sip_username: None,
        phone_id: Some(phone.id),
        account_index: None,
        enabled: true,
    };
    let (d1, _) = extensions::create_device(&pool, T, &sb, &e20, &on_phone("A"))
        .await
        .unwrap();
    let (d2, _) = extensions::create_device(&pool, T, &sb, &e21, &on_phone("B"))
        .await
        .unwrap();
    assert_eq!(
        (d1.account_index, d2.account_index),
        (Some(1), Some(2)),
        "slots are assigned in order"
    );
    let clash = DeviceInput {
        account_index: Some(1),
        ..on_phone("C")
    };
    assert!(
        extensions::create_device(&pool, T, &sb, &e21, &clash)
            .await
            .is_err(),
        "slot already taken"
    );

    let accounts = phones::accounts(&pool, phone.id).await.unwrap();
    assert_eq!(
        accounts
            .iter()
            .map(|a| a.extension_number.as_str())
            .collect::<Vec<_>>(),
        ["20", "21"]
    );
    let (tenant, found) = phones::find_by_mac(&pool, "805ec0aabbcc")
        .await
        .unwrap()
        .unwrap();
    assert_eq!((tenant, found.id), (T, phone.id));
    phones::record_seen(&pool, phone.id, Some("10.0.0.5"), Some("96.86.0.70"))
        .await
        .unwrap();
    assert_eq!(
        phones::get(&pool, T, phone.id)
            .await
            .unwrap()
            .last_firmware
            .as_deref(),
        Some("96.86.0.70")
    );

    // Deleting the phone keeps the devices (unplaced).
    phones::delete(&pool, T, phone.id).await.unwrap();
    assert_eq!(
        extensions::get_device(&pool, T, d1.id)
            .await
            .unwrap()
            .phone_id,
        None
    );

    // Provisioning secrets are generated once and stay stable.
    let s1 = phones::provisioning_secrets(&pool, T, &sb, false)
        .await
        .unwrap();
    let s2 = phones::provisioning_secrets(&pool, T, &sb, false)
        .await
        .unwrap();
    assert_eq!(s1.password, s2.password);
    assert_eq!(s1.username, "provision");
    let s3 = phones::provisioning_secrets(&pool, T, &sb, true)
        .await
        .unwrap();
    assert_ne!(s1.password, s3.password);

    // Only one active firmware per model.
    let f1 = phones::create_firmware(
        &pool,
        T,
        uuid::Uuid::new_v4(),
        "t54w",
        "T54W-1.rom",
        10,
        "aa",
    )
    .await
    .unwrap();
    let f2 = phones::create_firmware(
        &pool,
        T,
        uuid::Uuid::new_v4(),
        "t54w",
        "T54W-2.rom",
        10,
        "bb",
    )
    .await
    .unwrap();
    phones::set_firmware_active(&pool, T, f1.id, true)
        .await
        .unwrap();
    phones::set_firmware_active(&pool, T, f2.id, true)
        .await
        .unwrap();
    assert_eq!(
        phones::active_firmware(&pool, T, "t54w")
            .await
            .unwrap()
            .unwrap()
            .id,
        f2.id
    );
    assert!(!phones::get_firmware(&pool, T, f1.id).await.unwrap().active);

    let c = phones::create_contact(
        &pool,
        T,
        &ContactInput {
            name: "Taxi".into(),
            company: String::new(),
            phone_work: "089 / 12 34-5".into(),
            phone_mobile: String::new(),
            phone_other: String::new(),
        },
    )
    .await
    .unwrap();
    assert_eq!(c.phone_work, "08912345");
    assert!(
        phones::create_contact(
            &pool,
            T,
            &ContactInput {
                name: "X".into(),
                company: String::new(),
                phone_work: String::new(),
                phone_mobile: String::new(),
                phone_other: String::new()
            }
        )
        .await
        .is_err()
    );
    assert!(
        phones::create_contact(
            &pool,
            T,
            &ContactInput {
                name: "X".into(),
                company: String::new(),
                phone_work: "abc".into(),
                phone_mobile: String::new(),
                phone_other: String::new()
            }
        )
        .await
        .is_err()
    );

    // DND and forwarding via feature codes.
    extensions::set_dnd(&pool, T, e20.id, true).await.unwrap();
    extensions::set_forward_all(&pool, T, e20.id, Some("030123"))
        .await
        .unwrap();
    let e = extensions::get(&pool, T, e20.id).await.unwrap();
    assert!(e.dnd);
    assert_eq!(e.forward_all.as_deref(), Some("030123"));
    assert!(
        extensions::set_forward_all(&pool, T, e20.id, Some("x"))
            .await
            .is_err()
    );
}
