//! Audio clips: voices, generating, uploading, greetings and cleanup.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use common::*;

fn wav(samples: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::new(&mut out, spec).unwrap();
    for i in 0..samples {
        w.write_sample(((i % 64) as i16 - 32) * 100).unwrap();
    }
    w.finalize().unwrap();
    out.into_inner()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn speech_speed_is_stored_and_shared_greetings_are_copied(db: PgPool) {
    let router = talkops_api::app(state(db.clone()), None);
    let admin = setup_admin(&router).await;
    let (status, clip) = admin
        .post(
            "/api/v1/audio/clips/tts",
            json!({"text":"Hallo <speed:1.2>Welt</speed>", "speech_speed":0.7}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{clip}");
    assert_eq!(clip["speech_speed"], 0.7);
    let clip_id = clip["id"].as_str().unwrap();
    let work = talkops_core::audio::render_job(&db, clip_id.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(work.speech_speed, 0.7);
    assert!(work.text.contains("<speed:1.2>"));
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"301", "display_name":"Test"}),
        )
        .await;
    let vm = format!(
        "/api/v1/extensions/{}/voicemail",
        ext["id"].as_str().unwrap()
    );
    let (_, default) = admin.get(&vm).await;
    assert_eq!(default["speech_speed"], 1.0);
    let (status, box_view) = admin
        .put(
            &vm,
            json!({"enabled":true, "greeting":"clip",
        "greeting_clip_id":clip_id, "speech_speed":0.8}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{box_view}");
    assert_eq!(box_view["speech_speed"], 0.8);
    let copy_id = box_view["greeting_clip_id"].as_str().unwrap();
    assert_ne!(copy_id, clip_id);
    let (_, original) = admin.get(&format!("/api/v1/audio/clips/{clip_id}")).await;
    let (_, copy) = admin.get(&format!("/api/v1/audio/clips/{copy_id}")).await;
    assert_eq!(original["speech_speed"], 0.7);
    assert_eq!(copy["speech_speed"], 0.8);
    assert_eq!(original["text"], copy["text"]);
    let (status, kept) = admin
        .put(
            &vm,
            json!({"enabled":true, "greeting":"clip", "greeting_clip_id":copy_id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        kept["speech_speed"], 0.8,
        "old clients do not reset the box speed"
    );
    assert_eq!(kept["greeting_clip_id"], copy_id);
    for input in [
        json!({"enabled":true,"speech_speed":0.4}),
        json!({"enabled":true,"speech_speed":2.1}),
        json!({"enabled":true,"greeting":"tts","greeting_text":"<speed:0.7>x"}),
    ] {
        let (status, _) = admin.put(&vm, input).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'tts_prompts'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(
        queued, 1,
        "only the actual speed change queues system prompts"
    );
    assert!(
        sqlx::query("UPDATE voicemail_boxes SET speech_speed = 0.4")
            .execute(&db)
            .await
            .is_err()
    );
}

async fn upload(
    router: &axum::Router,
    c: &Client,
    source: &str,
    data: &[u8],
) -> (StatusCode, Value) {
    let mut body = format!(
        "--b\r\nContent-Disposition: form-data; name=\"source\"\r\n\r\n{source}\r\n\
         --b\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.wav\"\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\r\n--b--\r\n");
    let res = raw(
        router,
        Request::post("/api/v1/audio/clips")
            .header("cookie", &c.cookie)
            .header("x-requested-with", "TalkOps")
            .header("x-csrf-token", &c.csrf)
            .header("content-type", "multipart/form-data; boundary=b")
            .body(Body::from(body))
            .unwrap(),
    )
    .await;
    let status = res.status();
    (status, body_json(res).await)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn audio_clips(db: PgPool) {
    let state = state(db.clone());
    let media = state.media.clone();
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;

    // Two voices per language.
    let (status, voices) = admin.get("/api/v1/audio/voices").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(voices.as_array().unwrap().len(), 4);
    assert!(
        voices
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "Kerstin")
    );

    // Generating queues a rendering; the clip is pending until then.
    let (status, clip) = admin
        .post(
            "/api/v1/audio/clips/tts",
            json!({"text": "  Willkommen   bei ITSH  ", "language": "de", "voice": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{clip}");
    assert_eq!(clip["status"], "pending");
    assert_eq!(clip["text"], "Willkommen bei ITSH");
    assert_eq!(
        clip["speech_speed"], 1.0,
        "old requests retain normal speed"
    );
    let tts_id = clip["id"].as_str().unwrap().to_owned();
    let jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'tts.clip' AND payload->>'clip_id' = $1",
    )
    .bind(&tts_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(jobs, 1);
    let (status, _) = admin
        .get(&format!("/api/v1/audio/clips/{tts_id}/audio"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "not rendered yet");
    for bad in [
        json!({"text": "   "}),
        json!({"text": "x".repeat(1001)}),
        json!({"text": "Hallo", "language": "fr"}),
        json!({"text": "Hallo", "voice": 3}),
        json!({"text": "Hallo", "speech_speed": 0.4}),
        json!({"text": "Hallo", "speech_speed": 2.1}),
        json!({"text": "<speed:0.7>Hallo"}),
        json!({"text": "<speed:1><speed:1>Hallo</speed></speed>"}),
    ] {
        let (status, _) = admin.post("/api/v1/audio/clips/tts", bad).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    // The worker's result.
    let tts_uuid: Uuid = tts_id.parse().unwrap();
    talkops_core::audio::set_rendered(&db, tts_uuid, Some(1500))
        .await
        .unwrap();
    let (_, clip) = admin.get(&format!("/api/v1/audio/clips/{tts_id}")).await;
    assert_eq!(clip["status"], "ready");
    assert_eq!(clip["duration_ms"], 1500);

    // Uploads and recordings arrive as WAV.
    let (status, _) = upload(&router, &admin, "upload", b"RIFF nonsense").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = upload(&router, &admin, "tts", &wav(1600)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "invalid source");
    let (status, rec) = upload(&router, &admin, "recording", &wav(8000)).await;
    assert_eq!(status, StatusCode::OK, "{rec}");
    assert_eq!(rec["status"], "ready");
    assert_eq!(rec["duration_ms"], 500);
    let rec_id = rec["id"].as_str().unwrap().to_owned();
    let res = raw(
        &router,
        Request::get(format!("/api/v1/audio/clips/{rec_id}/audio"))
            .header("cookie", &admin.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "audio/wav");
    let audits: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE entity_type = 'audio_clip'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(audits, 2);

    // Greetings reference clips.
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "300", "display_name": "Anna"}),
        )
        .await;
    let ext_id = ext["id"].as_str().unwrap();
    let vm = format!("/api/v1/extensions/{ext_id}/voicemail");
    let (status, _) = admin
        .put(
            &vm,
            json!({"enabled": true, "pin": "2468", "greeting": "clip"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "clip missing");
    let (status, _) = admin
        .put(
            &vm,
            json!({"enabled": true, "pin": "2468", "greeting": "clip",
                   "greeting_clip_id": Uuid::new_v4()}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unknown clip");
    let (status, b) = admin
        .put(
            &vm,
            json!({"enabled": true, "pin": "2468", "greeting": "clip", "greeting_clip_id": rec_id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    assert_eq!(b["greeting"], "clip");
    assert_eq!(b["greeting_clip_id"], rec_id.as_str());
    let (status, b) = admin
        .put(&vm, json!({"enabled": true, "greeting": "none"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    assert!(b["greeting_clip_id"].is_null());

    let (status, menu) = admin
        .post(
            "/api/v1/attendants",
            json!({"name": "Main", "flow": {"type": "play", "id": "start", "clip_id": tts_id}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{menu}");
    let (status, _) = admin
        .post(
            "/api/v1/attendants",
            json!({"name": "Bad", "flow": {"type": "play", "id": "start",
                   "clip_id": Uuid::new_v4()}}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unknown clip");

    // Cleanup: unreferenced clips older than a day go, with their files.
    sqlx::query("UPDATE audio_clips SET created_at = now() - interval '2 days'")
        .execute(&db)
        .await
        .unwrap();
    let rec_file = media.sounds.join(talkops_core::audio::clip_file(
        talkops_core::tenant::TenantId::DEFAULT,
        rec_id.parse().unwrap(),
    ));
    assert!(rec_file.is_file());
    let purged = talkops_api::retention::purge_clips(&db, &media.sounds)
        .await
        .unwrap();
    assert_eq!(purged, 1, "the recording is no longer referenced");
    assert!(!rec_file.exists());
    let (status, _) = admin.get(&format!("/api/v1/audio/clips/{tts_id}")).await;
    assert_eq!(status, StatusCode::OK, "the menu still uses it");
}
