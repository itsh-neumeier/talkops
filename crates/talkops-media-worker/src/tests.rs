//! Worker tests with real database state and a deterministic Piper process.

use super::*;
use talkops_core::tenant::TenantId;
use talkops_core::{extensions, voicemail_config};

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn renders_box_speeds_and_refreshes_existing_message_information(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("talkops-worker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let ctx = Ctx {
        pool: pool.clone(),
        piper: tts::tests::fake_piper(&dir),
        whisper: Whisper {
            bin: dir.join("unused"),
            models_dir: dir.clone(),
            model: "base".into(),
            threads: 1,
            vad: false,
        },
        secrets: None,
        voicemail_dir: dir.join("voicemail"),
        sounds_dir: dir.join("sounds"),
        recordings_dir: dir.join("recordings"),
        worker_id: "test".into(),
        streams: realtime::Streams::default(),
    };
    let tenant = TenantId::DEFAULT;
    let input: extensions::ExtensionInput = serde_json::from_value(serde_json::json!({
        "number":"205", "display_name":"Test"
    }))
    .unwrap();
    let ext = extensions::create(&pool, tenant, &input, &[])
        .await
        .unwrap();
    let mut box_input: voicemail::VoicemailBoxInput = serde_json::from_value(serde_json::json!({
        "enabled":true, "speech_speed":0.7
    }))
    .unwrap();
    voicemail::update_box(&pool, tenant, ext.id, &box_input, "de")
        .await
        .unwrap();
    let msg = voicemail::create_message(
        &pool,
        tenant,
        &voicemail::NewMessage {
            id: uuid::Uuid::new_v4(),
            extension_id: ext.id,
            caller_number: "+4930123456".into(),
            caller_name: "Test Caller".into(),
            duration_secs: 3,
            call_uuid: None,
        },
    )
    .await
    .unwrap();
    let file = ctx.voicemail_dir.join(&msg.file);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"recording").unwrap();
    assert!(tenant_prompts(&ctx, Some(tenant)).await.unwrap() > 0);
    let config = voicemail_config::VoicemailConfig::default();
    let text = config.rendered_text("vm_main_menu", "de", 0.7).unwrap();
    let slow = talkops_core::prompts::path_for(
        &ctx.sounds_dir,
        "vm_main_menu",
        "de",
        config.voice_model("de"),
        &text,
    );
    assert!(slow.is_file());
    let old = voicemail_config::message_info_job(&pool, tenant, &msg)
        .await
        .unwrap()
        .unwrap();
    assert!(ctx.voicemail_dir.join(&old.file).is_file());
    assert_eq!(tenant_prompts(&ctx, Some(tenant)).await.unwrap(), 0);
    box_input.speech_speed = Some(1.0);
    voicemail::update_box(&pool, tenant, ext.id, &box_input, "de")
        .await
        .unwrap();
    assert!(
        tenant_prompts(&ctx, Some(tenant)).await.unwrap() > 0,
        "explicit jobs also refresh a default configuration"
    );
    let new = voicemail_config::message_info_job(&pool, tenant, &msg)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(new.file, old.file);
    assert!(ctx.voicemail_dir.join(new.file).is_file());
    assert_eq!(
        tenant_prompts(&ctx, Some(TenantId(uuid::Uuid::new_v4())))
            .await
            .unwrap(),
        0
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// A fake Realtime server: answers each commit with a committed item and its
/// transcript, and records the events it received.
async fn fake_realtime(events: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>>) -> u16 {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut n = 0;
        while let Ok((tcp, _)) = listener.accept().await {
            let events = events.clone();
            n += 1;
            let channel = n;
            tokio::spawn(async move {
                #[allow(clippy::result_large_err)] // the callback signature is fixed
                fn check(req: &Request, res: Response) -> Result<Response, ErrorResponse> {
                    assert_eq!(req.uri().path(), "/v1/realtime");
                    assert_eq!(req.uri().query(), Some("intent=transcription"));
                    assert_eq!(req.headers()["authorization"], "Bearer sk-live");
                    Ok(res)
                }
                let mut ws = tokio_tungstenite::accept_hdr_async(tcp, check)
                    .await
                    .unwrap();
                let mut item = 0;
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
                    if v["type"] == "input_audio_buffer.commit" {
                        item += 1;
                        let id = format!("item-{channel}-{item}");
                        for answer in [
                            serde_json::json!({"type": "input_audio_buffer.committed", "item_id": id}),
                            serde_json::json!({"type": "conversation.item.input_audio_transcription.delta", "item_id": id, "delta": "Hal"}),
                            serde_json::json!({"type": "conversation.item.input_audio_transcription.completed", "item_id": id, "transcript": "Hallo Welt"}),
                        ] {
                            ws.send(Message::Text(answer.to_string().into()))
                                .await
                                .unwrap();
                        }
                    }
                    if v["type"] != "input_audio_buffer.append" {
                        events.lock().unwrap().push(v);
                    }
                }
            });
        }
    });
    port
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn streams_live_previews_over_the_realtime_api(pool: PgPool) {
    use talkops_core::transcription_api::{self as providers, ProviderInput};
    use talkops_core::transcription_profiles::{self as profiles, Profile};
    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    let tenant = TenantId::DEFAULT;
    let dir = std::env::temp_dir().join(format!("talkops-worker-{}", uuid::Uuid::new_v4()));
    let secrets = SecretBox::from_hex(KEY).unwrap();
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let port = fake_realtime(events.clone()).await;
    let provider = providers::create(
        &pool,
        tenant,
        &secrets,
        &ProviderInput {
            name: "Fake".into(),
            kind: "compatible".into(),
            url: format!("http://127.0.0.1:{port}/v1"),
            key: Some("sk-live".into()),
        },
    )
    .await
    .unwrap();
    profiles::update(
        &pool,
        tenant,
        &[Profile {
            purpose: "live".into(),
            engine: "cloud".into(),
            local_model: "base".into(),
            provider_id: Some(provider.id),
            model: "gpt-live-transcribe".into(),
            realtime: true,
            price_per_minute: Some(0.06),
            list_price: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("UPDATE tenant_settings SET transcription_enabled = true, default_language = 'de'")
        .execute(&pool)
        .await
        .unwrap();
    // An active recording: the caller speaks for two seconds.
    let call = uuid::Uuid::new_v4();
    let file = format!("{tenant}/2026-10/{call}.wav");
    let path = dir.join("recordings").join(&file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..8000 * 10 {
        let speech = (8000 * 4..8000 * 6).contains(&i);
        let s = if speech {
            ((i as f32 * 0.3).sin() * 9000.0) as i16
        } else {
            0
        };
        writer.write_sample(s).unwrap();
        writer.write_sample(0i16).unwrap();
    }
    writer.finalize().unwrap();
    let user: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, username, display_name) VALUES ($1, 'viewer', 'Viewer') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let preview = talkops_core::live_transcripts::request(
        &pool,
        tenant,
        user,
        call,
        &file,
        sqlx::types::chrono::Utc::now(),
    )
    .await
    .unwrap()
    .0;
    let ctx = Ctx {
        pool: pool.clone(),
        piper: tts::tests::fake_piper(&dir),
        whisper: Whisper {
            bin: dir.join("unused"),
            models_dir: dir.clone(),
            model: "base".into(),
            threads: 1,
            vad: false,
        },
        secrets: Some(secrets),
        voicemail_dir: dir.join("voicemail"),
        sounds_dir: dir.join("sounds"),
        recordings_dir: dir.join("recordings"),
        worker_id: "test".into(),
        streams: realtime::Streams::default(),
    };
    let job = jobs::claim(&pool, "test", &[talkops_core::live_transcripts::JOB])
        .await
        .unwrap()
        .unwrap();
    run(&ctx, &job).await.unwrap();
    // The stream runs in the background; a second job does not start another.
    run(&ctx, &job).await.unwrap();
    let mut segments = Vec::new();
    for _ in 0..100 {
        let work = talkops_core::live_transcripts::work(&pool, tenant, preview.id)
            .await
            .unwrap()
            .unwrap();
        segments = work.segments.0;
        if !segments.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(segments.len(), 1, "{segments:?}");
    assert_eq!(segments[0].text, "Hallo Welt");
    assert_eq!(segments[0].speaker, "caller");
    // The file has 9.5 s readable; the backlog starts at 1.5 s, speech at 4 s.
    assert!((segments[0].start - 3.7).abs() < 0.05, "{segments:?}");
    assert!((segments[0].end - 6.7).abs() < 0.05, "{segments:?}");
    let received = events.lock().unwrap().clone();
    assert_eq!(
        received
            .iter()
            .filter(|e| e["type"] == "session.update")
            .count(),
        2,
        "one session per channel"
    );
    assert_eq!(
        received[0]["session"]["audio"]["input"]["transcription"]["model"],
        "gpt-live-transcribe"
    );
    // Only the caller's speech (with lead-in and pause) is billed.
    let usage = profiles::usage(&pool, tenant).await.unwrap();
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].engine, "Fake/gpt-live-transcribe");
    assert!((usage[0].seconds - 3.0).abs() < 0.05, "{usage:?}");
    // The viewer leaves: the stream stops at the next heartbeat.
    talkops_core::live_transcripts::stop(&pool, tenant, user, call)
        .await
        .unwrap();
    for _ in 0..100 {
        if ctx.streams.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ctx.streams.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}
