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
use talkops_core::voicemail::{self, NewMessage, VoicemailBoxInput};
use talkops_core::{audit, cdr, jobs, mail, recordings, settings};
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
        record_calls: "inherit".into(),
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
        recording_id: None,
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

fn vm_input() -> VoicemailBoxInput {
    VoicemailBoxInput {
        enabled: true,
        pin: Some("1234".into()),
        email_notify: true,
        attach_audio: true,
        language: None,
        greeting: "default".into(),
        greeting_text: String::new(),
        max_message_secs: 120,
        greeting_clip_id: None,
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn voicemail_boxes_messages_and_smtp(pool: PgPool) {
    let e = extensions::create(&pool, T, &ext("20"), &[]).await.unwrap();

    // Unconfigured boxes are disabled.
    let b = voicemail::get_box(&pool, T, e.id).await.unwrap();
    assert!(!b.enabled && !b.has_pin());

    // Validation.
    for bad in [
        VoicemailBoxInput {
            pin: Some("12".into()),
            ..vm_input()
        },
        VoicemailBoxInput {
            pin: Some("12ab".into()),
            ..vm_input()
        },
        VoicemailBoxInput {
            language: Some("fr".into()),
            ..vm_input()
        },
        VoicemailBoxInput {
            greeting: "tts".into(),
            ..vm_input()
        },
        VoicemailBoxInput {
            greeting: "recorded".into(),
            ..vm_input()
        },
        VoicemailBoxInput {
            max_message_secs: 5,
            ..vm_input()
        },
    ] {
        assert!(matches!(
            voicemail::update_box(&pool, T, e.id, &bad, "de").await,
            Err(CoreError::Validation(_))
        ));
    }
    assert!(matches!(
        voicemail::update_box(&pool, T, Uuid::new_v4(), &vm_input(), "de").await,
        Err(CoreError::NotFound)
    ));

    let b = voicemail::update_box(&pool, T, e.id, &vm_input(), "de")
        .await
        .unwrap();
    assert!(b.enabled && b.verify_pin("1234") && !b.verify_pin("4321"));
    // null keeps the PIN.
    let b = voicemail::update_box(
        &pool,
        T,
        e.id,
        &VoicemailBoxInput {
            pin: None,
            ..vm_input()
        },
        "de",
    )
    .await
    .unwrap();
    assert!(b.verify_pin("1234"));

    // TTS greeting: queued once, status follows the worker.
    let tts = VoicemailBoxInput {
        pin: None,
        greeting: "tts".into(),
        greeting_text: "Hallo, hier ist die Mailbox von Anna.".into(),
        ..vm_input()
    };
    let b = voicemail::update_box(&pool, T, e.id, &tts, "en")
        .await
        .unwrap();
    assert_eq!(b.greeting_status, "pending");
    assert!(!b.uses_custom_greeting());
    voicemail::update_box(&pool, T, e.id, &tts, "en")
        .await
        .unwrap();
    let job = jobs::claim(&pool, "w", &[voicemail::JOB_TTS_GREETING])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.payload["language"], "en");
    assert_eq!(job.payload["file"], voicemail::greeting_file(T, e.id));
    assert!(
        jobs::claim(&pool, "w", &[voicemail::JOB_TTS_GREETING])
            .await
            .unwrap()
            .is_none()
    );
    voicemail::set_greeting_status(&pool, e.id, "Hallo, hier ist die Mailbox von Anna.", true)
        .await
        .unwrap();
    assert!(
        voicemail::get_box(&pool, T, e.id)
            .await
            .unwrap()
            .uses_custom_greeting()
    );
    voicemail::set_recorded_greeting(&pool, T, e.id)
        .await
        .unwrap();
    assert_eq!(
        voicemail::get_box(&pool, T, e.id).await.unwrap().greeting,
        "recorded"
    );

    // Messages: create queues the mail, list orders new first.
    let first = voicemail::create_message(
        &pool,
        T,
        &NewMessage {
            id: Uuid::new_v4(),
            extension_id: e.id,
            caller_number: "+4930123".into(),
            caller_name: "Anna".into(),
            duration_secs: 7,
            call_uuid: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(first.file, voicemail::message_file(T, e.id, first.id));
    let second = voicemail::create_message(
        &pool,
        T,
        &NewMessage {
            id: Uuid::new_v4(),
            extension_id: e.id,
            caller_number: "21".into(),
            caller_name: String::new(),
            duration_secs: 3,
            call_uuid: Some("abc".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(voicemail::counts(&pool, e.id).await.unwrap(), (2, 0));
    let heard = voicemail::set_message_status(&pool, T, first.id, true)
        .await
        .unwrap();
    assert!(heard.heard_at.is_some());
    let list = voicemail::list_messages(&pool, T, e.id).await.unwrap();
    assert_eq!(list[0].id, second.id);
    assert_eq!(voicemail::counts(&pool, e.id).await.unwrap(), (1, 1));

    let info = voicemail::mail_info(&pool, first.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(info.extension_number, "20");
    assert!(info.email.is_none());
    let mails = jobs::claim(&pool, "w", &[voicemail::JOB_MAIL])
        .await
        .unwrap();
    assert!(mails.is_some());

    voicemail::delete_message(&pool, T, first.id).await.unwrap();
    assert!(matches!(
        voicemail::get_message(&pool, T, first.id).await,
        Err(CoreError::Db(_)) | Err(CoreError::NotFound)
    ));
    assert_eq!(voicemail::counts(&pool, e.id).await.unwrap(), (1, 0));

    // With transcription, the mail waits for the transcript.
    while jobs::claim(&pool, "w", &[voicemail::JOB_MAIL])
        .await
        .unwrap()
        .is_some()
    {}
    sqlx::query("UPDATE tenant_settings SET transcription_enabled = true")
        .execute(&pool)
        .await
        .unwrap();
    let third = voicemail::create_message(
        &pool,
        T,
        &NewMessage {
            id: Uuid::new_v4(),
            extension_id: e.id,
            caller_number: "22".into(),
            caller_name: String::new(),
            duration_secs: 4,
            call_uuid: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(third.transcript_status, "pending");
    assert!(
        jobs::claim(&pool, "w", &[voicemail::JOB_MAIL])
            .await
            .unwrap()
            .is_none()
    );
    let job = jobs::claim(&pool, "w", &[recordings::JOB_TRANSCRIBE])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.payload["voicemail_id"], third.id.to_string());
    recordings::save_transcript(
        &pool,
        T,
        recordings::Source::Voicemail(third.id),
        "de",
        &[recordings::Segment {
            start: 0.0,
            end: 2.0,
            speaker: String::new(),
            text: " Ruf mich zurück.".into(),
        }],
    )
    .await
    .unwrap();
    let third = voicemail::get_message(&pool, T, third.id).await.unwrap();
    assert_eq!(third.transcript_status, "done");
    let info = voicemail::mail_info(&pool, third.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(info.transcript.as_deref(), Some("Ruf mich zurück."));
    let hits = recordings::search(&pool, T, "zurück", Some(&[e.id]), 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].voicemail_id, Some(third.id));
    assert_eq!(hits[0].destination.as_deref(), Some("20"));
    assert!(
        recordings::search(&pool, T, "zurück", Some(&[]), 10)
            .await
            .unwrap()
            .is_empty()
    );

    // SMTP settings: password encrypted, kept on null, removed on "".
    let secrets = SecretBox::from_hex(KEY).unwrap();
    assert!(mail::config(&pool, T, &secrets).await.unwrap().is_none());
    let input = mail::SmtpInput {
        host: "smtp.example.com".into(),
        port: 587,
        security: "starttls".into(),
        username: "pbx".into(),
        password: Some("s3cret".into()),
        from: "TalkOps <pbx@example.com>".into(),
    };
    let s = mail::update(&pool, T, &secrets, &input).await.unwrap();
    assert!(s.has_password);
    let s = mail::update(
        &pool,
        T,
        &secrets,
        &mail::SmtpInput {
            password: None,
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert!(s.has_password);
    let c = mail::config(&pool, T, &secrets).await.unwrap().unwrap();
    assert_eq!(c.password.as_deref(), Some("s3cret"));
    let s = mail::update(
        &pool,
        T,
        &secrets,
        &mail::SmtpInput {
            password: Some(String::new()),
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert!(!s.has_password);
    assert!(
        mail::update(
            &pool,
            T,
            &secrets,
            &mail::SmtpInput {
                security: "ssl".into(),
                ..input.clone()
            }
        )
        .await
        .is_err()
    );
    assert!(
        mail::update(
            &pool,
            T,
            &secrets,
            &mail::SmtpInput {
                from: "nobody".into(),
                ..input
            }
        )
        .await
        .is_err()
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn door_stations_and_events(pool: PgPool) {
    use talkops_core::doors::{self, DoorStationInput};
    let secrets = SecretBox::from_hex(KEY).unwrap();
    let door = extensions::create(&pool, T, &ext("8001"), &emergency())
        .await
        .unwrap();
    let input = DoorStationInput {
        name: "Gate".into(),
        extension_id: door.id,
        host: "192.168.1.20".into(),
        port: 80,
        username: "admin".into(),
        password: Some("pw".into()),
        doors: 2,
        destination_type: NumberDestination::None,
        destination_id: None,
        buttons: vec![],
        events_enabled: true,
        snapshots: true,
        webhook_url: Some("https://ha.local/api/webhook/abc".into()),
        enabled: true,
    };
    let s = doors::create(&pool, T, &secrets, &input).await.unwrap();
    // None keeps password and webhook, "" removes them.
    let s2 = doors::update(
        &pool,
        T,
        &secrets,
        s.id,
        &DoorStationInput {
            password: None,
            webhook_url: Some(String::new()),
            name: "Gate 2".into(),
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert!(s2.has_password && !s2.has_webhook);
    assert_eq!(s2.name, "Gate 2");
    let conn = doors::connection(&pool, &secrets, s.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(conn.password, "pw");
    assert_eq!(doors::with_api(&pool).await.unwrap().len(), 1);
    assert_eq!(
        doors::for_extension(&pool, door.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        s.id
    );
    // Online state changes are reported once.
    assert!(
        doors::set_online(&pool, s.id, true, Some("VTO2202F-P"))
            .await
            .unwrap()
    );
    assert!(!doors::set_online(&pool, s.id, true, None).await.unwrap());
    let s3 = doors::get(&pool, T, s.id).await.unwrap();
    assert!(s3.online && s3.last_seen.is_some());
    assert_eq!(s3.model, "VTO2202F-P");
    assert!(doors::set_online(&pool, s.id, false, None).await.unwrap());
    // Unknown kinds are rejected; old events are purged with the recording
    // retention (default 90 days).
    assert!(
        doors::add_event(&pool, T, s.id, "nope", serde_json::json!({}))
            .await
            .is_err()
    );
    let e = doors::add_event(&pool, T, s.id, "ring", serde_json::json!({}))
        .await
        .unwrap();
    doors::set_snapshot(&pool, e.id, "t/2026-01/x.jpg")
        .await
        .unwrap();
    doors::add_event(&pool, T, s.id, "opened", serde_json::json!({}))
        .await
        .unwrap();
    sqlx::query("UPDATE door_events SET created_at = now() - interval '91 days' WHERE id = $1")
        .bind(e.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        doors::purge_events(&pool, 100).await.unwrap(),
        ["t/2026-01/x.jpg"]
    );
    assert_eq!(
        doors::list_events(&pool, T, Some(s.id), 10)
            .await
            .unwrap()
            .len(),
        1
    );
    // Deleting the extension removes the station.
    extensions::delete(&pool, T, door.id).await.unwrap();
    assert!(doors::list(&pool, T).await.unwrap().is_empty());
}
