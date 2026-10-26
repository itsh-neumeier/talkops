//! The service with the mock backend: HTTP interface, realtime sessions,
//! overload and device failure. No Edge TPU is needed.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use talkops_coral_stt::engine::Engine;
use talkops_coral_stt::http::{AppState, router};
use talkops_coral_stt::mock::{self, MockBackend};
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;

const RATE: u32 = 16_000;

/// German speech in the mock: a low tone; English: a high tone.
fn tone(hz: f32, secs: f32, rate: u32) -> Vec<f32> {
    (0..(secs * rate as f32) as usize)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin())
        .collect()
}

fn silence(secs: f32, rate: u32) -> Vec<f32> {
    vec![0.0; (secs * rate as f32) as usize]
}

fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut w = hound::WavWriter::new(
        std::io::Cursor::new(&mut bytes),
        hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for s in samples {
        w.write_sample((s * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
    bytes
}

async fn start(
    devices: usize,
    delay_ms: u64,
    queue: usize,
    sessions: usize,
) -> (Arc<Engine>, Arc<std::sync::atomic::AtomicBool>) {
    let backend = MockBackend::new(devices, Duration::from_millis(delay_ms));
    let fail = backend.fail.clone();
    let engine = Engine::start(Arc::new(backend), mock::catalog(true), queue, sessions);
    // Wait until every device has loaded its models.
    for _ in 0..250 {
        if engine.devices().len() == devices && engine.devices().iter().all(|d| d.state == "ready")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        engine
            .devices()
            .iter()
            .filter(|d| d.state == "ready")
            .count(),
        devices
    );
    (engine, fail)
}

fn app(engine: &Arc<Engine>, key: Option<&str>) -> axum::Router {
    router(AppState {
        engine: engine.clone(),
        api_key: key.map(str::to_owned),
        stable_preview: false,
    })
}

fn multipart(fields: &[(&str, &str)], file: &[u8]) -> (String, Vec<u8>) {
    let boundary = "coral-test-boundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .bytes(),
        );
    }
    body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.wav\"\r\nContent-Type: audio/wav\r\n\r\n").bytes());
    body.extend_from_slice(file);
    body.extend(format!("\r\n--{boundary}--\r\n").bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn call(
    app: axum::Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let res = app.oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, body)
}

async fn transcribe(
    app: axum::Router,
    fields: &[(&str, &str)],
    file: &[u8],
    key: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let (ctype, body) = multipart(fields, file);
    let mut req = Request::post("/v1/audio/transcriptions").header("content-type", ctype);
    if let Some(key) = key {
        req = req.header("authorization", format!("Bearer {key}"));
    }
    call(app, req.body(Body::from(body)).unwrap()).await
}

#[tokio::test]
async fn health_and_models() {
    let (engine, _) = start(2, 0, 16, 4).await;
    let (status, _, body) = call(
        app(&engine, None),
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let h: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(h["status"], "ok");
    assert_eq!(h["backend"], "mock");
    assert_eq!(h["tpus"].as_array().unwrap().len(), 2);
    assert_eq!(h["tpus"][0]["state"], "ready");
    assert_eq!(
        h["tpus"][0]["models"],
        json!(["coral-ctc-de", "coral-ctc-en", "langid"])
    );
    assert_eq!(h["queue"]["capacity"], 16);
    assert_eq!(h["queue"]["max_sessions"], 8);
    let (status, _, body) = call(
        app(&engine, None),
        Request::get("/v1/models").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let m: Value = serde_json::from_slice(&body).unwrap();
    let ids: Vec<&str> = m["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["coral-ctc-de", "coral-ctc-en", "coral-auto"]);
    // A key protects /v1 but not /health.
    let (status, _, _) = call(
        app(&engine, Some("k")),
        Request::get("/v1/models").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = call(
        app(&engine, Some("k")),
        Request::get("/v1/models")
            .header("authorization", "Bearer k")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        app(&engine, Some("k")),
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    engine.shutdown();
}

#[tokio::test]
async fn transcribes_files_with_language_per_segment() {
    let (engine, _) = start(1, 0, 16, 4).await;
    // German (3 s), a pause, English (3 s); 8 kHz telephone audio.
    let mut audio = silence(0.5, 8000);
    audio.extend(tone(200.0, 3.0, 8000));
    audio.extend(silence(1.5, 8000));
    audio.extend(tone(1500.0, 3.0, 8000));
    audio.extend(silence(1.0, 8000));
    let file = wav(&audio, 8000);
    let fields = [
        ("model", "coral-auto"),
        ("language", "de"),
        ("response_format", "verbose_json"),
        ("timestamp_granularities[]", "segment"),
        ("timestamp_granularities[]", "word"),
        ("prompt", "ignored"),
    ];
    let (status, _, body) = transcribe(app(&engine, None), &fields, &file, None).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let v: Value = serde_json::from_slice(&body).unwrap();
    let segments = v["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 2, "{v}");
    assert_eq!(segments[0]["language"], "de");
    assert!(
        segments[0]["text"].as_str().unwrap().contains("hallo"),
        "{v}"
    );
    assert_eq!(segments[1]["language"], "en");
    assert!(
        segments[1]["text"].as_str().unwrap().contains("hello"),
        "{v}"
    );
    let (s0, s1) = (
        segments[0]["start"].as_f64().unwrap(),
        segments[1]["start"].as_f64().unwrap(),
    );
    assert!(
        (s0 - 0.2).abs() < 0.05 && (s1 - 4.7).abs() < 0.05,
        "{s0} {s1}"
    );
    assert!((v["duration"].as_f64().unwrap() - 9.0).abs() < 0.01);
    let words = v["words"].as_array().unwrap();
    assert!(!words.is_empty());
    assert!(
        words
            .iter()
            .all(|w| w["end"].as_f64() >= w["start"].as_f64())
    );
    // Equal amounts of both languages: either may be the main language.
    assert!(v["language"] == "de" || v["language"] == "en");

    // A fixed model; json and text formats.
    let (status, _, body) = transcribe(
        app(&engine, None),
        &[("model", "coral-ctc-en"), ("response_format", "json")],
        &file,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["language"], "en");
    assert!(!v["text"].as_str().unwrap().contains("hallo"));
    let (status, _, body) = transcribe(
        app(&engine, None),
        &[("model", "coral-ctc-de"), ("response_format", "text")],
        &file,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8(body).unwrap().starts_with("hallo"));

    // Errors in the OpenAI format.
    let (status, _, body) =
        transcribe(app(&engine, None), &[("model", "whisper-1")], &file, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
        "model_not_found"
    );
    let (status, _, _) = transcribe(
        app(&engine, None),
        &[("model", "coral-auto"), ("response_format", "srt")],
        &file,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = transcribe(
        app(&engine, None),
        &[("model", "coral-auto")],
        b"not audio",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = transcribe(
        app(&engine, Some("k")),
        &[("model", "coral-auto")],
        &file,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = transcribe(
        app(&engine, Some("k")),
        &[("model", "coral-auto")],
        &file,
        Some("k"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    engine.shutdown();
}

#[tokio::test]
async fn rejects_work_when_overloaded_or_without_tpu() {
    // One slow device and room for one waiting job.
    let (engine, _) = start(1, 300, 1, 4).await;
    let file = wav(&tone(200.0, 2.0, RATE), RATE);
    let mut tasks = Vec::new();
    for _ in 0..4 {
        let app = app(&engine, None);
        let file = file.clone();
        tasks.push(tokio::spawn(async move {
            transcribe(app, &[("model", "coral-ctc-de")], &file, None).await
        }));
    }
    let mut overloaded = 0;
    for t in tasks {
        let (status, headers, body) = t.await.unwrap();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            overloaded += 1;
            assert_eq!(headers["retry-after"], "5");
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
                "overloaded"
            );
        } else {
            assert_eq!(status, StatusCode::OK);
        }
    }
    assert!(overloaded >= 1, "some requests must be rejected");
    engine.shutdown();

    let (none, _) = start(0, 0, 4, 4).await;
    let (status, _, body) = call(
        app(&none, None),
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["status"],
        "unavailable"
    );
    let (status, headers, body) =
        transcribe(app(&none, None), &[("model", "coral-auto")], &file, None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(headers.contains_key("retry-after"));
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
        "unavailable"
    );
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn serve(engine: &Arc<Engine>) -> u16 {
    serve_with_preview(engine, false).await
}

async fn serve_with_preview(engine: &Arc<Engine>, stable_preview: bool) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = router(AppState {
        engine: engine.clone(),
        api_key: None,
        stable_preview,
    });
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

async fn next_event(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .ok()??
        {
            Ok(Message::Text(t)) => return Some(serde_json::from_str(&t).unwrap()),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

/// PCM16 at 24 kHz, base64, in 200 ms chunks as the worker sends it.
async fn append(ws: &mut Ws, samples: &[f32]) {
    for chunk in samples.chunks(4800) {
        let bytes: Vec<u8> = chunk
            .iter()
            .flat_map(|s| ((s * 32767.0) as i16).to_le_bytes())
            .collect();
        let audio = base64::engine::general_purpose::STANDARD.encode(bytes);
        send(
            ws,
            json!({"type": "input_audio_buffer.append", "audio": audio}),
        )
        .await;
    }
}

/// Collects events until the completed transcript of the turn.
async fn until_completed(ws: &mut Ws) -> Vec<Value> {
    let mut events = Vec::new();
    while let Some(e) = next_event(ws).await {
        let done = e["type"] == "conversation.item.input_audio_transcription.completed"
            || e["type"] == "error";
        events.push(e);
        if done {
            break;
        }
    }
    events
}

#[tokio::test]
async fn realtime_sessions_like_the_worker_client() {
    let (engine, _) = start(1, 0, 16, 1).await;
    let port = serve(&engine).await;
    let url = format!("ws://127.0.0.1:{port}/v1/realtime?intent=transcription");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    assert_eq!(
        next_event(&mut ws).await.unwrap()["type"],
        "session.created"
    );
    send(
        &mut ws,
        json!({"type": "session.update", "session": {"type": "transcription", "audio": {"input": {
        "format": {"type": "audio/pcm", "rate": 24000},
        "transcription": {"model": "coral-auto", "languages": ["en", "de"]},
        "turn_detection": null}}}}),
    )
    .await;
    let updated = next_event(&mut ws).await.unwrap();
    assert_eq!(updated["type"], "session.updated");
    assert_eq!(
        updated["session"]["audio"]["input"]["transcription"]["languages"],
        json!(["en", "de"])
    );

    // A long German turn: deltas arrive before the commit.
    append(&mut ws, &tone(200.0, 6.0, 24_000)).await;
    send(&mut ws, json!({"type": "input_audio_buffer.commit"})).await;
    let events = until_completed(&mut ws).await;
    let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    let committed = types
        .iter()
        .position(|t| *t == "input_audio_buffer.committed")
        .unwrap();
    let first_delta = types
        .iter()
        .position(|t| t.ends_with(".delta"))
        .expect("deltas");
    assert!(first_delta < committed, "{types:?}");
    let done = events.last().unwrap();
    assert_eq!(
        done["type"],
        "conversation.item.input_audio_transcription.completed"
    );
    assert_eq!(done["language"], "de");
    assert_eq!(done["item_id"], events[committed]["item_id"]);
    assert!(
        done["transcript"].as_str().unwrap().starts_with("hallo"),
        "{done}"
    );
    let deltas: String = events
        .iter()
        .filter(|e| e["type"].as_str().unwrap().ends_with(".delta"))
        .map(|e| e["delta"].as_str().unwrap())
        .collect();
    assert!(
        done["transcript"]
            .as_str()
            .unwrap()
            .starts_with(deltas.trim()),
        "{deltas}"
    );

    // A short English turn, then a cleared click.
    append(&mut ws, &tone(1500.0, 1.5, 24_000)).await;
    send(&mut ws, json!({"type": "input_audio_buffer.commit"})).await;
    let done = until_completed(&mut ws).await.pop().unwrap();
    assert_eq!(done["language"], "en");
    assert_eq!(done["transcript"], "hello");
    append(&mut ws, &tone(200.0, 0.1, 24_000)).await;
    send(&mut ws, json!({"type": "input_audio_buffer.clear"})).await;
    assert_eq!(
        next_event(&mut ws).await.unwrap()["type"],
        "input_audio_buffer.cleared"
    );

    // One session per TPU in this test: a second one is refused with 503.
    match tokio_tungstenite::connect_async(&url).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(res)) => {
            assert_eq!(res.status(), 503);
            assert_eq!(res.headers()["retry-after"], "5");
        }
        other => panic!("expected HTTP 503, got {other:?}"),
    }
    ws.close(None).await.unwrap();
    // The slot is free again after the close.
    let mut free = false;
    for _ in 0..50 {
        if engine.queue_status().sessions == 0 {
            free = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(free);
    let (status, _, _) = call(
        app(&engine, None),
        Request::get("/v1/realtime").body(Body::empty()).unwrap(),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
    engine.shutdown();
}

#[tokio::test]
async fn stable_preview_is_opt_in_and_preserves_completed_text() {
    let (engine, _) = start(1, 0, 16, 4).await;
    let mut completed = Vec::new();
    for enabled in [false, true] {
        let port = serve_with_preview(&engine, enabled).await;
        let url = format!("ws://127.0.0.1:{port}/v1/realtime?intent=transcription");
        let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let created = next_event(&mut ws).await.unwrap();
        assert_eq!(
            created["session"]["audio"]["input"]["transcription"]["stable_preview"],
            enabled
        );
        send(
            &mut ws,
            json!({"type": "session.update", "session": {"audio": {"input": {
                "format": {"type": "audio/pcm", "rate": 24000},
                "transcription": {"model": "coral-ctc-de", "stable_preview": !enabled},
                "turn_detection": null
            }}}}),
        )
        .await;
        let updated = next_event(&mut ws).await.unwrap();
        // A client cannot change this operator-controlled service feature.
        assert_eq!(
            updated["session"]["audio"]["input"]["transcription"]["stable_preview"],
            enabled
        );
        append(&mut ws, &tone(200.0, 6.0, 24000)).await;
        let first = next_event(&mut ws).await.expect("a delta before commit");
        assert_eq!(
            first["type"],
            "conversation.item.input_audio_transcription.delta"
        );
        send(&mut ws, json!({"type": "input_audio_buffer.commit"})).await;
        let mut events = vec![first];
        events.extend(until_completed(&mut ws).await);
        let final_event = events.last().unwrap();
        assert_eq!(
            final_event["type"],
            "conversation.item.input_audio_transcription.completed"
        );
        let text = final_event["transcript"].as_str().unwrap();
        let prefix: String = events.iter().filter_map(|e| e["delta"].as_str()).collect();
        assert!(text.starts_with(&prefix), "{prefix:?} must prefix {text:?}");
        completed.push(text.to_owned());
        // Clear discards an unfinished turn. A short following turn still
        // completes, even when it is too short for any preview window.
        append(&mut ws, &tone(200.0, 0.1, 24000)).await;
        send(&mut ws, json!({"type": "input_audio_buffer.clear"})).await;
        assert_eq!(
            next_event(&mut ws).await.unwrap()["type"],
            "input_audio_buffer.cleared"
        );
        append(&mut ws, &tone(200.0, 1.5, 24000)).await;
        send(&mut ws, json!({"type": "input_audio_buffer.commit"})).await;
        assert_eq!(
            until_completed(&mut ws).await.last().unwrap()["transcript"],
            "hallo"
        );
        ws.close(None).await.unwrap();
    }
    assert_eq!(completed[0], completed[1]);
    engine.shutdown();
}

#[tokio::test]
async fn a_failing_tpu_ends_the_session_with_an_error() {
    let (engine, fail) = start(1, 0, 16, 4).await;
    let port = serve(&engine).await;
    let url = format!("ws://127.0.0.1:{port}/v1/realtime?intent=transcription");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    next_event(&mut ws).await.unwrap();
    fail.store(true, Ordering::Relaxed);
    append(&mut ws, &tone(200.0, 3.0, 24_000)).await;
    send(&mut ws, json!({"type": "input_audio_buffer.commit"})).await;
    let events = until_completed(&mut ws).await;
    let error = events.last().unwrap();
    assert_eq!(error["type"], "error", "{events:?}");
    assert!(
        ["inference_failed", "unavailable"].contains(&error["error"]["code"].as_str().unwrap())
    );
    assert!(next_event(&mut ws).await.is_none(), "the session closes");
    let devices = engine.devices();
    assert_eq!(devices[0].state, "error");
    assert!(
        devices[0]
            .error
            .as_deref()
            .unwrap()
            .contains("mock device failure")
    );
    // The supervisor restarts the device once it works again.
    fail.store(false, Ordering::Relaxed);
    engine.rescan();
    assert!(engine.wait_ready(Duration::from_secs(5)).await);
    engine.shutdown();
}
