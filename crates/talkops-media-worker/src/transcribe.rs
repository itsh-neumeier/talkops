//! Local transcription with whisper.cpp (`whisper-cli`).
//!
//! Call recordings are stereo (left: caller, right: called party); each
//! channel is transcribed on its own and the segments are merged by time,
//! which gives speaker labels without diarization. Audio is converted to
//! 16 kHz mono 16-bit PCM, the only input whisper.cpp accepts.
//!
//! Models are downloaded on first use into the models volume and verified
//! against pinned SHA-256 sums. Any other model must be placed there by the
//! administrator (`ggml-<name>.bin`).
//!
//! Voice activity detection (Silero VAD) passes only speech to Whisper: phone
//! recordings have long pauses and hold music, on which Whisper otherwise
//! invents text ("Untertitel im Auftrag des ZDF …").

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use talkops_core::recordings::Segment;
use talkops_core::settings::TranscriptionQuality;
use tokio::process::Command;

const SAMPLE_RATE: u32 = 16_000;
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Silero VAD model for whisper.cpp: file, SHA-256, size.
const VAD_MODEL: (&str, &str, u64) = (
    "ggml-silero-v5.1.2.bin",
    "29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf",
    885_098,
);
const VAD_URL: &str = "https://huggingface.co/ggml-org/whisper-vad/resolve/main";

/// Models that are downloaded automatically: name, SHA-256, size.
const KNOWN_MODELS: &[(&str, &str, u64)] = &[
    (
        "base",
        "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
        147_951_465,
    ),
    (
        "small",
        "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
        487_601_967,
    ),
    (
        "medium-q5_0",
        "19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f",
        539_212_467,
    ),
    (
        "medium",
        "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
        1_533_763_059,
    ),
    (
        "large-v3-turbo-q5_0",
        "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        574_041_195,
    ),
    (
        "large-v3-turbo-q8_0",
        "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
        874_188_075,
    ),
    (
        "large-v3-turbo",
        "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
        1_624_555_275,
    ),
    (
        "large-v3-q5_0",
        "d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1",
        1_081_140_203,
    ),
    (
        "large-v3",
        "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
        3_095_033_483,
    ),
];

/// Per-tenant choices for one transcription.
#[derive(Debug, Clone)]
pub struct Options {
    pub quality: TranscriptionQuality,
    /// Names and terms to recognise (comma-separated).
    pub vocabulary: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            quality: TranscriptionQuality::Fast,
            vocabulary: String::new(),
        }
    }
}

/// Context for Whisper: a well-punctuated sentence in the call's language
/// (it imitates the style) followed by the vocabulary.
fn initial_prompt(language: &str, vocabulary: &str) -> String {
    let base = match language {
        "de" => "Ein Telefongespräch auf Deutsch. Hallo, guten Tag!",
        _ => "A phone call in English. Hello, good morning!",
    };
    let words = vocabulary.trim();
    if words.is_empty() {
        base.to_owned()
    } else {
        let label = if language == "de" {
            "Begriffe"
        } else {
            "Terms"
        };
        format!("{base} {label}: {words}.")
    }
}

#[derive(Debug, Clone)]
pub struct Whisper {
    pub bin: PathBuf,
    pub models_dir: PathBuf,
    pub model: String,
    pub threads: usize,
    /// Transcribe only detected speech (Silero VAD).
    pub vad: bool,
}

impl Whisper {
    /// The model for a quality level; `fast` is the configured model.
    fn model_name(&self, quality: TranscriptionQuality) -> &str {
        match quality {
            TranscriptionQuality::Fast => &self.model,
            TranscriptionQuality::Accurate => "large-v3-q5_0",
            TranscriptionQuality::Best => "large-v3",
        }
    }

    /// Returns the model file, downloading a known model if it is missing.
    pub async fn ensure_model(&self, name: &str) -> anyhow::Result<PathBuf> {
        let file = self.models_dir.join(format!("ggml-{name}.bin"));
        if file.is_file() {
            return Ok(file);
        }
        let Some(&(_, sum, size)) = KNOWN_MODELS.iter().find(|(n, _, _)| *n == name) else {
            bail!(
                "whisper model {} not found; place it there or use one of: {}",
                file.display(),
                KNOWN_MODELS
                    .iter()
                    .map(|m| m.0)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        };
        let url = format!("{MODEL_URL}/ggml-{name}.bin");
        download(&url, &file, sum, size).await?;
        Ok(file)
    }

    /// The VAD model, downloaded on first use.
    async fn ensure_vad_model(&self) -> anyhow::Result<PathBuf> {
        let (name, sum, size) = VAD_MODEL;
        let file = self.models_dir.join(name);
        if !file.is_file() {
            download(&format!("{VAD_URL}/{name}"), &file, sum, size).await?;
        }
        Ok(file)
    }

    /// Arguments for whisper-cli besides input and output.
    async fn decode_args(
        &self,
        model: &Path,
        language: &str,
        opts: &Options,
    ) -> anyhow::Result<Vec<String>> {
        let mut args = vec![
            "--prompt".to_owned(),
            initial_prompt(language, &opts.vocabulary),
            "-m".to_owned(),
            model.to_string_lossy().into_owned(),
            "-l".to_owned(),
            language.to_owned(),
            "-t".to_owned(),
            self.threads.to_string(),
            // No "[Musik]", "(lacht)" and the like.
            "-sns".to_owned(),
        ];
        if opts.quality != TranscriptionQuality::Fast {
            // Wider beam search: slower, fewer wrong words.
            args.extend(["-bs", "8", "-bo", "8"].map(str::to_owned));
        }
        if self.vad {
            match self.ensure_vad_model().await {
                Ok(vad) => args.extend([
                    "--vad".to_owned(),
                    "-vm".to_owned(),
                    vad.to_string_lossy().into_owned(),
                    // Keep word beginnings and endings (default 30 ms cuts
                    // them off), and do not split at short pauses.
                    "-vp".to_owned(),
                    "200".to_owned(),
                    "-vsd".to_owned(),
                    "300".to_owned(),
                ]),
                // Without the VAD model, transcribe everything.
                Err(err) => tracing::warn!(error = %err, "VAD model unavailable"),
            }
        }
        Ok(args)
    }

    /// Transcribes a WAV file. Stereo files are split into `caller` and
    /// `called`; mono files get no speaker.
    pub async fn transcribe(
        &self,
        wav: &Path,
        language: &str,
        opts: &Options,
    ) -> anyhow::Result<Vec<Segment>> {
        let model = self.ensure_model(self.model_name(opts.quality)).await?;
        let args = self.decode_args(&model, language, opts).await?;
        let work = tempdir()?;
        let channels = {
            let (wav, work) = (wav.to_owned(), work.path.clone());
            tokio::task::spawn_blocking(move || prepare(&wav, &work)).await??
        };
        let mut segments = Vec::new();
        for (speaker, file, samples) in channels {
            let out = file.with_extension("");
            let output = Command::new(&self.bin)
                .args(&args)
                .arg("-f")
                .arg(&file)
                .args(["-np", "-oj", "-of"])
                .arg(&out)
                .output()
                .await
                .with_context(|| format!("run {}", self.bin.display()))?;
            if !output.status.success() {
                bail!(
                    "whisper failed ({}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                        .lines()
                        .last()
                        .unwrap_or_default()
                );
            }
            let json = tokio::fs::read(out.with_extension("json"))
                .await
                .context("whisper output")?;
            let mut parsed = parse_output(&json, speaker)?;
            refine_starts(&mut parsed, &samples);
            segments.extend(parsed);
        }
        segments.sort_by(|a, b| a.start.total_cmp(&b.start));
        Ok(segments)
    }
}

/// Downloads `url` to `file`, checking size and SHA-256.
async fn download(url: &str, file: &Path, sum: &str, size: u64) -> anyhow::Result<()> {
    if let Some(dir) = file.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = file.with_extension("part");
    tracing::info!(url, bytes = size, "downloading model");
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&tmp)
        .arg(url)
        .status()
        .await
        .context("run curl")?;
    if !status.success() {
        let _ = tokio::fs::remove_file(&tmp).await;
        bail!("downloading {url} failed ({status})");
    }
    let actual = {
        let tmp = tmp.clone();
        tokio::task::spawn_blocking(move || sha256_file(&tmp)).await??
    };
    if actual != sum {
        let _ = tokio::fs::remove_file(&tmp).await;
        bail!("checksum mismatch for {url}: {actual}");
    }
    tokio::fs::rename(&tmp, file).await?;
    tracing::info!(file = %file.display(), "model ready");
    Ok(())
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// A temporary directory removed on drop.
struct TempDir {
    path: PathBuf,
}

fn tempdir() -> anyhow::Result<TempDir> {
    let path = std::env::temp_dir().join(format!("talkops-whisper-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path)?;
    Ok(TempDir { path })
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Peak below which a channel counts as silent (no speech to transcribe).
const SILENCE_PEAK: f32 = 0.01;

/// Splits a WAV file into 16 kHz mono files: one per non-silent channel of a
/// stereo recording (`caller`, `called`), or one without speaker.
fn prepare(wav: &Path, work: &Path) -> anyhow::Result<Vec<(&'static str, PathBuf, Vec<f32>)>> {
    let mut reader =
        hound::WavReader::open(wav).with_context(|| format!("open {}", wav.display()))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|s| s as f32 / scale))
                .collect::<Result<_, _>>()?
        }
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
    };
    let n = spec.channels as usize;
    anyhow::ensure!(n > 0, "WAV without channels");
    let names: &[&'static str] = if n == 2 { &["caller", "called"] } else { &[""] };
    let mut out = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let channel: Vec<f32> = if names.len() == 1 {
            // Mono, or more channels than expected: mix down.
            samples
                .chunks(n)
                .map(|f| f.iter().sum::<f32>() / n as f32)
                .collect()
        } else {
            samples.iter().skip(i).step_by(n).copied().collect()
        };
        if channel.iter().all(|s| s.abs() < SILENCE_PEAK) {
            continue;
        }
        let file = work.join(format!("ch{i}.wav"));
        let mut resampled = resample(&channel, spec.sample_rate, SAMPLE_RATE);
        normalize(&mut resampled);
        write_wav(&file, &resampled)?;
        out.push((*name, file, resampled));
    }
    Ok(out)
}

/// Brings quiet telephone audio to a uniform level: the loudest parts (99.9th
/// percentile, ignoring clicks) to 0.9, at most 10× louder.
fn normalize(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    let mut mags: Vec<f32> = samples.iter().map(|s| s.abs()).collect();
    let k = (mags.len() as f64 * 0.999) as usize;
    let k = k.min(mags.len() - 1);
    let (_, peak, _) = mags.select_nth_unstable_by(k, f32::total_cmp);
    let peak = *peak;
    if peak <= f32::EPSILON {
        return;
    }
    let gain = (0.9 / peak).min(10.0);
    for s in samples.iter_mut() {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}

/// Whisper often starts a segment at the end of the previous one, i.e. at
/// the beginning of the silence before it. For a correct order of both
/// speakers, move each start to the first loud 20 ms frame of the segment.
fn refine_starts(segments: &mut [Segment], samples: &[f32]) {
    const FRAME: usize = (SAMPLE_RATE / 50) as usize;
    let rms: Vec<f32> = samples
        .chunks(FRAME)
        .map(|f| (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt())
        .collect();
    let threshold = (rms.iter().copied().fold(0.0, f32::max) * 0.1).max(0.003);
    for seg in segments {
        let first = (seg.start * 50.0) as usize;
        let last = ((seg.end * 50.0) as usize).min(rms.len());
        if let Some(i) = (first..last).find(|&i| rms[i] >= threshold) {
            seg.start = i as f32 / 50.0;
        }
    }
}

/// Linear resampling; when downsampling, a moving average first suppresses
/// most of what would alias (good enough for speech recognition).
fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let smoothed;
    let src = if ratio > 1.0 {
        let width = ratio.ceil() as usize;
        let mut acc = 0.0f32;
        smoothed = input
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                acc += s;
                if i >= width {
                    acc -= input[i - width];
                }
                acc / (i + 1).min(width) as f32
            })
            .collect::<Vec<_>>();
        &smoothed[..]
    } else {
        input
    };
    let len = ((input.len() as f64) / ratio).floor() as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;
            let a = src[idx.min(src.len() - 1)];
            let b = src[(idx + 1).min(src.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

fn write_wav(path: &Path, samples: &[f32]) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    w.finalize()?;
    Ok(())
}

#[derive(Deserialize)]
struct Output {
    transcription: Vec<OutSegment>,
}

#[derive(Deserialize)]
struct OutSegment {
    offsets: Offsets,
    text: String,
}

#[derive(Deserialize)]
struct Offsets {
    from: u64,
    to: u64,
}

/// Parses whisper-cli's `-oj` output; drops empty and non-speech segments
/// such as `[BLANK_AUDIO]` or `(music)`.
fn parse_output(json: &[u8], speaker: &str) -> anyhow::Result<Vec<Segment>> {
    let out: Output = serde_json::from_slice(json).context("parse whisper output")?;
    Ok(out
        .transcription
        .into_iter()
        .filter_map(|s| {
            let text = s.text.trim();
            let marker = (text.starts_with('[') && text.ends_with(']'))
                || (text.starts_with('(') && text.ends_with(')'));
            (!text.is_empty() && !marker).then(|| Segment {
                start: s.offsets.from as f32 / 1000.0,
                end: s.offsets.to as f32 / 1000.0,
                speaker: speaker.to_owned(),
                text: text.to_owned(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_and_normalizes() {
        assert_eq!(
            initial_prompt("de", " Neumeier, TalkOps "),
            "Ein Telefongespräch auf Deutsch. Hallo, guten Tag! Begriffe: Neumeier, TalkOps."
        );
        assert!(initial_prompt("en", "").starts_with("A phone call"));
        let mut quiet: Vec<f32> = (0..16000).map(|i| (i as f32 * 0.05).sin() * 0.2).collect();
        normalize(&mut quiet);
        let peak = quiet.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((0.85..=0.95).contains(&peak), "{peak}");
        let mut silence = vec![0.0f32; 100];
        normalize(&mut silence);
        assert!(silence.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn resamples_to_16k() {
        let tone: Vec<f32> = (0..8000).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let up = resample(&tone, 8000, 16000);
        assert_eq!(up.len(), 16000);
        assert!((up[2] - tone[1]).abs() < 1e-6);
        let down = resample(&vec![0.25; 48000], 48000, 16000);
        assert_eq!(down.len(), 16000);
        assert!(down[100..].iter().all(|s| (s - 0.25).abs() < 1e-5));
    }

    #[test]
    fn splits_stereo_and_skips_silent_channels() {
        let work = tempdir().unwrap();
        let wav = work.path.join("in.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&wav, spec).unwrap();
        for i in 0..8000 {
            w.write_sample(((i as f32 * 0.2).sin() * 8000.0) as i16)
                .unwrap();
            w.write_sample(0i16).unwrap();
        }
        w.finalize().unwrap();
        let out = prepare(&wav, &work.path).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "caller");
        assert_eq!(out[0].2.len(), 16000);
        let r = hound::WavReader::open(&out[0].1).unwrap();
        assert_eq!(r.spec().sample_rate, 16000);
        assert_eq!(r.spec().channels, 1);
        assert_eq!(r.len(), 16000);
    }

    #[test]
    fn segment_starts_skip_leading_silence() {
        let mut samples = vec![0.0; 16000];
        samples.extend((0..16000).map(|i| (i as f32 * 0.3).sin() * 0.5));
        let mut segs = vec![Segment {
            start: 0.0,
            end: 2.0,
            speaker: "called".into(),
            text: "x".into(),
        }];
        refine_starts(&mut segs, &samples);
        assert_eq!(segs[0].start, 1.0);
    }

    #[test]
    fn parses_whisper_json() {
        let json = br#"{"result":{"language":"de"},"transcription":[
            {"timestamps":{"from":"00:00:00,000","to":"00:00:02,660"},"offsets":{"from":0,"to":2660},"text":" Hallo, hier ist Anna."},
            {"timestamps":{"from":"00:00:02,660","to":"00:00:04,000"},"offsets":{"from":2660,"to":4000},"text":" [BLANK_AUDIO]"}
        ]}"#;
        let s = parse_output(json, "called").unwrap();
        assert_eq!(
            s,
            [Segment {
                start: 0.0,
                end: 2.66,
                speaker: "called".into(),
                text: "Hallo, hier ist Anna.".into()
            }]
        );
    }
}
