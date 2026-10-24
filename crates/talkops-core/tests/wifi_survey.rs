//! Wi-Fi survey and UniFi connection queries (need Postgres via DATABASE_URL).

use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::error::CoreError;
use talkops_core::extensions::{self, DeviceInput, DeviceKind, ExtensionInput};
use talkops_core::tenant::TenantId;
use talkops_core::unifi::{self, UnifiInput};
use talkops_core::wifi_survey::{self as ws, MediaSample, NewSurvey, RadioSample, Thresholds};
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

async fn device(pool: &PgPool, number: &str, kind: DeviceKind) -> Uuid {
    let e = extensions::create(
        pool,
        T,
        &ExtensionInput {
            number: number.into(),
            display_name: "Walker".into(),
            user_id: None,
            outbound_number_id: None,
            hide_caller_id: false,
            ring_timeout_secs: 30,
            enabled: true,
            dnd: false,
            forward_all: None,
            record_calls: "inherit".into(),
            video_enabled: false,
        },
        &[],
    )
    .await
    .unwrap();
    let sb = SecretBox::from_hex(KEY).unwrap();
    let (d, _) = extensions::create_device(
        pool,
        T,
        &sb,
        &e,
        &DeviceInput {
            sip_password_format: None,
            name: "Handset".into(),
            kind,
            sip_username: None,
            phone_id: None,
            account_index: None,
            phone_label: String::new(),
            phone_display_name: String::new(),
            codec_profile: None,
            video: None,
            enabled: true,
        },
    )
    .await
    .unwrap();
    d.id
}

fn sample(offset: i32, received: i32) -> MediaSample {
    MediaSample {
        offset_ms: offset,
        interval_ms: 1000,
        received,
        expected: 50,
        flaws: 50 - received,
        mos: Some(4.3),
        jitter_max_ms: Some(2.0),
        rtcp_fraction_lost: None,
        rtcp_jitter_ms: Some(3.0),
    }
}

fn radio(offset: i32, ap: &str, rssi: i32) -> RadioSample {
    RadioSample {
        offset_ms: offset,
        ap_mac: Some(ap.into()),
        ap_name: Some(format!("AP {ap}")),
        bssid: None,
        radio: Some("na".into()),
        channel: Some(36),
        rssi: Some(rssi),
        tx_rate_kbps: Some(144_000),
        rx_rate_kbps: Some(144_000),
        error: None,
    }
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn survey_lifecycle_analysis_and_retention(pool: PgPool) {
    let wifi = device(&pool, "301", DeviceKind::Wifi).await;
    let desk = device(&pool, "302", DeviceKind::Desk).await;
    assert!(matches!(
        ws::target(&pool, T, desk).await,
        Err(CoreError::Validation(_))
    ));
    let target = ws::target(&pool, T, wifi).await.unwrap();
    assert_eq!(target.mac, None);
    let input = NewSurvey {
        device_id: wifi,
        mode: "originate".into(),
        label: " Before ".into(),
    };
    let s = ws::create(&pool, T, &input, &target, None, json!({}))
        .await
        .unwrap();
    assert_eq!(s.state, "calling");
    assert_eq!(s.label, "Before");
    // One active survey for each device.
    assert!(matches!(
        ws::create(&pool, T, &input, &target, None, json!({})).await,
        Err(CoreError::Conflict(_))
    ));
    let call = Uuid::new_v4();
    ws::set_call(&pool, T, s.id, call).await.unwrap();
    assert_eq!(ws::by_call(&pool, call).await.unwrap().unwrap().1.id, s.id);
    let answered = Utc::now() - Duration::seconds(20);
    assert!(
        ws::mark_answered(&pool, T, s.id, answered, Some("wifi-surveys/x.wav"))
            .await
            .unwrap()
    );
    let s = ws::get(&pool, T, s.id).await.unwrap();
    assert_eq!(s.state, "running");
    for sec in 1..=20 {
        let lost = if sec == 10 { 5 } else { 0 };
        ws::add_media(&pool, T, s.id, Utc::now(), &sample(sec * 1000, 50 - lost))
            .await
            .unwrap();
        let ap = if sec < 10 { "aa" } else { "bb" };
        ws::add_radio(&pool, T, s.id, Utc::now(), &radio(sec * 1000, ap, -60))
            .await
            .unwrap();
    }
    let m = ws::add_marker(
        &pool,
        T,
        &s,
        "disturbance",
        "dtmf",
        "",
        answered + Duration::seconds(11),
    )
    .await
    .unwrap();
    assert_eq!(m.offset_ms, 11_000);
    ws::set_note(&pool, T, s.id, m.id, "Stairs").await.unwrap();
    assert!(
        ws::add_marker(&pool, T, &s, "other", "ui", "", Utc::now())
            .await
            .is_err()
    );
    ws::add_error_period(
        &pool,
        T,
        &s,
        answered + Duration::seconds(9),
        answered + Duration::seconds(12),
        5,
    )
    .await
    .unwrap();
    let tl = ws::timeline(&pool, T, s.id, 15_000).await.unwrap();
    assert_eq!(tl.media.len(), 5);
    assert!(tl.events.is_empty());

    // Deletion of an active survey is refused.
    assert!(matches!(
        ws::delete(&pool, T, s.id).await,
        Err(CoreError::Conflict(_))
    ));
    assert!(
        ws::finish(&pool, T, s.id, "NORMAL_CLEARING", Some(20))
            .await
            .unwrap()
    );
    assert!(
        !ws::finish(&pool, T, s.id, "NORMAL_CLEARING", None)
            .await
            .unwrap()
    );
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = $1")
        .bind(ws::JOB_ANALYSE)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);

    let result = ws::analyse_and_store(&pool, T, s.id).await.unwrap();
    assert_eq!(result.roams.len(), 1);
    assert_eq!(result.roams[0].gap_ms, 100);
    assert_eq!(result.roams[0].rating, ws::Rating::Audible);
    assert_eq!(result.roams[0].markers, 1);
    let stored = ws::roams(&pool, T, s.id).await.unwrap();
    assert_eq!(stored, result.roams);
    let s = ws::get(&pool, T, s.id).await.unwrap();
    assert_eq!(s.state, "analysed");
    assert_eq!(s.summary.unwrap()["roams"], 1);

    // New thresholds: the survey can be analysed again.
    ws::save_settings(
        &pool,
        T,
        &Thresholds {
            clean_ms: 120,
            ..Thresholds::default()
        },
    )
    .await
    .unwrap();
    ws::reanalyse(&pool, T, s.id).await.unwrap();
    let again = ws::analyse_and_store(&pool, T, s.id).await.unwrap();
    assert_eq!(again.roams[0].rating, ws::Rating::Clean);

    // Other tenants see nothing.
    let other: Uuid = sqlx::query_scalar("INSERT INTO tenants(name) VALUES('Other') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(ws::get(&pool, TenantId(other), s.id).await.is_err());
    assert!(
        ws::list(&pool, TenantId(other), None)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(ws::list(&pool, T, None).await.unwrap().len(), 1);

    // Retention removes the survey with all values.
    sqlx::query("UPDATE wifi_surveys SET expires_at = now() - interval '1 day'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        ws::purge_expired(&pool).await.unwrap(),
        vec!["wifi-surveys/x.wav".to_owned()]
    );
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM wifi_survey_media_samples)
              + (SELECT count(*) FROM wifi_survey_radio_samples)
              + (SELECT count(*) FROM wifi_survey_events)
              + (SELECT count(*) FROM wifi_survey_roams)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn self_dial_arming_and_limits(pool: PgPool) {
    let wifi = device(&pool, "301", DeviceKind::Wifi).await;
    let target = ws::target(&pool, T, wifi).await.unwrap();
    let s = ws::create(
        &pool,
        T,
        &NewSurvey {
            device_id: wifi,
            mode: "self_dial".into(),
            label: String::new(),
        },
        &target,
        None,
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(s.state, "armed");
    assert!(s.armed_until.is_some());
    // Another device cannot claim it.
    let call = Uuid::new_v4();
    assert!(
        ws::claim_armed(&pool, T, Uuid::new_v4(), call)
            .await
            .unwrap()
            .is_none()
    );
    let claimed = ws::claim_armed(&pool, T, wifi, call)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.state, "calling");
    assert_eq!(claimed.call_uuid, Some(call));
    assert!(
        ws::claim_armed(&pool, T, wifi, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        ws::fail(&pool, T, s.id, "cancelled", "stopped")
            .await
            .unwrap()
    );

    // An expired arming is cancelled.
    let s2 = ws::create(
        &pool,
        T,
        &NewSurvey {
            device_id: wifi,
            mode: "self_dial".into(),
            label: String::new(),
        },
        &target,
        None,
        json!({}),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE wifi_surveys SET armed_until = now() - interval '1 minute' WHERE id = $1")
        .bind(s2.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(ws::expire_armed(&pool).await.unwrap(), 1);
    assert_eq!(ws::get(&pool, T, s2.id).await.unwrap().state, "cancelled");

    // Parallel limit.
    ws::save_settings(
        &pool,
        T,
        &Thresholds {
            max_parallel: 1,
            ..Thresholds::default()
        },
    )
    .await
    .unwrap();
    let other = device(&pool, "302", DeviceKind::Wifi).await;
    ws::create(
        &pool,
        T,
        &NewSurvey {
            device_id: wifi,
            mode: "originate".into(),
            label: String::new(),
        },
        &target,
        None,
        json!({}),
    )
    .await
    .unwrap();
    let t2 = ws::target(&pool, T, other).await.unwrap();
    assert!(matches!(
        ws::create(
            &pool,
            T,
            &NewSurvey {
                device_id: other,
                mode: "originate".into(),
                label: String::new(),
            },
            &t2,
            None,
            json!({}),
        )
        .await,
        Err(CoreError::Conflict(_))
    ));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn unifi_key_is_encrypted_and_kept(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let input = UnifiInput {
        url: "https://192.168.1.1/".into(),
        site: "default".into(),
        cert_sha256: String::new(),
        api_key: None,
    };
    assert!(unifi::save(&pool, T, &sb, &input).await.is_err());
    let saved = unifi::save(
        &pool,
        T,
        &sb,
        &UnifiInput {
            api_key: Some("key-123".into()),
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.url, "https://192.168.1.1");
    let enc: String = sqlx::query_scalar("SELECT api_key_enc FROM unifi_connections")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!enc.contains("key-123"));
    // Without a new key the stored key stays.
    unifi::save(&pool, T, &sb, &input).await.unwrap();
    assert_eq!(unifi::api_key(&pool, T, &sb).await.unwrap(), "key-123");
    unifi::record_check(&pool, T, Ok(("9.4.19", &json!({"classic": true}))))
        .await
        .unwrap();
    let c = unifi::get(&pool, T).await.unwrap().unwrap();
    assert_eq!(c.version.as_deref(), Some("9.4.19"));
    assert_eq!(c.capabilities["classic"], true);
    // The phone network data is off by default and stays on after a save.
    assert!(!c.phone_network);
    assert!(
        unifi::set_phone_network(&pool, T, true)
            .await
            .unwrap()
            .phone_network
    );
    assert!(
        unifi::save(&pool, T, &sb, &input)
            .await
            .unwrap()
            .phone_network
    );
    unifi::delete(&pool, T).await.unwrap();
    assert!(matches!(
        unifi::set_phone_network(&pool, T, true).await,
        Err(CoreError::NotFound)
    ));
    assert!(unifi::get(&pool, T).await.unwrap().is_none());
}
