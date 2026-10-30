//! A real model with sherpa-onnx on the CPU (or the GPU with
//! `TALKOPS_CUDA_TEST_PROVIDER=cuda`). Not part of CI: it needs a model
//! archive and a WAV file.
//!
//! ```sh
//! TALKOPS_CUDA_TEST_ARCHIVE=sherpa-onnx-nemo-transducer-stt_de_fastconformer_hybrid_large_pc-int8.tar.bz2 \
//! TALKOPS_CUDA_TEST_FAMILY=nemo_transducer TALKOPS_CUDA_TEST_LANGUAGE=de \
//! TALKOPS_CUDA_TEST_WAV=de.wav cargo test -p talkops-cuda-stt --test sherpa -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use talkops_cuda_stt::engine::{Engine, Settings};
use talkops_cuda_stt::library::{self, Install, Limits, ModelCheck, Precision};
use talkops_cuda_stt::models::Catalog;
use talkops_cuda_stt::sherpa::SherpaBackend;
use talkops_stt_service::Choice;
use talkops_stt_service::audio::{self, MODEL_RATE};
use talkops_stt_service::http::transcribe_samples;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

#[tokio::test]
#[ignore = "needs a model archive and a WAV file"]
async fn real_model_transcribes_a_file() {
    let dir = std::env::temp_dir().join(format!("cuda-sherpa-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let language = env("TALKOPS_CUDA_TEST_LANGUAGE");
    let provider: &'static str = match std::env::var("TALKOPS_CUDA_TEST_PROVIDER").as_deref() {
        Ok("cuda") => "cuda",
        _ => "cpu",
    };
    let check = ModelCheck::Load {
        exe: PathBuf::from(env!("CARGO_BIN_EXE_talkops-cuda-stt")),
        provider: "cpu".into(),
        timeout: Duration::from_secs(300),
    };
    let started = Instant::now();
    let model = library::add_archive(
        &dir,
        &PathBuf::from(env("TALKOPS_CUDA_TEST_ARCHIVE")),
        Install {
            name: "test".into(),
            family: env("TALKOPS_CUDA_TEST_FAMILY"),
            languages: language.clone(),
            license: "test".into(),
            source: "test".into(),
            sha256: None,
            precision: match std::env::var("TALKOPS_CUDA_TEST_PRECISION").as_deref() {
                Ok("int8") => Precision::Int8,
                _ => Precision::Full,
            },
        },
        &Limits::default(),
        &check,
    )
    .expect("install and load check");
    println!(
        "installed {} in {:?}: {:?}",
        model.id,
        started.elapsed(),
        model.manifest.files
    );
    library::select(&dir, &language, &model.id).unwrap();
    let engine = Engine::start(
        Arc::new(SherpaBackend {
            provider,
            threads: 2,
        }),
        Catalog::load(&dir),
        None,
        Settings {
            queue: 8,
            sessions: 1,
            batch: 4,
            partial_interval: Duration::from_secs(1),
            max_turn: Duration::from_secs(15),
        },
    );
    assert!(engine.wait_ready(Duration::from_secs(300)).await);
    assert_eq!(
        engine.loaded_languages(),
        std::slice::from_ref(&language),
        "{:?}",
        engine.device()
    );
    let (samples, rate) =
        audio::decode_wav(&std::fs::read(env("TALKOPS_CUDA_TEST_WAV")).unwrap()).unwrap();
    let samples = audio::resample(&samples, rate, MODEL_RATE);
    let started = Instant::now();
    let (segments, words) = transcribe_samples(&engine, &samples, &Choice::Fixed(language))
        .await
        .unwrap();
    let secs = started.elapsed().as_secs_f32();
    let text: Vec<&str> = segments.iter().map(|s| s.text.as_str()).collect();
    println!(
        "{provider}: {:.1} s audio in {secs:.2} s: {text:?}; {} words with times",
        samples.len() as f32 / MODEL_RATE as f32,
        words.len()
    );
    assert!(!text.join(" ").trim().is_empty());
    engine.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}
