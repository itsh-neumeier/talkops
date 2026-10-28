//! The service with the mock backend: HTTP interface, realtime sessions,
//! overload, device failure and the model library. No GPU is needed.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use talkops_cuda_stt::engine::{Engine, Settings};
use talkops_cuda_stt::http::{AppState, Library, router};
use talkops_cuda_stt::library::{Limits, ModelCheck};
use talkops_cuda_stt::mock::{self, MockBackend};
use talkops_cuda_stt::models::Catalog;
use talkops_stt_service::http::ModelAdmin;
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
    let mut w = hound_writer(&mut bytes, rate);
    for s in samples {
        w.write_sample((s * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
    bytes
}

fn hound_writer(bytes: &mut Vec<u8>, rate: u32) -> hound::WavWriter<std::io::Cursor<&mut Vec<u8>>> {
    hound::WavWriter::new(
        std::io::Cursor::new(bytes),
        hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap()
}

struct Setup {
    engine: Arc<Engine>,
    fail: Arc<AtomicBool>,
}

async fn start(catalog: Catalog, delay_ms: u64, queue: usize, sessions: usize) -> Setup {
    let backend = MockBackend::new(Duration::from_millis(delay_ms));
    let fail = backend.fail.clone();
    let engine = Engine::start(
        Arc::new(backend),
        catalog,
        None,
        Settings {
            queue,
            sessions,
            batch: 4,
            partial_interval: Duration::from_millis(500),
            max_turn: Duration::from_secs(15),
        },
    );
    assert!(engine.wait_ready(Duration::from_secs(5)).await);
    Setup { engine, fail }
}

fn app(engine: &Arc<Engine>, key: Option<&str>, admin: Option<ModelAdmin>) -> axum::Router {
    router(
        AppState {
            engine: engine.clone(),
            api_key: key.map(str::to_owned),
            stable_preview: false,
            admin,
        },
        Library {
            limits: Limits::default(),
            check: ModelCheck::Files,
            client: None,
        },
    )
}

fn multipart(fields: &[(&str, &str)], file: (&str, &[u8])) -> (String, Vec<u8>) {
    let boundary = "cuda-test-boundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .bytes(),
        );
    }
    body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"f\"\r\nContent-Type: application/octet-stream\r\n\r\n", file.0).bytes());
    body.extend_from_slice(file.1);
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
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let (ctype, body) = multipart(fields, ("file", file));
    let req = Request::post("/v1/audio/transcriptions")
        .header("content-type", ctype)
        .body(Body::from(body))
        .unwrap();
    let (status, headers, body) = call(app, req).await;
    (
        status,
        headers,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn health_and_models() {
    let s = start(mock::catalog(), 0, 16, 4).await;
    let (status, _, body) = call(
        app(&s.engine, None, None),
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let h: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(h["status"], "ok");
    assert_eq!(h["backend"], "mock");
    assert_eq!(h["device"]["models"].as_array().unwrap().len(), 3);
    assert_eq!(h["queue"]["max_sessions"], 4);
    let (status, _, body) = call(
        app(&s.engine, None, None),
        Request::get("/v1/models").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<String> = serde_json::from_slice::<Value>(&body).unwrap()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["cuda-de", "cuda-en", "cuda-auto"]);
    s.engine.shutdown();
}

#[tokio::test]
async fn no_model_means_unavailable() {
    let s = start(Catalog::default(), 0, 16, 4).await;
    let (status, headers, body) = call(
        app(&s.engine, None, None),
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(headers.get("retry-after").is_none());
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["status"],
        "unavailable"
    );
    let audio = wav(&tone(200.0, 1.0, RATE), RATE);
    let (status, _, body) = transcribe(
        app(&s.engine, None, None),
        &[("model", "cuda-auto")],
        &audio,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    s.engine.shutdown();
}

#[tokio::test]
async fn file_transcription_detects_the_language() {
    let s = start(mock::catalog(), 0, 16, 4).await;
    let mut samples = tone(200.0, 1.0, RATE);
    samples.extend(silence(1.5, RATE));
    samples.extend(tone(3000.0, 1.5, RATE));
    let audio = wav(&samples, RATE);
    let (status, _, v) = transcribe(
        app(&s.engine, None, None),
        &[
            ("model", "cuda-auto"),
            ("response_format", "verbose_json"),
            ("timestamp_granularities[]", "word"),
        ],
        &audio,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let segments = v["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 2, "{v}");
    assert_eq!(segments[0]["language"], "de");
    assert_eq!(segments[0]["text"], "hallo hallo");
    assert_eq!(segments[1]["language"], "en");
    assert!(
        segments[1]["text"]
            .as_str()
            .unwrap()
            .starts_with("hello hello hello"),
        "{v}"
    );
    assert_eq!(v["language"], "en", "the language with the most speech");
    let words = v["text"].as_str().unwrap().split(' ').count();
    assert_eq!(v["words"].as_array().unwrap().len(), words);
    // A fixed model, 8 kHz input.
    let audio = wav(&tone(200.0, 1.0, 8000), 8000);
    let (status, _, v) =
        transcribe(app(&s.engine, None, None), &[("model", "cuda-en")], &audio).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["text"], "hello hello");
    assert_eq!(v["language"], "en");
    let (status, _, v) =
        transcribe(app(&s.engine, None, None), &[("model", "cuda-fr")], &audio).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");
    s.engine.shutdown();
}

#[tokio::test]
async fn api_key_is_required_when_set() {
    let s = start(mock::catalog(), 0, 16, 4).await;
    let audio = wav(&tone(200.0, 1.0, RATE), RATE);
    let (status, _, _) = transcribe(
        app(&s.engine, Some("k"), None),
        &[("model", "cuda-de")],
        &audio,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = call(
        app(&s.engine, Some("k"), None),
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "health stays open for the container check"
    );
    s.engine.shutdown();
}

#[tokio::test]
async fn overload_and_device_failure() {
    let s = start(mock::catalog(), 300, 1, 4).await;
    let audio = wav(&tone(200.0, 1.0, RATE), RATE);
    let a = app(&s.engine, None, None);
    let requests = (0..4).map(|_| transcribe(a.clone(), &[("model", "cuda-de")], &audio));
    let results = futures_util::future::join_all(requests).await;
    let overloaded: Vec<_> = results
        .iter()
        .filter(|(status, _, _)| *status == StatusCode::SERVICE_UNAVAILABLE)
        .collect();
    assert!(
        !overloaded.is_empty(),
        "a queue of 1 rejects parallel files"
    );
    assert_eq!(overloaded[0].1["retry-after"], "5");
    assert_eq!(overloaded[0].2["error"]["code"], "overloaded");
    s.fail.store(true, Ordering::Relaxed);
    let (status, _, v) = transcribe(a.clone(), &[("model", "cuda-de")], &audio).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert_eq!(v["error"]["code"], "inference_failed");
    let (status, _, _) = call(a, Request::get("/health").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    s.engine.shutdown();
}

async fn serve(app: axum::Router) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn pcm16(samples: &[f32]) -> String {
    use base64::Engine as _;
    let bytes: Vec<u8> = samples
        .iter()
        .flat_map(|s| ((s * 32767.0) as i16).to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tokio::test]
async fn realtime_sends_partial_and_final_text() {
    let s = start(mock::catalog(), 0, 16, 1).await;
    let addr = serve(app(&s.engine, None, None)).await;
    let (mut ws, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/v1/realtime?intent=transcription"))
            .await
            .unwrap();
    let mut next = async || -> Value {
        loop {
            match ws.next().await.unwrap().unwrap() {
                Message::Text(t) => return serde_json::from_str(&t).unwrap(),
                _ => continue,
            }
        }
    };
    assert_eq!(next().await["type"], "session.created");
    // A second session exceeds the limit of one.
    let second =
        tokio_tungstenite::connect_async(format!("ws://{addr}/v1/realtime?intent=transcription"))
            .await;
    assert!(second.is_err(), "the session limit rejects the upgrade");
    drop(next);
    let send = |v: Value| Message::Text(v.to_string().into());
    ws.send(send(
        json!({"type": "session.update", "session": {"audio": {"input": {
        "format": {"type": "audio/pcm", "rate": 8000},
        "transcription": {"model": "cuda-auto", "languages": ["de", "en"]},
        "turn_detection": null}}}}),
    ))
    .await
    .unwrap();
    let mut events = Vec::new();
    let audio = tone(200.0, 2.5, 8000);
    for chunk in audio.chunks(160) {
        ws.send(send(
            json!({"type": "input_audio_buffer.append", "audio": pcm16(chunk)}),
        ))
        .await
        .unwrap();
    }
    ws.send(send(json!({"type": "input_audio_buffer.commit"})))
        .await
        .unwrap();
    while let Some(Ok(message)) = ws.next().await {
        let Message::Text(t) = message else { continue };
        let v: Value = serde_json::from_str(&t).unwrap();
        let done = v["type"] == "conversation.item.input_audio_transcription.completed";
        events.push(v);
        if done {
            break;
        }
    }
    let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert!(types.contains(&"session.updated"), "{types:?}");
    let deltas: String = events
        .iter()
        .filter(|e| e["type"] == "conversation.item.input_audio_transcription.delta")
        .map(|e| e["delta"].as_str().unwrap())
        .collect();
    assert!(
        deltas.starts_with("hallo"),
        "partial text before the commit: {types:?}"
    );
    let done = events.last().unwrap();
    assert_eq!(done["transcript"], "hallo hallo hallo hallo hallo");
    assert_eq!(done["language"], "de");
    s.engine.shutdown();
}

/// A tar.gz archive with the given files under one top directory.
fn archive(path: &Path, files: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    for (name, data) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, format!("model/{name}"), *data)
            .unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap();
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cuda-service-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn model_library_through_the_admin_interface() {
    let dir = temp("admin");
    let s = start(mock::catalog(), 0, 16, 4).await;
    let (restart, restarted) = tokio::sync::watch::channel(false);
    let admin = ModelAdmin {
        dir: dir.clone(),
        restart: Arc::new(restart),
    };
    let a = app(&s.engine, Some("k"), Some(admin));
    let auth = |r: axum::http::request::Builder| r.header("authorization", "Bearer k");

    let file = dir.join("upload.tar.gz");
    archive(
        &file,
        &[
            ("model.onnx", b"\x08\x07onnx"),
            ("tokens.txt", b"a 0\n"),
            ("test.wav", b"RIFF"),
        ],
    );
    let fields = [
        ("name", "Test CTC"),
        ("family", "nemo_ctc"),
        ("languages", "de"),
        ("license", "CC-BY-4.0"),
        ("source", "https://example.org"),
        ("slot", "de"),
    ];
    let (ctype, body) = multipart(&fields, ("archive", &std::fs::read(&file).unwrap()));
    let req = Request::post("/v1/admin/library").header("content-type", &ctype);
    let (status, _, _) = call(a.clone(), req.body(Body::from(body.clone())).unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let req = auth(Request::post("/v1/admin/library").header("content-type", &ctype));
    let (status, _, out) = call(a.clone(), req.body(Body::from(body)).unwrap()).await;
    let model: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(status, StatusCode::OK, "{model}");
    let id = model["id"].as_str().unwrap().to_owned();
    assert_eq!(model["slots"], json!(["de"]));
    assert_eq!(model["manifest"]["license"], "CC-BY-4.0");

    let (status, _, out) = call(
        a.clone(),
        auth(Request::get("/v1/admin/models"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let overview: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(overview["backend"], "mock");
    assert_eq!(overview["library"][0]["id"], id.as_str());
    assert_eq!(overview["slots"][0]["model_id"], id.as_str());
    assert_eq!(
        overview["slots"][0]["active"], false,
        "active after a restart"
    );
    assert!(overview["presets"]["models"].as_array().unwrap().len() >= 3);

    // A model that a slot selects cannot be deleted.
    let req = auth(Request::delete(format!("/v1/admin/library/{id}")));
    let (status, _, _) = call(a.clone(), req.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // English is not stated for this model.
    let req = auth(Request::put("/v1/admin/slots/en").header("content-type", "application/json"));
    let (status, _, _) = call(
        a.clone(),
        req.body(Body::from(json!({"id": id}).to_string())).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let req = auth(Request::delete("/v1/admin/slots/de"));
    let (status, _, _) = call(a.clone(), req.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let req = auth(Request::delete(format!("/v1/admin/library/{id}")));
    let (status, _, _) = call(a.clone(), req.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Downloads need an HTTPS URL and the digest.
    let req = auth(
        Request::post("/v1/admin/library/download").header("content-type", "application/json"),
    );
    let input = json!({"url": "http://example.org/a.tar.bz2", "sha256": "00", "name": "x", "family": "whisper",
        "languages": "de", "license": "MIT", "source": "x"});
    let (status, _, _) = call(a.clone(), req.body(Body::from(input.to_string())).unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let req = auth(Request::post("/v1/admin/restart"));
    let (status, _, _) = call(a.clone(), req.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(*restarted.borrow());
    s.engine.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn admin_is_disabled_without_a_models_directory() {
    let s = start(mock::catalog(), 0, 16, 4).await;
    let (status, _, body) = call(
        app(&s.engine, None, None),
        Request::get("/v1/admin/models")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(String::from_utf8_lossy(&body).contains("admin_disabled"));
    s.engine.shutdown();
}

#[tokio::test]
async fn a_broken_selected_model_degrades_the_service() {
    let mut catalog = mock::catalog();
    catalog.slots.get_mut("en").unwrap().manifest.name = "broken-en".into();
    let s = start(catalog, 0, 16, 4).await;
    let (status, _, body) = call(
        app(&s.engine, None, None),
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let h: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(h["status"], "degraded");
    assert_eq!(h["models"], json!(["cuda-de", "cuda-auto"]));
    s.engine.shutdown();
}
