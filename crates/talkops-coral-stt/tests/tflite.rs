//! The TensorFlow Lite backend with real models. Not in CI: it needs
//! `libtensorflowlite_c.so` and a models directory (see `tools/coral/`).
//!
//! ```sh
//! TALKOPS_TEST_TFLITE_LIB=/path/libtensorflowlite_c.so \
//! TALKOPS_TEST_CORAL_MODELS=/path/models \
//! cargo test -p talkops-coral-stt --test tflite -- --ignored
//! ```
//!
//! With `TALKOPS_TEST_EDGETPU=1` the test uses the Edge TPUs of the host
//! (`model_edgetpu.tflite`, libedgetpu.so.1) instead of the CPU.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use talkops_coral_stt::engine::Engine;
use talkops_coral_stt::http::transcribe_samples;
use talkops_coral_stt::models::Catalog;
use talkops_coral_stt::recognizer::Choice;
use talkops_coral_stt::tflite::TfliteBackend;

#[tokio::test]
#[ignore = "needs libtensorflowlite_c.so and models"]
async fn runs_real_models() {
    let lib = std::env::var("TALKOPS_TEST_TFLITE_LIB").expect("TALKOPS_TEST_TFLITE_LIB");
    let dir = PathBuf::from(
        std::env::var("TALKOPS_TEST_CORAL_MODELS").expect("TALKOPS_TEST_CORAL_MODELS"),
    );
    let tpu = std::env::var("TALKOPS_TEST_EDGETPU").is_ok();
    let file = if tpu {
        "model_edgetpu.tflite"
    } else {
        "model.tflite"
    };
    let catalog = Catalog::load(&dir, file).unwrap();
    assert!(!catalog.ctc.is_empty(), "no models in {}", dir.display());
    let backend = TfliteBackend {
        library: Some(lib.into()),
        delegate: tpu.then(|| "libedgetpu.so.1".into()),
        threads: 1,
        sysfs: "/sys/bus/usb/devices".into(),
        dev: "/dev".into(),
    };
    let engine = Engine::start(Arc::new(backend), catalog, 16, 4);
    assert!(
        engine.wait_ready(Duration::from_secs(60)).await,
        "{:?}",
        engine.devices()
    );
    // Ten seconds of a modulated tone: the models run; the text is not checked.
    let audio: Vec<f32> = (0..160_000)
        .map(|i| {
            let t = i as f32 / 16_000.0;
            0.3 * (2.0 * std::f32::consts::PI * 220.0 * t).sin() * (0.5 + 0.5 * (6.0 * t).sin())
        })
        .collect();
    let started = Instant::now();
    let languages = engine.catalog.languages();
    let (segments, _) = transcribe_samples(&engine, &audio, &Choice::Auto(languages))
        .await
        .unwrap();
    let secs = started.elapsed().as_secs_f32();
    let devices = engine.devices();
    println!(
        "segments {segments:?}\nreal-time factor {:.3}\ndevices {devices:?}",
        secs / 10.0
    );
    assert!(devices.iter().all(|d| d.state == "ready"));
    assert!(devices.iter().map(|d| d.inferences).sum::<u64>() >= 4);
    engine.shutdown();
}
