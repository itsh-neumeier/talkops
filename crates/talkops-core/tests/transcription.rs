//! Cloud connections, transcription profiles and usage (ADR 0024).

use sqlx::PgPool;
use talkops_core::crypto::SecretBox;
use talkops_core::error::CoreError;
use talkops_core::settings::TranscriptionQuality;
use talkops_core::tenant::TenantId;
use talkops_core::transcription_api::{self as providers, ProviderInput};
use talkops_core::transcription_profiles::{self as profiles, Engine, Profile, Purpose};
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
/// The migration that adds profiles.
const PROFILES_MIGRATION: i64 = 20261028000001;

fn input(name: &str, kind: &str, url: &str, key: Option<&str>) -> ProviderInput {
    ProviderInput {
        name: name.into(),
        kind: kind.into(),
        url: url.into(),
        key: key.map(Into::into),
    }
}

fn cloud(purpose: &str, provider: Uuid, model: &str) -> Profile {
    Profile {
        purpose: purpose.into(),
        engine: "cloud".into(),
        local_model: "fast".into(),
        provider_id: Some(provider),
        model: model.into(),
        realtime: false,
        coral_stable_preview: false,
        price_per_minute: None,
        list_price: None,
        provider_kind: None,
    }
}

async fn other_tenant(pool: &PgPool) -> TenantId {
    let id: Uuid = sqlx::query_scalar("INSERT INTO tenants (name) VALUES ('Other') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap();
    TenantId(id)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn coral_stable_preview_is_persisted_and_tenant_checked(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let coral = providers::create(&pool, T, &sb, &input("Coral", "coral", "", None))
        .await
        .unwrap();
    let mut p = cloud("live", coral.id, "coral-ctc-de");
    p.realtime = true;
    p.coral_stable_preview = true;
    let saved = profiles::update(&pool, T, &[p.clone()]).await.unwrap();
    assert!(
        saved
            .iter()
            .find(|p| p.purpose == "live")
            .unwrap()
            .coral_stable_preview
    );
    assert!(matches!(
        profiles::engine(&pool, T, Purpose::Live).await.unwrap(),
        Some(Engine::Cloud {
            coral_stable_preview: true,
            ..
        })
    ));
    let other = other_tenant(&pool).await;
    assert!(profiles::update(&pool, other, &[p.clone()]).await.is_err());
    let external = providers::create(
        &pool,
        T,
        &sb,
        &input("Other", "compatible", "http://127.0.0.1:10000/v1", None),
    )
    .await
    .unwrap();
    p.provider_id = Some(external.id);
    p.provider_kind = Some("coral".into()); // Client-supplied read-only data is not trusted.
    assert!(profiles::update(&pool, T, &[p.clone()]).await.is_err());
    p.provider_id = Some(coral.id);
    p.realtime = false;
    assert!(profiles::update(&pool, T, &[p]).await.is_err());
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn providers_keep_keys_encrypted(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    // The URL of an OpenAI connection is fixed.
    let openai = providers::create(
        &pool,
        T,
        &sb,
        &input("OpenAI", "openai", "https://evil.example/v1", Some("sk-1")),
    )
    .await
    .unwrap();
    assert_eq!(openai.url, providers::OPENAI_URL);
    assert!(openai.has_key);
    let stored: String = sqlx::query_scalar("SELECT key_enc FROM transcription_providers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!stored.contains("sk-1"));
    let lan = providers::create(
        &pool,
        T,
        &sb,
        &input("LAN", "compatible", "http://10.0.0.5:8000/v1/", None),
    )
    .await
    .unwrap();
    assert_eq!(lan.url, "http://10.0.0.5:8000/v1");
    assert!(!lan.has_key);
    for bad in [
        input("", "openai", "", None),
        input("X", "deepgram", "", None),
        input("X", "compatible", "ftp://x", None),
        input("X", "compatible", "https://x/v1", Some("with space")),
    ] {
        assert!(matches!(
            providers::create(&pool, T, &sb, &bad).await,
            Err(CoreError::Validation(_))
        ));
    }
    assert!(matches!(
        providers::create(&pool, T, &sb, &input("LAN", "openai", "", None)).await,
        Err(CoreError::Conflict(_))
    ));
    // `None` keeps the key, "" removes it.
    let kept = providers::update(
        &pool,
        T,
        &sb,
        openai.id,
        &input("OpenAI", "openai", "", None),
    )
    .await
    .unwrap();
    assert!(kept.has_key);
    let cfg = providers::config(&pool, T, Some(&sb), openai.id, "gpt-transcribe")
        .await
        .unwrap();
    assert_eq!(cfg.key.as_deref(), Some("sk-1"));
    assert_eq!(cfg.label(), "OpenAI/gpt-transcribe");
    let removed = providers::update(
        &pool,
        T,
        &sb,
        openai.id,
        &input("OpenAI", "openai", "", Some("")),
    )
    .await
    .unwrap();
    assert!(!removed.has_key);
    // Other tenants see nothing.
    let other = other_tenant(&pool).await;
    assert!(providers::list(&pool, other).await.unwrap().is_empty());
    assert!(matches!(
        providers::config(&pool, other, Some(&sb), lan.id, "m").await,
        Err(CoreError::NotFound)
    ));
    assert_eq!(providers::list(&pool, T).await.unwrap().len(), 2);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn profiles_select_engines_per_purpose(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let defaults = profiles::list(&pool, T).await.unwrap();
    let purposes: Vec<_> = defaults.iter().map(|p| p.purpose.as_str()).collect();
    assert_eq!(
        purposes,
        [
            "live",
            "recording",
            "recording_refine",
            "voicemail",
            "voicemail_refine"
        ]
    );
    let openai = providers::create(&pool, T, &sb, &input("OpenAI", "openai", "", Some("k")))
        .await
        .unwrap();
    let mut live = cloud("live", openai.id, "gpt-live-transcribe");
    live.realtime = true;
    let mut refine = cloud("recording_refine", openai.id, "gpt-transcribe");
    refine.price_per_minute = Some(0.01);
    let mut voicemail = defaults[3].clone();
    voicemail.local_model = "german".into();
    let saved = profiles::update(&pool, T, &[live, refine, voicemail])
        .await
        .unwrap();
    assert_eq!(saved[0].list_price, Some(0.017));
    assert!(saved[0].realtime);
    assert_eq!(saved[1], defaults[1], "untouched purposes stay");
    assert_eq!(
        profiles::engine(&pool, T, Purpose::Live).await.unwrap(),
        Some(Engine::Cloud {
            provider_id: openai.id,
            model: "gpt-live-transcribe".into(),
            realtime: true,
            coral_stable_preview: false,
            price: Some(0.017),
            coral: false,
            fallback: None,
        })
    );
    assert_eq!(
        profiles::engine(&pool, T, Purpose::RecordingRefine)
            .await
            .unwrap(),
        Some(Engine::Cloud {
            provider_id: openai.id,
            model: "gpt-transcribe".into(),
            realtime: false,
            coral_stable_preview: false,
            price: Some(0.01),
            coral: false,
            fallback: None,
        })
    );
    assert_eq!(
        profiles::engine(&pool, T, Purpose::Voicemail)
            .await
            .unwrap(),
        Some(Engine::Local(TranscriptionQuality::German))
    );
    // A connection in use cannot be deleted.
    assert!(matches!(
        providers::delete(&pool, T, openai.id).await,
        Err(CoreError::Conflict(msg)) if msg.contains("live")
    ));
    // Duplicate purposes and connections of other tenants are rejected.
    let twice = [cloud("live", openai.id, "a"), cloud("live", openai.id, "b")];
    assert!(profiles::update(&pool, T, &twice).await.is_err());
    let other = other_tenant(&pool).await;
    assert!(matches!(
        profiles::update(&pool, other, &[cloud("recording", openai.id, "m")]).await,
        Err(CoreError::Validation(_))
    ));
    // Back to local: the connection is free again.
    let local: Vec<Profile> = defaults
        .iter()
        .filter(|p| p.purpose == "live" || p.purpose == "recording_refine")
        .cloned()
        .collect();
    let saved = profiles::update(&pool, T, &local).await.unwrap();
    assert_eq!(saved[0].provider_id, None);
    providers::delete(&pool, T, openai.id).await.unwrap();
    assert!(matches!(
        providers::delete(&pool, T, openai.id).await,
        Err(CoreError::NotFound)
    ));
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn usage_adds_up_per_month(pool: PgPool) {
    let record = |purpose, engine: &'static str, secs, price| {
        let pool = pool.clone();
        async move {
            profiles::record_usage(&pool, T, purpose, engine, secs, price)
                .await
                .unwrap()
        }
    };
    record(
        Purpose::Live,
        "OpenAI/gpt-live-transcribe",
        60.0,
        Some(0.017),
    )
    .await;
    record(
        Purpose::Live,
        "OpenAI/gpt-live-transcribe",
        120.0,
        Some(0.017),
    )
    .await;
    record(Purpose::Recording, "LAN/whisper", 30.0, None).await;
    record(Purpose::Recording, "LAN/whisper", 0.0, None).await;
    sqlx::query(
        "INSERT INTO transcription_usage (tenant_id, day, purpose, engine, seconds, cost)
         VALUES ($1, current_date - 800, 'live', 'old', 60, 1)",
    )
    .bind(T)
    .execute(&pool)
    .await
    .unwrap();
    let usage = profiles::usage(&pool, T).await.unwrap();
    assert_eq!(usage.len(), 2, "{usage:?}");
    let live = usage.iter().find(|u| u.purpose == "live").unwrap();
    assert_eq!(live.seconds, 180.0);
    assert!((live.cost - 0.051).abs() < 1e-9);
    let lan = usage.iter().find(|u| u.purpose == "recording").unwrap();
    assert_eq!((lan.cost, lan.unpriced_seconds), (0.0, 30.0));
    assert_eq!(profiles::cleanup(&pool).await.unwrap(), 1);
    assert!(
        profiles::usage(&pool, other_tenant(&pool).await)
            .await
            .unwrap()
            .is_empty()
    );
}

/// Runs the migrations before the profiles migration, stores settings in the
/// old columns, then applies the remaining upgrade path. The current profile
/// reader requires the current schema; migrated profile data must survive.
async fn migrate_with(pool: &PgPool, settings: &str) {
    let migrator = &talkops_core::db::MIGRATOR;
    for m in migrator.iter().filter(|m| m.version < PROFILES_MIGRATION) {
        sqlx::raw_sql(&m.sql).execute(pool).await.unwrap();
    }
    sqlx::raw_sql(settings).execute(pool).await.unwrap();
    for m in migrator.iter().filter(|m| m.version >= PROFILES_MIGRATION) {
        sqlx::raw_sql(&m.sql).execute(pool).await.unwrap();
    }
}

#[sqlx::test(migrations = false)]
async fn migration_copies_api_settings(pool: PgPool) {
    migrate_with(
        &pool,
        "UPDATE tenant_settings SET transcription_quality = 'fast', transcription_refine = 'api',
             transcription_api_model = 'gpt-4o-transcribe', transcription_api_key_enc = 'enc'",
    )
    .await;
    let list = providers::list(&pool, T).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        (
            list[0].name.as_str(),
            list[0].kind.as_str(),
            list[0].has_key
        ),
        ("OpenAI", "openai", true)
    );
    let p = profiles::list(&pool, T).await.unwrap();
    let get = |purpose: &str| p.iter().find(|p| p.purpose == purpose).unwrap();
    assert_eq!(get("live").local_model, "base");
    for purpose in ["recording", "voicemail"] {
        assert_eq!(
            (
                get(purpose).engine.as_str(),
                get(purpose).local_model.as_str()
            ),
            ("local", "fast")
        );
    }
    for purpose in ["recording_refine", "voicemail_refine"] {
        let r = get(purpose);
        assert_eq!(
            (r.engine.as_str(), r.provider_id, r.model.as_str()),
            ("cloud", Some(list[0].id), "gpt-4o-transcribe")
        );
        assert_eq!(r.list_price, Some(0.006));
    }
}

#[sqlx::test(migrations = false)]
async fn migration_keeps_local_settings(pool: PgPool) {
    migrate_with(
        &pool,
        "UPDATE tenant_settings SET transcription_quality = 'german', transcription_refine = 'german'",
    )
    .await;
    assert!(providers::list(&pool, T).await.unwrap().is_empty());
    let p = profiles::list(&pool, T).await.unwrap();
    assert_eq!(
        (p[1].engine.as_str(), p[1].local_model.as_str()),
        ("local", "german")
    );
    // The same engine twice never ran a second pass.
    assert_eq!(p[2].engine, "off");
    assert_eq!(p[4].engine, "off");
}

/// Coral connections (ADR 0046): loopback only, default URL, no key, no
/// list price; the profile engine knows its local fallback.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn coral_connections_and_fallback(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let coral = providers::create(&pool, T, &sb, &input("Coral", "coral", "", None))
        .await
        .unwrap();
    assert_eq!(
        (coral.kind.as_str(), coral.url.as_str()),
        ("coral", providers::CORAL_URL)
    );
    assert!(!coral.has_key);
    let remote = providers::create(
        &pool,
        T,
        &sb,
        &input("LAN", "coral", "http://192.168.1.9:8095/v1", None),
    )
    .await;
    assert!(
        matches!(remote, Err(CoreError::Validation(_))),
        "{remote:?}"
    );
    let other_port = providers::update(
        &pool,
        T,
        &sb,
        coral.id,
        &input("Coral", "coral", "http://localhost:9000/v1/", Some("k1")),
    )
    .await
    .unwrap();
    assert_eq!(other_port.url, "http://localhost:9000/v1");
    assert!(other_port.has_key);
    // Without TALKOPS_SECRET_KEY a key cannot be read; without a key it works.
    assert!(
        providers::config(&pool, T, None, coral.id, "coral-auto")
            .await
            .is_err()
    );
    providers::update(
        &pool,
        T,
        &sb,
        coral.id,
        &input("Coral", "coral", "", Some("")),
    )
    .await
    .unwrap();
    let cfg = providers::config(&pool, T, None, coral.id, "coral-auto")
        .await
        .unwrap();
    assert!(cfg.is_local() && cfg.key.is_none());
    assert_eq!(
        cfg.realtime_endpoint(),
        "ws://127.0.0.1:8095/v1/realtime?intent=transcription"
    );

    let mut live = cloud("live", coral.id, "coral-auto");
    live.realtime = true;
    let mut recording = cloud("recording", coral.id, "coral-auto");
    recording.local_model = "accurate".into();
    recording.price_per_minute = Some(0.5);
    let saved = profiles::update(&pool, T, &[live, recording])
        .await
        .unwrap();
    let rec = saved.iter().find(|p| p.purpose == "recording").unwrap();
    assert_eq!(rec.provider_kind.as_deref(), Some("coral"));
    assert_eq!(rec.list_price, None);
    assert_eq!(
        profiles::engine(&pool, T, Purpose::Recording)
            .await
            .unwrap(),
        Some(Engine::Cloud {
            provider_id: coral.id,
            model: "coral-auto".into(),
            realtime: false,
            coral_stable_preview: false,
            price: None,
            coral: true,
            fallback: Some(TranscriptionQuality::Accurate),
        })
    );
    assert!(matches!(
        profiles::engine(&pool, T, Purpose::Live).await.unwrap(),
        Some(Engine::Cloud {
            realtime: true,
            coral_stable_preview: false,
            coral: true,
            fallback: Some(TranscriptionQuality::Base),
            ..
        })
    ));
}

/// CUDA connections (ADR 0053) work as Coral connections: loopback only,
/// own default URL, no list price, local fallback, complete-word preview.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn cuda_connections_and_fallback(pool: PgPool) {
    let sb = SecretBox::from_hex(KEY).unwrap();
    let cuda = providers::create(&pool, T, &sb, &input("GPU", "cuda", "", None))
        .await
        .unwrap();
    assert_eq!(
        (cuda.kind.as_str(), cuda.url.as_str()),
        ("cuda", providers::CUDA_URL)
    );
    let remote = providers::create(
        &pool,
        T,
        &sb,
        &input("LAN", "cuda", "http://192.168.1.9:8096/v1", None),
    )
    .await;
    assert!(
        matches!(remote, Err(CoreError::Validation(_))),
        "{remote:?}"
    );
    let cfg = providers::config(&pool, T, None, cuda.id, "cuda-auto")
        .await
        .unwrap();
    assert!(cfg.is_local());
    assert_eq!(cfg.root(), "http://127.0.0.1:8096");

    let mut live = cloud("live", cuda.id, "cuda-auto");
    live.realtime = true;
    live.coral_stable_preview = true;
    let mut recording = cloud("recording", cuda.id, "cuda-de");
    recording.price_per_minute = Some(0.5);
    let saved = profiles::update(&pool, T, &[live, recording])
        .await
        .unwrap();
    let rec = saved.iter().find(|p| p.purpose == "recording").unwrap();
    assert_eq!(rec.list_price, None);
    assert!(matches!(
        profiles::engine(&pool, T, Purpose::Recording)
            .await
            .unwrap(),
        Some(Engine::Cloud {
            price: None,
            coral: true,
            fallback: Some(TranscriptionQuality::Fast),
            ..
        })
    ));
    assert!(matches!(
        profiles::engine(&pool, T, Purpose::Live).await.unwrap(),
        Some(Engine::Cloud {
            realtime: true,
            coral_stable_preview: true,
            coral: true,
            fallback: Some(TranscriptionQuality::Base),
            ..
        })
    ));
}

/// Transcripts keep the fallback and the language of each line; live
/// previews show their engine and fallback instead of a failure.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn fallback_and_language_are_stored(pool: PgPool) {
    use talkops_core::live_transcripts as live;
    use talkops_core::recordings::{self, Segment, Source, TranscriptMeta};
    let rec: Uuid = sqlx::query_scalar(
        "INSERT INTO recordings (tenant_id, call_uuid, file, duration_secs) VALUES ($1, 'c', 'c.wav', 3) RETURNING id",
    )
    .bind(T)
    .fetch_one(&pool)
    .await
    .unwrap();
    let line = Segment {
        start: 0.0,
        end: 1.0,
        speaker: "caller".into(),
        text: "hello".into(),
        language: Some("en".into()),
    };
    let meta = TranscriptMeta {
        engine: "whisper:base",
        is_final: true,
        fallback_from: Some("Coral/coral-auto"),
    };
    recordings::save_transcript(
        &pool,
        T,
        Source::Recording(rec),
        "en",
        std::slice::from_ref(&line),
        meta,
    )
    .await
    .unwrap();
    let t = recordings::transcript_of(&pool, T, Source::Recording(rec))
        .await
        .unwrap();
    assert_eq!(t.fallback_from.as_deref(), Some("Coral/coral-auto"));
    assert_eq!(
        (t.language.as_str(), t.segments[0].language.as_deref()),
        ("en", Some("en"))
    );
    // Lines without a language stay as before in JSON.
    let plain = Segment {
        language: None,
        ..line
    };
    assert!(!serde_json::to_string(&plain).unwrap().contains("language"));

    let user: Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, username, display_name) VALUES ($1, 'v', 'V') RETURNING id",
    )
    .bind(T)
    .fetch_one(&pool)
    .await
    .unwrap();
    let call = Uuid::new_v4();
    let file = format!("{T}/2026-10/{call}.wav");
    let now = chrono::Utc::now();
    let p = live::request(&pool, T, user, call, &file, now)
        .await
        .unwrap()
        .0;
    assert_eq!((p.engine.as_str(), p.fallback_from.as_deref()), ("", None));
    assert!(!live::stream_failed_recently(&pool, T, p.id).await.unwrap());
    assert!(live::claim_stream(&pool, T, p.id, "w1").await.unwrap());
    live::set_engine(&pool, T, p.id, "api:Coral/coral-auto", None)
        .await
        .unwrap();
    live::release_stream(&pool, T, p.id, "w1", true)
        .await
        .unwrap();
    assert!(live::stream_failed_recently(&pool, T, p.id).await.unwrap());
    assert_eq!(
        live::request(&pool, T, user, call, &file, now)
            .await
            .unwrap()
            .0
            .status,
        "failed"
    );
    // A local fallback runs: no failure to show.
    live::set_engine(&pool, T, p.id, "whisper:base", Some("Coral/coral-auto"))
        .await
        .unwrap();
    let p = live::request(&pool, T, user, call, &file, now)
        .await
        .unwrap()
        .0;
    assert_ne!(p.status, "failed");
    assert_eq!(
        (p.engine.as_str(), p.fallback_from.as_deref()),
        ("whisper:base", Some("Coral/coral-auto"))
    );
    // Another tenant cannot change it.
    let other = other_tenant(&pool).await;
    live::set_engine(&pool, other, p.id, "x", None)
        .await
        .unwrap();
    let debug = live::diagnostic_states(&pool, T).await.unwrap();
    assert_eq!(debug.len(), 1);
    assert_eq!(debug[0]["engine"], "whisper:base");
    assert!(debug[0].get("segments").is_none() && debug[0].get("file").is_none());
    assert!(
        live::diagnostic_states(&pool, other)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !live::stream_failed_recently(&pool, other, p.id)
            .await
            .unwrap()
    );
    let p = live::request(&pool, T, user, call, &file, now)
        .await
        .unwrap()
        .0;
    assert_eq!(p.engine, "whisper:base");
}
