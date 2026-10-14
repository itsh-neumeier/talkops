//! Integration tests for the domain repositories (need Postgres via DATABASE_URL).

use std::path::Path;

use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::error::CoreError;
use talkops_core::extensions::{
    self, CodecProfile, DeviceInput, DeviceKind, ExtensionInput, SipPasswordFormat,
};
use talkops_core::phones::{self, ContactInput, MediaKind, PhoneInput, SectionInput};
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
        video_enabled: false,
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

/// Generated SIP passwords follow the device's format. A format change
/// never changes a stored password; only a reset generates a new one.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn sip_password_formats(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let e = extensions::create(&pool, T, &ext("800"), &emergency())
        .await
        .unwrap();
    let base = DeviceInput {
        sip_password_format: None,
        name: "Desk".into(),
        kind: DeviceKind::Desk,
        sip_username: None,
        phone_id: None,
        account_index: None,
        phone_label: String::new(),
        phone_display_name: String::new(),
        codec_profile: None,
        enabled: true,
    };
    let stored = |id: Uuid| {
        let pool = pool.clone();
        let sb = sb.clone();
        async move {
            let enc: String =
                sqlx::query_scalar("SELECT sip_password_enc FROM devices WHERE id = $1")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            (enc.clone(), sb.decrypt(&enc).unwrap())
        }
    };

    // Default: unchanged standard format with 20 characters.
    let (desk, pw) = extensions::create_device(&pool, T, &sb, &e, &base)
        .await
        .unwrap();
    assert_eq!(desk.sip_password_format, SipPasswordFormat::Standard);
    assert_eq!(pw.len(), 20);
    assert_eq!(stored(desk.id).await.1, pw);

    // Compact format: 15 letters and digits, stored encrypted.
    let compact = DeviceInput {
        sip_password_format: Some(SipPasswordFormat::Compact15),
        kind: DeviceKind::Door,
        sip_username: Some("800vth".into()),
        ..base.clone()
    };
    let (vth, pw) = extensions::create_device(&pool, T, &sb, &e, &compact)
        .await
        .unwrap();
    assert_eq!(vth.sip_password_format, SipPasswordFormat::Compact15);
    assert_eq!(pw.len(), 15);
    assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
    let (enc, plain) = stored(vth.id).await;
    assert_eq!(plain, pw);
    assert!(!enc.contains(&pw));
    let auth = extensions::device_auth(&pool, "800vth")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sb.decrypt(&auth.sip_password_enc).unwrap(), pw);

    // A reset keeps the stored format.
    let (pw2, format) = extensions::reset_device_password(&pool, T, &sb, vth.id, None)
        .await
        .unwrap();
    assert_eq!(format, SipPasswordFormat::Compact15);
    assert_eq!(pw2.len(), 15);
    assert_ne!(pw2, pw);
    assert_eq!(stored(vth.id).await.1, pw2);

    // An update changes the stored format, not the password.
    let (enc_before, _) = stored(desk.id).await;
    let updated = extensions::update_device(
        &pool,
        T,
        desk.id,
        &DeviceInput {
            sip_password_format: Some(SipPasswordFormat::Compact15),
            ..base.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.sip_password_format, SipPasswordFormat::Compact15);
    assert_eq!(stored(desk.id).await.0, enc_before);
    // Omitted on update: the stored format stays.
    let kept = extensions::update_device(&pool, T, desk.id, &base)
        .await
        .unwrap();
    assert_eq!(kept.sip_password_format, SipPasswordFormat::Compact15);
    let (pw3, _) = extensions::reset_device_password(&pool, T, &sb, desk.id, None)
        .await
        .unwrap();
    assert_eq!(pw3.len(), 15);

    // Changing the kind must not carry a compact format into a browser.
    for format in [None, Some(SipPasswordFormat::Compact15)] {
        assert!(matches!(
            extensions::update_device(
                &pool,
                T,
                desk.id,
                &DeviceInput {
                    kind: DeviceKind::Browser,
                    sip_password_format: format,
                    ..base.clone()
                }
            )
            .await,
            Err(CoreError::Validation(_))
        ));
    }
    let unchanged = extensions::get_device(&pool, T, desk.id).await.unwrap();
    assert_eq!(unchanged.kind, DeviceKind::Desk);
    assert_eq!(unchanged.sip_password_format, SipPasswordFormat::Compact15);
    assert_eq!(stored(desk.id).await.1, pw3);

    // A reset with a format stores the new format.
    let (pw4, format) = extensions::reset_device_password(
        &pool,
        T,
        &sb,
        desk.id,
        Some(SipPasswordFormat::Standard),
    )
    .await
    .unwrap();
    assert_eq!((pw4.len(), format), (20, SipPasswordFormat::Standard));
    assert_eq!(
        extensions::get_device(&pool, T, desk.id)
            .await
            .unwrap()
            .sip_password_format,
        SipPasswordFormat::Standard
    );

    // Browser devices keep the standard format.
    let browser = DeviceInput {
        kind: DeviceKind::Browser,
        sip_password_format: Some(SipPasswordFormat::Compact15),
        ..base.clone()
    };
    assert!(matches!(
        extensions::create_device(&pool, T, &sb, &e, &browser).await,
        Err(CoreError::Validation(_))
    ));
    let (web, _) = extensions::create_device(
        &pool,
        T,
        &sb,
        &e,
        &DeviceInput {
            sip_password_format: None,
            ..browser.clone()
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        extensions::reset_device_password(
            &pool,
            T,
            &sb,
            web.id,
            Some(SipPasswordFormat::Compact15)
        )
        .await,
        Err(CoreError::Validation(_))
    ));
    assert!(matches!(
        extensions::reset_device_password(&pool, T, &sb, Uuid::new_v4(), None).await,
        Err(CoreError::NotFound)
    ));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn extensions_and_devices(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let e = extensions::create(&pool, T, &ext("200"), &emergency())
        .await
        .unwrap();
    assert!(matches!(
        extensions::create(&pool, T, &ext("200"), &emergency()).await,
        Err(CoreError::Conflict(_))
    ));

    // New numbers start at 100 (*1-*99 are system codes); a 2-digit number
    // of an older installation is kept on update but cannot be newly set.
    assert!(matches!(
        extensions::create(&pool, T, &ext("20"), &emergency()).await,
        Err(CoreError::Validation(_))
    ));
    let legacy = extensions::create(&pool, T, &ext("300"), &emergency())
        .await
        .unwrap();
    sqlx::query("UPDATE extensions SET number = '30' WHERE id = $1")
        .bind(legacy.id)
        .execute(&pool)
        .await
        .unwrap();
    let mut renamed = ext("30");
    renamed.display_name = "Legacy".into();
    let kept = extensions::update(&pool, T, legacy.id, &renamed, &emergency())
        .await
        .unwrap();
    assert_eq!(
        (kept.number.as_str(), kept.display_name.as_str()),
        ("30", "Legacy")
    );
    assert!(matches!(
        extensions::update(&pool, T, legacy.id, &ext("31"), &emergency()).await,
        Err(CoreError::Validation(_))
    ));
    extensions::delete(&pool, T, legacy.id).await.unwrap();

    let input = DeviceInput {
        sip_password_format: None,
        name: "Desk".into(),
        kind: DeviceKind::Desk,
        sip_username: None,
        phone_id: None,
        account_index: None,
        phone_label: String::new(),
        phone_display_name: String::new(),
        codec_profile: None,
        enabled: true,
    };
    let (dev, password) = extensions::create_device(&pool, T, &sb, &e, &input)
        .await
        .unwrap();
    assert_eq!(dev.sip_username, "200-1");
    assert_eq!(sb.decrypt(&dev.sip_password_enc).unwrap(), password);
    let (dev2, _) = extensions::create_device(&pool, T, &sb, &e, &input)
        .await
        .unwrap();
    assert_eq!(dev2.sip_username, "200-2");

    // SIP usernames: extension number plus a free suffix. Door stations get
    // a name without '-' (Dahua VTO does not accept it).
    let named = |kind, name: Option<&str>| DeviceInput {
        kind,
        sip_username: name.map(Into::into),
        ..input.clone()
    };
    let (vto, _) = extensions::create_device(&pool, T, &sb, &e, &named(DeviceKind::Door, None))
        .await
        .unwrap();
    assert_eq!(vto.sip_username, "200vto");
    let (vto2, _) = extensions::create_device(&pool, T, &sb, &e, &named(DeviceKind::Door, None))
        .await
        .unwrap();
    assert_eq!(vto2.sip_username, "200vto2");
    let (vth, _) = extensions::create_device(
        &pool,
        T,
        &sb,
        &e,
        &named(DeviceKind::Other, Some(" 200vth ")),
    )
    .await
    .unwrap();
    assert_eq!(vth.sip_username, "200vth");
    for bad in ["201vth", "2001", "200 x", "200-ä", "web-200"] {
        assert!(
            matches!(
                extensions::create_device(&pool, T, &sb, &e, &named(DeviceKind::Desk, Some(bad)))
                    .await,
                Err(CoreError::Validation(_))
            ),
            "{bad}"
        );
    }
    // Unique without regard to case.
    assert!(matches!(
        extensions::create_device(&pool, T, &sb, &e, &named(DeviceKind::Desk, Some("200VTH")))
            .await,
        Err(CoreError::Conflict(_))
    ));
    // Rename; an empty name keeps the current one.
    let renamed =
        extensions::update_device(&pool, T, vto2.id, &named(DeviceKind::Door, Some("200gate")))
            .await
            .unwrap();
    assert_eq!(renamed.sip_username, "200gate");
    let kept = extensions::update_device(&pool, T, vto2.id, &named(DeviceKind::Door, Some("")))
        .await
        .unwrap();
    assert_eq!(kept.sip_username, "200gate");
    assert!(
        extensions::update_device(&pool, T, vto2.id, &named(DeviceKind::Door, Some("200vto")))
            .await
            .is_err()
    );
    assert!(
        extensions::device_auth(&pool, "200gate")
            .await
            .unwrap()
            .is_some()
    );
    // The lowest free number is used after a device is deleted.
    for d in [vto.id, vto2.id, vth.id] {
        extensions::delete_device(&pool, T, d).await.unwrap();
    }
    extensions::delete_device(&pool, T, dev.id).await.unwrap();
    let (again, _) = extensions::create_device(&pool, T, &sb, &e, &input)
        .await
        .unwrap();
    assert_eq!(again.sip_username, "200-1");
    extensions::delete_device(&pool, T, again.id).await.unwrap();
    let (dev, _) = extensions::create_device(&pool, T, &sb, &e, &input)
        .await
        .unwrap();
    assert_eq!(dev.sip_username, "200-1");

    let auth = extensions::device_auth(&pool, "200-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(auth.extension_number, "200");
    assert_eq!(
        extensions::ring_targets(&pool, e.id)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.sip_username)
            .collect::<Vec<_>>(),
        vec!["200-1", "200-2"]
    );
    // Codec profile: automatic from the kind until one is set.
    assert_eq!(auth.codec_profile, None);
    assert_eq!(auth.effective_codec_profile(), CodecProfile::Standard);
    let handset = DeviceInput {
        kind: DeviceKind::Wifi,
        ..input.clone()
    };
    extensions::update_device(&pool, T, dev2.id, &handset)
        .await
        .unwrap();
    let auth = extensions::device_auth(&pool, "200-2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(auth.kind, DeviceKind::Wifi);
    assert_eq!(auth.effective_codec_profile(), CodecProfile::Hd);
    let robust = DeviceInput {
        codec_profile: Some(CodecProfile::Robust),
        ..handset
    };
    let updated = extensions::update_device(&pool, T, dev2.id, &robust)
        .await
        .unwrap();
    assert_eq!(updated.codec_profile, Some(CodecProfile::Robust));
    let auth = extensions::device_auth(&pool, "200-2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(auth.effective_codec_profile(), CodecProfile::Robust);

    let (new_pw, format) = extensions::reset_device_password(&pool, T, &sb, dev.id, None)
        .await
        .unwrap();
    assert_eq!(format, SipPasswordFormat::Standard);
    assert_eq!(new_pw.len(), 20);
    let auth = extensions::device_auth(&pool, "200-1")
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
            ..ext("200")
        },
        &emergency(),
    )
    .await
    .unwrap();
    assert!(
        extensions::device_auth(&pool, "200-1")
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
        video_enabled: false,
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
            video_enabled: false,
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
            destination_type: trunks::NumberDestination::None,
            destination_id: None,
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
                enabled: true,
                destination_type: trunks::NumberDestination::None,
                destination_id: None,
            }
        )
        .await
        .is_err()
    );

    let e = extensions::create(&pool, T, &ext("210"), &emergency())
        .await
        .unwrap();
    let num_input = NumberInput {
        trunk_id: trunk.id,
        account_id: Some(acc.id),
        e164: "+49891234567".into(),
        label: "Main".into(),
        destination_type: NumberDestination::Extension,
        destination_id: Some(e.id),
        extra_extensions: vec![],
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
            destination_type: trunks::NumberDestination::None,
            destination_id: None,
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
            video_enabled: false,
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
    s.hold_music = "ponce-preludio-in-e-major".into();
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
        caller_number: "200".into(),
        caller_name: "A".into(),
        destination: "210".into(),
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
            ringtone_id: None,
            wallpaper_id: None,
            phonebook_sections: vec![],
            settings: serde_json::json!({}),
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
                line_keys: json!([]),
                ringtone_id: None,
                wallpaper_id: None,
                phonebook_sections: vec![],
                settings: serde_json::json!({}),
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
                line_keys: json!([]),
                ringtone_id: None,
                wallpaper_id: None,
                phonebook_sections: vec![],
                settings: serde_json::json!({}),
            }
        )
        .await
        .is_err()
    );

    let e20 = extensions::create(&pool, T, &ext("200"), &emergency())
        .await
        .unwrap();
    let e21 = extensions::create(&pool, T, &ext("210"), &emergency())
        .await
        .unwrap();
    let on_phone = |name: &str| DeviceInput {
        sip_password_format: None,
        name: name.into(),
        kind: DeviceKind::Desk,
        sip_username: None,
        phone_id: Some(phone.id),
        account_index: None,
        phone_label: String::new(),
        phone_display_name: String::new(),
        codec_profile: None,
        enabled: true,
    };
    let (d1, _) = extensions::create_device(&pool, T, &sb, &e20, &on_phone("A"))
        .await
        .unwrap();
    let labelled = DeviceInput {
        phone_label: " Labor ".into(),
        phone_display_name: "Labor Anna".into(),
        ..on_phone("B")
    };
    let (d2, _) = extensions::create_device(&pool, T, &sb, &e21, &labelled)
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
        ["200", "210"]
    );
    assert_eq!(
        (
            accounts[1].phone_label.as_str(),
            accounts[1].phone_display_name.as_str()
        ),
        ("Labor", "Labor Anna"),
        "label and display name are trimmed and stored"
    );
    let too_long = DeviceInput {
        phone_label: "x".repeat(33),
        ..on_phone("D")
    };
    assert!(
        extensions::create_device(&pool, T, &sb, &e21, &too_long)
            .await
            .is_err(),
        "label longer than 32 characters"
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
            section_id: None,
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
                phone_other: String::new(),
                section_id: None,
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
                phone_other: String::new(),
                section_id: None,
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

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn phone_media_and_phonebook_sections(pool: PgPool) {
    let input = |sections: Vec<Uuid>, ringtone: Option<Uuid>, wallpaper: Option<Uuid>| PhoneInput {
        mac: "805ec0000001".into(),
        model: "t54w".into(),
        name: "Küche".into(),
        line_keys: json!([]),
        ringtone_id: ringtone,
        wallpaper_id: wallpaper,
        phonebook_sections: sections,
        settings: serde_json::json!({}),
    };
    let ring = phones::create_media(
        &pool,
        T,
        Uuid::new_v4(),
        MediaKind::Ringtone,
        " Gong ",
        "talkops-a.wav",
        1200,
    )
    .await
    .unwrap();
    assert_eq!(ring.name, "Gong");
    let wall = phones::create_media(
        &pool,
        T,
        Uuid::new_v4(),
        MediaKind::Wallpaper,
        "Foto",
        "talkops-b.jpg",
        9000,
    )
    .await
    .unwrap();
    assert!(
        phones::create_media(
            &pool,
            T,
            Uuid::new_v4(),
            MediaKind::Wallpaper,
            " ",
            "x.jpg",
            1
        )
        .await
        .is_err(),
        "name required"
    );
    assert_eq!(phones::list_media(&pool, T).await.unwrap().len(), 2);

    let family = phones::create_section(
        &pool,
        T,
        &SectionInput {
            name: "Familie".into(),
        },
    )
    .await
    .unwrap();
    let office = phones::create_section(
        &pool,
        T,
        &SectionInput {
            name: "Büro".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        phones::create_section(
            &pool,
            T,
            &SectionInput {
                name: "Familie".into()
            }
        )
        .await
        .is_err(),
        "names are unique"
    );

    // Swapped kinds, unknown sections and too many sections are rejected.
    assert!(
        phones::create(&pool, T, &input(vec![], Some(wall.id), None))
            .await
            .is_err()
    );
    assert!(
        phones::create(&pool, T, &input(vec![Uuid::new_v4()], None, None))
            .await
            .is_err()
    );
    let extra: Vec<Uuid> = create_sections(&pool, 3).await;
    let mut four = vec![family.id];
    four.extend(extra);
    assert!(
        phones::create(&pool, T, &input(four, None, None))
            .await
            .is_err()
    );

    let phone = phones::create(
        &pool,
        T,
        &input(
            vec![family.id, office.id, family.id],
            Some(ring.id),
            Some(wall.id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        phone.phonebook_sections,
        [family.id, office.id],
        "duplicates dropped"
    );
    assert_eq!(
        (phone.ringtone_id, phone.wallpaper_id),
        (Some(ring.id), Some(wall.id))
    );

    let contact = |name: &str, section: Option<Uuid>| ContactInput {
        name: name.into(),
        company: String::new(),
        phone_work: "0301234".into(),
        phone_mobile: String::new(),
        phone_other: String::new(),
        section_id: section,
    };
    phones::create_contact(&pool, T, &contact("Pizza", None))
        .await
        .unwrap();
    phones::create_contact(&pool, T, &contact("Oma", Some(family.id)))
        .await
        .unwrap();
    assert!(
        phones::create_contact(&pool, T, &contact("X", Some(Uuid::new_v4())))
            .await
            .is_err()
    );
    let names = |c: Vec<phones::Contact>| c.into_iter().map(|c| c.name).collect::<Vec<_>>();
    assert_eq!(
        names(phones::list_contacts_in(&pool, T, None).await.unwrap()),
        ["Pizza"]
    );
    assert_eq!(
        names(
            phones::list_contacts_in(&pool, T, Some(family.id))
                .await
                .unwrap()
        ),
        ["Oma"]
    );
    assert_eq!(phones::list_contacts(&pool, T).await.unwrap().len(), 2);

    // Deleting a section removes its contacts and its place on phones;
    // deleting media clears it from phones.
    phones::delete_section(&pool, T, family.id).await.unwrap();
    phones::delete_media(&pool, T, ring.id).await.unwrap();
    let phone = phones::get(&pool, T, phone.id).await.unwrap();
    assert_eq!(phone.phonebook_sections, [office.id]);
    assert_eq!(phone.ringtone_id, None);
    assert_eq!(
        names(phones::list_contacts(&pool, T).await.unwrap()),
        ["Pizza"]
    );
}

/// Creates `n` phone book sections named `S1` … `Sn`.
async fn create_sections(pool: &PgPool, n: usize) -> Vec<Uuid> {
    let mut ids = Vec::new();
    for i in 1..=n {
        let s = phones::create_section(
            pool,
            T,
            &SectionInput {
                name: format!("S{i}"),
            },
        )
        .await
        .unwrap();
        ids.push(s.id);
    }
    ids
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
        ring_timeout_secs: None,
        speech_speed: None,
        greeting_clip_id: None,
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn greeting_speed_changes_queue_audio_and_ignore_old_results(pool: PgPool) {
    let e = extensions::create(&pool, T, &ext("203"), &[])
        .await
        .unwrap();
    let mut input = vm_input();
    input.greeting = "tts".into();
    input.greeting_text = "Hallo <speed:0.7>Anna</speed>.".into();
    let normal = voicemail::update_box(&pool, T, e.id, &input, "de")
        .await
        .unwrap();
    assert_eq!(normal.speech_speed, 1.0);
    input.speech_speed = Some(0.8);
    voicemail::update_box(&pool, T, e.id, &input, "de")
        .await
        .unwrap();
    voicemail::set_greeting_status(&pool, e.id, &input.greeting_text, true)
        .await
        .unwrap();
    assert_eq!(
        voicemail::get_box(&pool, T, e.id)
            .await
            .unwrap()
            .greeting_status,
        "pending"
    );
    voicemail::set_greeting_status_at_speed(&pool, e.id, &input.greeting_text, 0.8, true)
        .await
        .unwrap();
    assert_eq!(
        voicemail::get_box(&pool, T, e.id)
            .await
            .unwrap()
            .greeting_status,
        "ready"
    );
    let payloads: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM jobs WHERE kind = 'tts.greeting' ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(payloads.len(), 2);
    assert_eq!(payloads[1]["speech_speed"], 0.8);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn voicemail_boxes_messages_and_smtp(pool: PgPool) {
    let e = extensions::create(&pool, T, &ext("200"), &[])
        .await
        .unwrap();

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
            ring_timeout_secs: None,
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
    assert_eq!(
        job.payload["file"],
        voicemail::greeting_tts_file(T, e.id, "en", &tts.greeting_text, 1.0).unwrap()
    );
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
            caller_number: "210".into(),
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
    assert_eq!(info.extension_number, "200");
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
            caller_number: "220".into(),
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
        Default::default(),
    )
    .await
    .unwrap();
    let third = voicemail::get_message(&pool, T, third.id).await.unwrap();
    assert_eq!(third.transcript_status, "done");
    let source = recordings::Source::Voicemail(third.id);
    let t = recordings::transcript_of(&pool, T, source).await.unwrap();
    assert!(t.is_final);
    // A first pass with a second one to come, then the second fails.
    let meta = recordings::TranscriptMeta {
        engine: "whisper:base",
        is_final: false,
    };
    let seg = recordings::Segment {
        start: 0.0,
        end: 2.0,
        speaker: String::new(),
        text: " Ruf mich zurück.".into(),
    };
    recordings::save_transcript(&pool, T, source, "de", std::slice::from_ref(&seg), meta)
        .await
        .unwrap();
    let t = recordings::transcript_of(&pool, T, source).await.unwrap();
    assert_eq!((t.engine.as_str(), t.is_final), ("whisper:base", false));
    recordings::finalize_transcript(&pool, source)
        .await
        .unwrap();
    let t = recordings::transcript_of(&pool, T, source).await.unwrap();
    assert!(t.is_final);
    // Engines per purpose: defaults without stored profiles.
    use talkops_core::settings::TranscriptionQuality;
    use talkops_core::transcription_profiles::{self as profiles, Engine, Purpose};
    assert_eq!(
        profiles::engine(&pool, T, Purpose::VoicemailRefine)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        profiles::engine(&pool, T, Purpose::Live).await.unwrap(),
        Some(Engine::Local(TranscriptionQuality::Base))
    );
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
    assert_eq!(hits[0].destination.as_deref(), Some("200"));
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
        vendor: None,
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
        unlock_code_1: Some("*1".into()),
        unlock_code_2: None,
        dtmf_allowed_numbers: vec![],
        enabled: true,
    };
    let s = doors::create(&pool, T, &secrets, &input).await.unwrap();
    assert!(s.has_unlock_code_1 && !s.has_unlock_code_2);
    // Omitted vendor: Dahua (ADR 0026).
    assert_eq!(s.vendor, doors::DoorVendor::Dahua);
    let conn = doors::connection(&pool, &secrets, s.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(conn.vendor, doors::DoorVendor::Dahua);
    // The codes are stored encrypted, never in plain text.
    let raw: Option<String> =
        sqlx::query_scalar("SELECT unlock_code_1_enc FROM door_stations WHERE id = $1")
            .bind(s.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(raw.is_some_and(|r| !r.contains("*1")));
    assert_eq!(
        doors::unlock_codes(&pool, &secrets, T, s.id).await.unwrap(),
        [Some("*1".into()), None]
    );
    // Invalid codes; codes that end with each other (also a kept code).
    for (c1, c2) in [
        (Some("1"), None),
        (Some("12a"), None),
        (Some("123456789"), None),
        (Some("#91"), Some("91")),
        (None, Some("#*1")),
    ] {
        let bad = DoorStationInput {
            unlock_code_1: c1.map(Into::into),
            unlock_code_2: c2.map(Into::into),
            ..input.clone()
        };
        assert!(
            matches!(
                doors::update(&pool, T, &secrets, s.id, &bad).await,
                Err(CoreError::Validation(_))
            ),
            "{c1:?} {c2:?}"
        );
    }
    // Door 2 needs a second lock.
    assert!(
        doors::update(
            &pool,
            T,
            &secrets,
            s.id,
            &DoorStationInput {
                doors: 1,
                unlock_code_2: Some("#2".into()),
                ..input.clone()
            },
        )
        .await
        .is_err()
    );
    let codes = doors::update(
        &pool,
        T,
        &secrets,
        s.id,
        &DoorStationInput {
            unlock_code_1: None,
            unlock_code_2: Some("#2".into()),
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert!(codes.has_unlock_code_1 && codes.has_unlock_code_2);
    assert_eq!(
        doors::unlock_codes(&pool, &secrets, T, s.id).await.unwrap(),
        [Some("*1".into()), Some("#2".into())]
    );
    // "" removes a code; one door removes the code of door 2.
    let one = doors::update(
        &pool,
        T,
        &secrets,
        s.id,
        &DoorStationInput {
            doors: 1,
            unlock_code_1: Some(String::new()),
            unlock_code_2: None,
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert!(!one.has_unlock_code_1 && !one.has_unlock_code_2);
    doors::update(&pool, T, &secrets, s.id, &input)
        .await
        .unwrap();
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
