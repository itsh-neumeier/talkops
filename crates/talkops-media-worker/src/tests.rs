//! Worker tests with real database state and a deterministic Piper process.

use super::*;
use std::os::unix::fs::PermissionsExt;
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
        current_job: std::sync::Mutex::new(None),
        coral_backoff: CoralBackoff::default(),
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
            provider_kind: None,
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
        current_job: std::sync::Mutex::new(None),
        coral_backoff: CoralBackoff::default(),
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

/// A whisper-cli stand-in: one word per 0.8 s of run time, progress on
/// stderr, word segments on stdout, the JSON result at the end. Words are
/// named `w<n>` for a whole channel and `r<n>` for a resumed rest.
fn fake_whisper(dir: &Path) -> Whisper {
    let bin = dir.join("whisper-cli");
    std::fs::write(
        &bin,
        r#"#!/usr/bin/env python3
import json, sys, time
args = sys.argv[1:]
src = args[args.index('-f') + 1]
out = args[args.index('-of') + 1]
prefix = 'r' if src.endswith('-rest.wav') else 'w'
with open(src + '.log', 'a') as log:
    log.write(' '.join(args) + '\n')
segments = []
for k in range(6):
    time.sleep(0.8)
    start, end = k * 1000, k * 1000 + 300
    text = ' %s%d' % (prefix, k)
    segments.append({'offsets': {'from': start, 'to': end}, 'text': text})
    print('[00:00:%06.3f --> 00:00:%06.3f] %s' % (start / 1000, end / 1000, text), flush=True)
    print('whisper_print_progress_callback: progress = %3d%%' % ((k + 1) * 100 // 6), file=sys.stderr, flush=True)
with open(out + '.json', 'w') as f:
    json.dump({'transcription': segments}, f)
"#,
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(dir.join("ggml-base.bin"), b"model").unwrap();
    Whisper {
        bin,
        models_dir: dir.to_owned(),
        model: "base".into(),
        threads: 1,
        vad: false,
    }
}

/// A manual transcription with a selected model: pause during the run,
/// resume at the paused position, finish with all words once; then a
/// stopped job keeps the transcript.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn manual_transcription_pauses_and_resumes_at_the_paused_position(pool: PgPool) {
    use talkops_core::jobs::JobStatus;
    use talkops_core::transcription_jobs::{self as tj, Action, EngineChoice, Scope};
    let dir = std::env::temp_dir().join(format!("talkops-worker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("recordings")).unwrap();
    let ctx = Ctx {
        pool: pool.clone(),
        piper: tts::tests::fake_piper(&dir),
        whisper: fake_whisper(&dir),
        secrets: None,
        voicemail_dir: dir.join("voicemail"),
        sounds_dir: dir.join("sounds"),
        recordings_dir: dir.join("recordings"),
        worker_id: "test".into(),
        streams: realtime::Streams::default(),
        current_job: std::sync::Mutex::new(None),
        coral_backoff: CoralBackoff::default(),
    };
    let tenant = TenantId::DEFAULT;
    // A mono recording of 10 s with a tone, so the channel is not silent.
    let wav = dir.join("recordings/call.wav");
    let mut w = hound::WavWriter::create(
        &wav,
        hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..80_000 {
        w.write_sample(((i as f32 * 0.05).sin() * 8000.0) as i16)
            .unwrap();
    }
    w.finalize().unwrap();
    let rec: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO recordings (tenant_id, call_uuid, file, duration_secs)
         VALUES ($1, 'call', 'call.wav', 10) RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let admin: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, username, display_name, role)
         VALUES ($1, 'admin', 'Admin', 'admin') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let choice = EngineChoice::Local {
        model: "base".into(),
    };
    let job = tj::enqueue_manual(&pool, tenant, rec, &choice, admin)
        .await
        .unwrap();
    // A second manual job of the same recording waits for the first.
    assert!(
        tj::enqueue_manual(&pool, tenant, rec, &choice, admin)
            .await
            .is_err()
    );

    let status = |pool: PgPool| async move {
        sqlx::query_as::<_, (JobStatus, Option<f32>)>(
            "SELECT status, progress FROM jobs WHERE id = $1",
        )
        .bind(job)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let ctx = std::sync::Arc::new(ctx);
    let runner = {
        let ctx = ctx.clone();
        tokio::spawn(async move { drain_queue(&ctx, "test", &[recordings::JOB_TRANSCRIBE]).await })
    };
    // Wait for progress, then pause.
    for _ in 0..100 {
        if status(pool.clone()).await.1.unwrap_or(0.0) > 0.2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tj::act(&pool, tenant, job, Action::Pause).await.unwrap();
    runner.await.unwrap().unwrap();
    let (paused, progress) = status(pool.clone()).await;
    assert_eq!(paused, JobStatus::Paused);
    let progress = progress.unwrap();
    assert!(progress > 0.0 && progress < 1.0, "{progress}");
    let list = tj::list(&pool, tenant, &Scope::All, false).await.unwrap();
    assert!(list[0].resumable);
    assert_eq!(list[0].requested_by.as_deref(), Some("Admin"));

    // Resume: only the rest of the audio is transcribed.
    tj::act(&pool, tenant, job, Action::Resume).await.unwrap();
    drain_queue(&ctx, "test", &[recordings::JOB_TRANSCRIBE])
        .await
        .unwrap();
    assert_eq!(status(pool.clone()).await.0, JobStatus::Done);
    let t = recordings::transcript_of(&pool, tenant, Source::Recording(rec))
        .await
        .unwrap();
    let words: Vec<&str> = t.text.split_whitespace().collect();
    let firsts = words.iter().filter(|w| w.starts_with('w')).count();
    assert!((1..6).contains(&firsts), "{words:?}");
    assert_eq!(
        words.iter().filter(|w| w.starts_with('r')).count(),
        6,
        "the rest after the pause: {words:?}"
    );
    assert_eq!(words.len(), firsts + 6, "no word twice: {words:?}");
    // The resumed words come after the paused position.
    let starts: Vec<f32> = t.segments.iter().map(|s| s.start).collect();
    assert!(starts.windows(2).all(|p| p[0] <= p[1]), "{starts:?}");
    assert_eq!(t.engine, "whisper:base");
    assert!(t.is_final);
    let speeds = tj::speeds(&pool).await.unwrap();
    assert!(
        !speeds.contains_key("local:base"),
        "a resumed run is no speed sample"
    );

    // Stop: a queued manual job is cancelled, the transcript stays.
    let again = tj::enqueue_manual(&pool, tenant, rec, &choice, admin)
        .await
        .unwrap();
    tj::act(&pool, tenant, again, Action::Stop).await.unwrap();
    let cancelled: JobStatus = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(again)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cancelled, JobStatus::Cancelled);
    assert_eq!(
        recordings::get(&pool, tenant, rec)
            .await
            .unwrap()
            .transcript_status,
        "done"
    );
    let history = tj::list(&pool, tenant, &Scope::All, true).await.unwrap();
    assert_eq!(history.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

/// With a real whisper-cli (not in CI): a run that is paused in the middle
/// and resumed gives about the same words as a run without a pause. Use
/// speech without repeated sentences: Whisper can drop or repeat whole
/// sentences of a looped sample after any cut. `TALKOPS_TEST_VAD=1` turns
/// on VAD (needs the Silero model in the models directory).
/// `TALKOPS_TEST_WHISPER=<whisper-cli> TALKOPS_TEST_MODELS=<dir with
/// ggml-base.bin> TALKOPS_TEST_WAV=<speech.wav> cargo test -- --ignored`
#[tokio::test]
#[ignore = "needs whisper-cli, a model and a speech recording"]
async fn real_whisper_resumes_at_the_paused_position() {
    let var = |k: &str| std::env::var(k).expect(k);
    let whisper = Whisper {
        bin: var("TALKOPS_TEST_WHISPER").into(),
        models_dir: var("TALKOPS_TEST_MODELS").into(),
        model: "base".into(),
        threads: 4,
        vad: std::env::var("TALKOPS_TEST_VAD").is_ok(),
    };
    let wav = std::path::PathBuf::from(var("TALKOPS_TEST_WAV"));
    let opts = transcribe::Options {
        quality: settings::TranscriptionQuality::Fast,
        vocabulary: String::new(),
    };
    let words = |segments: &[recordings::Segment]| -> Vec<String> {
        segments
            .iter()
            .flat_map(|s| s.text.split_whitespace())
            .map(|w| {
                w.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .filter(|w| !w.is_empty())
            .collect()
    };
    let full = whisper.transcribe(&wav, "en", &opts).await.unwrap();
    let full = words(&full);

    let (progress_tx, mut progress_rx) = tokio::sync::watch::channel(0.0f32);
    let (interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(None);
    let control = Control {
        progress: progress_tx,
        interrupt: interrupt_rx,
    };
    let pauser = tokio::spawn(async move {
        while progress_rx.changed().await.is_ok() {
            if *progress_rx.borrow() > 0.45 {
                let _ = interrupt_tx.send(Some(Interrupt::Pause));
                return;
            }
        }
    });
    let checkpoint = match whisper
        .transcribe_with(&wav, "en", &opts, Some(&control), None)
        .await
        .unwrap()
    {
        Outcome::Paused(c) => c,
        _ => panic!("the run did not pause"),
    };
    pauser.await.unwrap();
    assert!(checkpoint.offset > 0.0 && !checkpoint.partial.is_empty());
    let resumed = match whisper
        .transcribe_with(&wav, "en", &opts, None, Some(checkpoint.clone()))
        .await
        .unwrap()
    {
        Outcome::Done(segments) => segments,
        _ => panic!("the resumed run did not finish"),
    };
    let resumed = words(&resumed);
    eprintln!("paused at {:.1} s", checkpoint.offset);

    eprintln!("full:    {}", full.join(" "));
    eprintln!("resumed: {}", resumed.join(" "));
    let diff = (full.len() as f32 - resumed.len() as f32).abs() / full.len() as f32;
    assert!(diff < 0.1, "{} vs {} words", full.len(), resumed.len());
    let same = resumed.iter().filter(|w| full.contains(w)).count();
    assert!(same as f32 / resumed.len() as f32 > 0.85);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn analyses_a_finished_wifi_survey(pool: PgPool) {
    use talkops_core::wifi_survey as ws;
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
        current_job: std::sync::Mutex::new(None),
        coral_backoff: CoralBackoff::default(),
    };
    let tenant = TenantId::DEFAULT;
    let input: extensions::ExtensionInput =
        serde_json::from_value(serde_json::json!({"number":"206", "display_name":"Walker"}))
            .unwrap();
    let ext = extensions::create(&pool, tenant, &input, &[])
        .await
        .unwrap();
    let device: extensions::DeviceInput =
        serde_json::from_value(serde_json::json!({"name":"AX86R", "kind":"wifi"})).unwrap();
    let sb = talkops_core::crypto::SecretBox::from_hex(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
    )
    .unwrap();
    let (device, _) = extensions::create_device(&pool, tenant, &sb, &ext, &device)
        .await
        .unwrap();
    let target = ws::target(&pool, tenant, device.id).await.unwrap();
    let survey = ws::create(
        &pool,
        tenant,
        &ws::NewSurvey {
            device_id: device.id,
            mode: "originate".into(),
            label: String::new(),
        },
        &target,
        None,
        serde_json::json!({}),
    )
    .await
    .unwrap();
    ws::mark_answered(
        &pool,
        tenant,
        survey.id,
        sqlx::types::chrono::Utc::now(),
        None,
    )
    .await
    .unwrap();
    for s in 1..=10 {
        ws::add_radio(
            &pool,
            tenant,
            survey.id,
            sqlx::types::chrono::Utc::now(),
            &ws::RadioSample {
                offset_ms: s * 1000,
                ap_mac: Some(if s < 5 { "a" } else { "b" }.into()),
                ap_name: None,
                bssid: None,
                radio: Some("na".into()),
                channel: Some(36),
                rssi: Some(-60),
                tx_rate_kbps: None,
                rx_rate_kbps: None,
                error: None,
            },
        )
        .await
        .unwrap();
    }
    ws::finish(&pool, tenant, survey.id, "NORMAL_CLEARING", None)
        .await
        .unwrap();
    let job = jobs::claim(&pool, "test", &[ws::JOB_ANALYSE])
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(run(&ctx, &job).await.unwrap(), JobEnd::Done));
    let s = ws::get(&pool, tenant, survey.id).await.unwrap();
    assert_eq!(s.state, "analysed");
    assert_eq!(ws::roams(&pool, tenant, survey.id).await.unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A whisper-cli stand-in without delays: words `f0`…`f2` per channel.
fn fast_whisper(dir: &Path) -> Whisper {
    let bin = dir.join("fast-whisper-cli");
    std::fs::write(
        &bin,
        r#"#!/usr/bin/env python3
import json, sys
args = sys.argv[1:]
out = args[args.index('-of') + 1]
segments = [{'offsets': {'from': k * 1000, 'to': k * 1000 + 300}, 'text': ' f%d' % k} for k in range(3)]
with open(out + '.json', 'w') as f:
    json.dump({'transcription': segments}, f)
"#,
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(dir.join("ggml-base.bin"), b"model").unwrap();
    std::fs::write(dir.join("ggml-large-v3-q5_0.bin"), b"model").unwrap();
    Whisper {
        bin,
        models_dir: dir.to_owned(),
        model: "base".into(),
        threads: 1,
        vad: false,
    }
}

fn coral_ctx(pool: &PgPool, dir: &Path) -> Ctx {
    Ctx {
        pool: pool.clone(),
        piper: tts::tests::fake_piper(dir),
        whisper: fast_whisper(dir),
        // A Coral connection has no key: no TALKOPS_SECRET_KEY needed.
        secrets: None,
        voicemail_dir: dir.join("voicemail"),
        sounds_dir: dir.join("sounds"),
        recordings_dir: dir.join("recordings"),
        worker_id: "test".into(),
        streams: realtime::Streams::default(),
        current_job: std::sync::Mutex::new(None),
        coral_backoff: CoralBackoff::default(),
    }
}

/// How the Coral test double answers.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CoralMode {
    Ok,
    /// HTTP 503 / realtime `error` with code `overloaded`.
    Overloaded,
    /// The realtime connection closes without an answer.
    Close,
}

/// A coral-stt stand-in: `/v1/audio/transcriptions` answers verbose JSON
/// with the language of each segment; `/v1/realtime` answers each commit
/// with an English transcript. Counts the requests.
async fn fake_coral(
    mode: std::sync::Arc<std::sync::Mutex<CoralMode>>,
    hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> u16 {
    use futures_util::{SinkExt, StreamExt};
    use std::sync::atomic::Ordering;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_tungstenite::tungstenite::Message;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let (mode, hits) = (mode.clone(), hits.clone());
            tokio::spawn(async move {
                let mut head = [0u8; 4];
                let _ = tcp.peek(&mut head).await;
                hits.fetch_add(1, Ordering::Relaxed);
                let now = *mode.lock().unwrap();
                let mut peek = vec![0u8; 512];
                let n = tcp.peek(&mut peek).await.unwrap_or(0);
                if String::from_utf8_lossy(&peek[..n]).contains("/v1/realtime") {
                    let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else {
                        return;
                    };
                    let mut item = 0;
                    while let Some(Ok(Message::Text(text))) = ws.next().await {
                        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
                        match (now, v["type"].as_str().unwrap_or_default()) {
                            (CoralMode::Overloaded, "session.update") => {
                                let e = serde_json::json!({"type": "error", "error": {"type": "server_error", "code": "overloaded", "message": "all Edge TPUs are busy"}});
                                let _ = ws.send(Message::Text(e.to_string().into())).await;
                                let _ = ws.close(None).await;
                                return;
                            }
                            (CoralMode::Close, _) => return,
                            (CoralMode::Ok, "input_audio_buffer.commit") => {
                                item += 1;
                                let id = format!("item_{item}");
                                for e in [
                                    serde_json::json!({"type": "input_audio_buffer.committed", "item_id": id}),
                                    serde_json::json!({"type": "conversation.item.input_audio_transcription.completed", "item_id": id, "transcript": "hello there", "language": "en"}),
                                ] {
                                    ws.send(Message::Text(e.to_string().into())).await.unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                    return;
                }
                // HTTP: read the whole request, then answer.
                let mut bytes = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    let n = tcp.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        let len: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .map_or(0, |v| v.trim().parse().unwrap());
                        if bytes.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let (status, body) = match now {
                    CoralMode::Ok => (
                        "200 OK",
                        serde_json::json!({
                            "task": "transcribe", "language": "en", "duration": 4.0, "text": "hello there",
                            "segments": [{"id": 0, "start": 0.2, "end": 1.5, "text": "hello there", "language": "en"}],
                            "words": [{"word": "hello", "start": 0.2, "end": 0.6}, {"word": "there", "start": 0.7, "end": 1.5}]
                        }),
                    ),
                    _ => (
                        "503 Service Unavailable",
                        serde_json::json!({"error": {"message": "all Edge TPUs are busy", "type": "server_error", "code": "overloaded"}}),
                    ),
                };
                let body = body.to_string();
                let res = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\nretry-after: 5\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = tcp.write_all(res.as_bytes()).await;
            });
        }
    });
    port
}

async fn coral_provider(pool: &PgPool, url: &str) -> uuid::Uuid {
    use talkops_core::transcription_api::{self as providers, ProviderInput};
    let secrets = SecretBox::from_hex(&"00".repeat(32)).unwrap();
    providers::create(
        pool,
        TenantId::DEFAULT,
        &secrets,
        &ProviderInput {
            name: "Coral".into(),
            kind: "coral".into(),
            url: url.into(),
            key: None,
        },
    )
    .await
    .unwrap()
    .id
}

fn coral_profile(
    purpose: &str,
    provider: uuid::Uuid,
    realtime: bool,
) -> talkops_core::transcription_profiles::Profile {
    talkops_core::transcription_profiles::Profile {
        purpose: purpose.into(),
        engine: "cloud".into(),
        // The local fallback of recordings.
        local_model: if purpose == "live" {
            "base"
        } else {
            "accurate"
        }
        .into(),
        provider_id: Some(provider),
        model: "coral-auto".into(),
        realtime,
        price_per_minute: None,
        list_price: None,
        provider_kind: None,
    }
}

async fn recording_job(pool: &PgPool, dir: &Path, name: &str) -> uuid::Uuid {
    let tenant = TenantId::DEFAULT;
    let wav = dir.join("recordings").join(format!("{name}.wav"));
    std::fs::create_dir_all(wav.parent().unwrap()).unwrap();
    let mut w = hound::WavWriter::create(
        &wav,
        hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..32_000 {
        w.write_sample(((i as f32 * 0.05).sin() * 8000.0) as i16)
            .unwrap();
    }
    w.finalize().unwrap();
    let rec: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO recordings (tenant_id, call_uuid, file, duration_secs)
         VALUES ($1, $2, $3, 4) RETURNING id",
    )
    .bind(tenant)
    .bind(name)
    .bind(format!("{name}.wav"))
    .fetch_one(pool)
    .await
    .unwrap();
    let payload = TranscribePayload {
        recording_id: Some(rec),
        voicemail_id: None,
        pass: 1,
        engine: None,
    };
    let mut job = jobs::NewJob::new(
        recordings::JOB_TRANSCRIBE,
        serde_json::to_value(&payload).unwrap(),
    );
    job.tenant_id = tenant;
    jobs::enqueue(pool, job).await.unwrap();
    rec
}

/// Recordings with a Coral connection: the service transcribes and names
/// the language; overload, the 30 s backoff and an unreachable service use
/// the local fallback model of the profile.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn coral_recordings_use_the_service_or_the_local_fallback(pool: PgPool) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use talkops_core::transcription_profiles as profiles;
    let tenant = TenantId::DEFAULT;
    let dir = std::env::temp_dir().join(format!("talkops-worker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mode = std::sync::Arc::new(std::sync::Mutex::new(CoralMode::Ok));
    let hits = std::sync::Arc::new(AtomicUsize::new(0));
    let port = fake_coral(mode.clone(), hits.clone()).await;
    let provider = coral_provider(&pool, &format!("http://127.0.0.1:{port}/v1")).await;
    profiles::update(
        &pool,
        tenant,
        &[coral_profile("recording", provider, false)],
    )
    .await
    .unwrap();
    let ctx = coral_ctx(&pool, &dir);
    let lane = [recordings::JOB_TRANSCRIBE];

    // 1. Coral answers: its text and detected language are stored.
    let rec = recording_job(&pool, &dir, "ok").await;
    drain_queue(&ctx, "test", &lane).await.unwrap();
    let t = recordings::transcript_of(&pool, tenant, Source::Recording(rec))
        .await
        .unwrap();
    assert_eq!(t.engine, "api:Coral/coral-auto");
    assert_eq!(t.fallback_from, None);
    assert_eq!(t.language, "en");
    assert_eq!(t.text, "hello there");
    assert_eq!(t.segments[0].language.as_deref(), Some("en"));
    assert!(
        profiles::usage(&pool, tenant).await.unwrap().is_empty(),
        "Coral costs nothing"
    );
    let after_ok = hits.load(Ordering::Relaxed);
    assert!(after_ok >= 1);

    // 2. Overloaded (HTTP 503): the local fallback model of the profile.
    *mode.lock().unwrap() = CoralMode::Overloaded;
    let rec = recording_job(&pool, &dir, "busy").await;
    drain_queue(&ctx, "test", &lane).await.unwrap();
    let t = recordings::transcript_of(&pool, tenant, Source::Recording(rec))
        .await
        .unwrap();
    assert_eq!(
        t.engine, "whisper:large-v3-q5_0",
        "the profile's local model (accurate)"
    );
    assert_eq!(t.fallback_from.as_deref(), Some("Coral/coral-auto"));
    assert_eq!(t.language, "de", "the tenant language without Coral");
    assert!(t.text.starts_with("f0"), "{}", t.text);
    let after_busy = hits.load(Ordering::Relaxed);
    assert_eq!(after_busy, after_ok + 1);

    // 3. Within the 30 s backoff Coral is not asked again.
    *mode.lock().unwrap() = CoralMode::Ok;
    let rec = recording_job(&pool, &dir, "backoff").await;
    drain_queue(&ctx, "test", &lane).await.unwrap();
    let t = recordings::transcript_of(&pool, tenant, Source::Recording(rec))
        .await
        .unwrap();
    assert_eq!(t.fallback_from.as_deref(), Some("Coral/coral-auto"));
    assert_eq!(hits.load(Ordering::Relaxed), after_busy);

    // 4. After the backoff Coral is tried again.
    ctx.coral_backoff.0.lock().unwrap().clear();
    let rec = recording_job(&pool, &dir, "again").await;
    drain_queue(&ctx, "test", &lane).await.unwrap();
    let t = recordings::transcript_of(&pool, tenant, Source::Recording(rec))
        .await
        .unwrap();
    assert_eq!(t.engine, "api:Coral/coral-auto");

    // 5. Not reachable (nothing listens): the fallback.
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead = closed.local_addr().unwrap().port();
    drop(closed);
    sqlx::query("UPDATE transcription_providers SET url = $1")
        .bind(format!("http://127.0.0.1:{dead}/v1"))
        .execute(&pool)
        .await
        .unwrap();
    let rec = recording_job(&pool, &dir, "down").await;
    drain_queue(&ctx, "test", &lane).await.unwrap();
    let t = recordings::transcript_of(&pool, tenant, Source::Recording(rec))
        .await
        .unwrap();
    assert_eq!(t.fallback_from.as_deref(), Some("Coral/coral-auto"));
    assert!(t.is_final);
    std::fs::remove_dir_all(&dir).ok();
}

/// The live preview with Coral Realtime: a failed stream (overloaded,
/// broken connection, service down) is replaced by local Whisper `base` in
/// parts; after the 30 s backoff the stream starts again and the segments
/// name their language.
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn coral_live_preview_falls_back_and_retries(pool: PgPool) {
    use std::sync::atomic::AtomicUsize;
    use talkops_core::live_transcripts as live;
    use talkops_core::transcription_profiles as profiles;
    let tenant = TenantId::DEFAULT;
    let dir = std::env::temp_dir().join(format!("talkops-worker-{}", uuid::Uuid::new_v4()));
    let mode = std::sync::Arc::new(std::sync::Mutex::new(CoralMode::Overloaded));
    let hits = std::sync::Arc::new(AtomicUsize::new(0));
    let port = fake_coral(mode.clone(), hits.clone()).await;
    let provider = coral_provider(&pool, &format!("http://127.0.0.1:{port}/v1")).await;
    profiles::update(&pool, tenant, &[coral_profile("live", provider, true)])
        .await
        .unwrap();
    sqlx::query("UPDATE tenant_settings SET transcription_enabled = true, default_language = 'de'")
        .execute(&pool)
        .await
        .unwrap();
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
    let request = || {
        live::request(
            &pool,
            tenant,
            user,
            call,
            &file,
            sqlx::types::chrono::Utc::now(),
        )
    };
    let preview = request().await.unwrap().0;
    let ctx = coral_ctx(&pool, &dir);
    let job = jobs::claim(&pool, "test", &[live::JOB])
        .await
        .unwrap()
        .unwrap();
    let failed = || live::stream_failed_recently(&pool, tenant, preview.id);
    let wait_failed = || async {
        for _ in 0..100 {
            if failed().await.unwrap() && ctx.streams.is_empty() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    };
    let reset = || async {
        sqlx::query(
            "UPDATE live_transcript_previews SET stream_failed_at = now() - interval '31 seconds'",
        )
        .execute(&pool)
        .await
        .unwrap();
    };

    for (case, down) in [
        (CoralMode::Overloaded, false),
        (CoralMode::Close, false),
        (CoralMode::Ok, true),
    ] {
        *mode.lock().unwrap() = case;
        if down {
            // The service is not running.
            let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let dead = closed.local_addr().unwrap().port();
            drop(closed);
            sqlx::query("UPDATE transcription_providers SET url = $1")
                .bind(format!("http://127.0.0.1:{dead}/v1"))
                .execute(&pool)
                .await
                .unwrap();
        }
        // The stream fails in the background.
        run(&ctx, &job).await.unwrap();
        assert!(wait_failed().await, "{case:?}: the stream fails");
        // The next job runs the local fallback in parts.
        run(&ctx, &job).await.unwrap();
        let p = request().await.unwrap().0;
        assert_eq!(p.engine, "whisper:base", "{case:?}: {p:?}");
        assert_eq!(
            p.fallback_from.as_deref(),
            Some("Coral/coral-auto"),
            "{case:?}"
        );
        assert_ne!(p.status, "failed", "{case:?}: the fallback is no failure");
        assert!(
            p.segments.iter().any(|s| s.text.starts_with("f")),
            "{case:?}: {:?}",
            p.segments
        );
        reset().await;
    }

    // Back up: after the backoff the next job streams over Coral again.
    sqlx::query("UPDATE transcription_providers SET url = $1")
        .bind(format!("http://127.0.0.1:{port}/v1"))
        .execute(&pool)
        .await
        .unwrap();
    *mode.lock().unwrap() = CoralMode::Ok;
    // Clear the text of the fallback, so the streamed text shows.
    sqlx::query("UPDATE live_transcript_previews SET segments = '[]', cursor_frames = -1")
        .execute(&pool)
        .await
        .unwrap();
    run(&ctx, &job).await.unwrap();
    let mut streamed = None;
    for _ in 0..100 {
        let p = request().await.unwrap().0;
        if let Some(s) = p.segments.iter().find(|s| s.text == "hello there") {
            streamed = Some((p.clone(), s.clone()));
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let (p, segment) = streamed.expect("streamed text");
    assert_eq!(p.engine, "api:Coral/coral-auto");
    assert_eq!(p.fallback_from, None);
    assert_eq!(segment.language.as_deref(), Some("en"));
    assert!(
        profiles::usage(&pool, tenant).await.unwrap().is_empty(),
        "Coral costs nothing"
    );
    live::stop(&pool, tenant, user, call).await.unwrap();
    for _ in 0..100 {
        if ctx.streams.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    std::fs::remove_dir_all(&dir).ok();
}
