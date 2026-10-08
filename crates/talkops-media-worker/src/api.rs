//! Transcription via an OpenAI-compatible API (`/audio/transcriptions`):
//! OpenAI, Groq, Mistral or a self-hosted server.
//!
//! Each channel of a recording is sent on its own (speaker labels as with
//! local Whisper), as 16 kHz mono WAV. Models with segment timestamps
//! (`verbose_json`, e.g. `whisper-1`) get the channel in chunks of up to
//! ten minutes (upload limits are around 25 MB). Models without timestamps
//! (e.g. `gpt-4o-transcribe`) get each stretch of speech on its own, so the
//! conversation can still be put in order; servers that reject the OpenAI
//! options (`prompt`, `response_format`) get only file, model and language. Requests run through curl like
//! the model downloads; the key is passed on stdin, never on the command
//! line.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, bail};
use serde::Deserialize;
use talkops_core::recordings::Segment;
use talkops_core::transcription_api::ApiConfig;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::transcribe::{SAMPLE_RATE, initial_prompt, prepare, tempdir, write_wav};

/// Longest chunk sent at once (10 min of 16 kHz 16-bit mono ≈ 19 MB).
const MAX_CHUNK_SECS: usize = 600;
/// Longest stretch of speech sent at once without timestamps.
const MAX_UTTERANCE_SECS: f32 = 30.0;
/// Per request.
const TIMEOUT_SECS: &str = "900";

/// Transcribes a WAV file with the API. Stereo files are split into
/// `caller` and `called`, as with local Whisper.
pub async fn transcribe(
    cfg: &ApiConfig,
    wav: &Path,
    language: &str,
    vocabulary: &str,
) -> anyhow::Result<Vec<Segment>> {
    let work = tempdir()?;
    let channels = {
        let (wav, dir) = (wav.to_owned(), work.path.clone());
        tokio::task::spawn_blocking(move || prepare(&wav, &dir)).await??
    };
    let prompt = initial_prompt(language, vocabulary);
    let mut segments = Vec::new();
    for (i, (speaker, _, samples)) in channels.into_iter().enumerate() {
        let request = Request {
            cfg,
            language,
            prompt: &prompt,
            dir: &work.path,
            minimal: AtomicBool::new(false),
        };
        let mut parsed = match request.timed(&samples, i).await? {
            Some(segs) => segs,
            // No timestamps from this model: send each stretch of speech.
            None => request.by_utterance(&samples, i).await?,
        };
        for seg in &mut parsed {
            seg.speaker = speaker.to_owned();
        }
        segments.extend(parsed.into_iter().filter(|s| !s.text.trim().is_empty()));
    }
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(segments)
}

struct Request<'a> {
    cfg: &'a ApiConfig,
    language: &'a str,
    prompt: &'a str,
    dir: &'a Path,
    /// The server rejects `prompt`/`response_format`: send only the basics.
    minimal: AtomicBool,
}

/// Which form fields a request carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// Segment timestamps (`verbose_json`), with the vocabulary prompt.
    Verbose,
    /// Plain text (`json`), with the vocabulary prompt.
    Json,
    /// Only file, model and language, for servers that know no more.
    Minimal,
}

/// Answer of `/audio/transcriptions` (`json` or `verbose_json`).
#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(default)]
    text: String,
    segments: Option<Vec<ApiSegment>>,
}

#[derive(Debug, Deserialize)]
struct ApiSegment {
    start: f32,
    end: f32,
    text: String,
}

enum Outcome {
    Ok(Answer),
    /// The server rejected the request (HTTP 400/422), e.g. `verbose_json`.
    Rejected(String),
}

impl Request<'_> {
    /// With segment timestamps, in chunks. `None` if the model has none.
    async fn timed(&self, samples: &[f32], channel: usize) -> anyhow::Result<Option<Vec<Segment>>> {
        let chunk = MAX_CHUNK_SECS * SAMPLE_RATE as usize;
        let mut out = Vec::new();
        for (n, part) in samples.chunks(chunk).enumerate() {
            let offset = (n * MAX_CHUNK_SECS) as f32;
            let file = self.dir.join(format!("api-{channel}-{n}.wav"));
            write_wav(&file, part)?;
            let answer = match self.send(&file, Form::Verbose).await? {
                Outcome::Ok(a) => a,
                Outcome::Rejected(_) => return Ok(None),
            };
            let Some(segs) = answer.segments else {
                // Accepted, but without timestamps.
                return Ok(None);
            };
            out.extend(segs.into_iter().map(|s| Segment {
                start: offset + s.start,
                end: offset + s.end,
                speaker: String::new(),
                text: s.text.trim().to_owned(),
            }));
        }
        Ok(Some(out))
    }

    /// One request per stretch of speech; the stretch gives the time.
    async fn by_utterance(&self, samples: &[f32], channel: usize) -> anyhow::Result<Vec<Segment>> {
        let mut out = Vec::new();
        for (n, (start, end)) in speech_regions(samples).into_iter().enumerate() {
            let from = (start * SAMPLE_RATE as f32) as usize;
            let to = ((end * SAMPLE_RATE as f32) as usize).min(samples.len());
            let file = self.dir.join(format!("utt-{channel}-{n}.wav"));
            write_wav(&file, &samples[from..to])?;
            let mut answer = None;
            for form in [Form::Json, Form::Minimal] {
                // Once a server rejected the OpenAI options, skip them.
                if form == Form::Json && self.minimal.load(Ordering::Relaxed) {
                    continue;
                }
                match self.send(&file, form).await? {
                    Outcome::Ok(a) => {
                        answer = Some(a);
                        break;
                    }
                    Outcome::Rejected(msg) if form == Form::Minimal => {
                        bail!("transcription API rejected the request: {msg}")
                    }
                    Outcome::Rejected(_) => self.minimal.store(true, Ordering::Relaxed),
                }
            }
            let answer = answer.context("no answer from transcription API")?;
            out.push(Segment {
                start,
                end,
                speaker: String::new(),
                text: answer.text.trim().to_owned(),
            });
        }
        Ok(out)
    }

    async fn send(&self, file: &Path, form: Form) -> anyhow::Result<Outcome> {
        let body = file.with_extension("json");
        let mut cmd = Command::new("curl");
        cmd.args([
            "-sS",
            "--max-time",
            TIMEOUT_SECS,
            "-w",
            "%{http_code}",
            "-o",
        ])
        .arg(&body)
        .args(["-K", "-", "-F"])
        .arg(format!("file=@{};type=audio/wav", file.display()))
        .args(["--form-string", &format!("model={}", self.cfg.model)])
        .args(["--form-string", &format!("language={}", self.language)]);
        match form {
            Form::Verbose => {
                cmd.args(["--form-string", &format!("prompt={}", self.prompt)])
                    .args(["--form-string", "response_format=verbose_json"])
                    .args(["--form-string", "timestamp_granularities[]=segment"]);
            }
            Form::Json => {
                cmd.args(["--form-string", &format!("prompt={}", self.prompt)])
                    .args(["--form-string", "response_format=json"]);
            }
            Form::Minimal => {}
        }
        cmd.arg(self.cfg.endpoint())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().context("run curl")?;
        // Key via curl's config on stdin: not visible in the process list.
        let config = match &self.cfg.key {
            Some(key) => format!("header = \"Authorization: Bearer {key}\"\n"),
            None => String::new(),
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(config.as_bytes()).await?;
        }
        let output = child.wait_with_output().await.context("run curl")?;
        if !output.status.success() {
            bail!(
                "transcription API not reachable ({}): {}",
                self.cfg.url,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let code: u16 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0);
        let text = tokio::fs::read(&body).await.unwrap_or_default();
        match code {
            200..=299 => Ok(Outcome::Ok(
                serde_json::from_slice(&text)
                    .context("unexpected answer from transcription API")?,
            )),
            400 | 422 => Ok(Outcome::Rejected(error_message(&text))),
            _ => bail!("transcription API: HTTP {code}: {}", error_message(&text)),
        }
    }
}

/// `{"error": {"message": …}}` (OpenAI style) or the start of the body.
fn error_message(body: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Err {
        error: serde_json::Value,
    }
    let msg = match serde_json::from_slice::<Err>(body) {
        Ok(Err { error }) => match error.get("message").and_then(|m| m.as_str()) {
            Some(m) => m.to_owned(),
            None => error.to_string(),
        },
        Err(_) => String::from_utf8_lossy(body).into_owned(),
    };
    msg.chars().take(300).collect()
}

/// Stretches of speech in seconds: loud 20 ms frames, merged across pauses
/// under 0.6 s, padded by 0.25 s and split at [MAX_UTTERANCE_SECS].
pub fn speech_regions(samples: &[f32]) -> Vec<(f32, f32)> {
    const FPS: f32 = 50.0;
    let frame = (SAMPLE_RATE / 50) as usize;
    let rms: Vec<f32> = samples
        .chunks(frame)
        .map(|f| (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt())
        .collect();
    let threshold = (rms.iter().copied().fold(0.0, f32::max) * 0.1).max(0.003);
    let total = samples.len() as f32 / SAMPLE_RATE as f32;
    let mut regions: Vec<(f32, f32)> = Vec::new();
    for (i, _) in rms.iter().enumerate().filter(|(_, r)| **r >= threshold) {
        let (start, end) = (i as f32 / FPS, (i + 1) as f32 / FPS);
        match regions.last_mut() {
            Some(last) if start - last.1 < 0.6 => last.1 = end,
            _ => regions.push((start, end)),
        }
    }
    let mut out = Vec::new();
    for (start, end) in regions {
        if end - start < 0.2 {
            continue; // a click
        }
        let (mut start, end) = ((start - 0.25).max(0.0), (end + 0.25).min(total));
        while end - start > MAX_UTTERANCE_SECS {
            out.push((start, start + MAX_UTTERANCE_SECS));
            start += MAX_UTTERANCE_SECS;
        }
        out.push((start, end));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(secs: f32) -> Vec<f32> {
        (0..(secs * SAMPLE_RATE as f32) as usize)
            .map(|i| (i as f32 * 0.1).sin() * 0.5)
            .collect()
    }

    fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (secs * SAMPLE_RATE as f32) as usize]
    }

    #[test]
    fn regions_follow_speech() {
        let mut s = silence(1.0);
        s.extend(tone(2.0));
        s.extend(silence(0.3)); // short pause: same utterance
        s.extend(tone(1.0));
        s.extend(silence(3.0));
        s.extend(tone(1.0));
        let r = speech_regions(&s);
        assert_eq!(r.len(), 2, "{r:?}");
        assert!(
            (r[0].0 - 0.75).abs() < 0.05 && (r[0].1 - 4.55).abs() < 0.05,
            "{r:?}"
        );
        assert!((r[1].0 - 7.05).abs() < 0.05, "{r:?}");
    }

    #[test]
    fn long_speech_is_split() {
        let r = speech_regions(&tone(70.0));
        assert_eq!(r.len(), 3, "{r:?}");
        assert!(r.iter().all(|(a, b)| b - a <= MAX_UTTERANCE_SECS + 0.01));
    }

    #[test]
    fn silence_has_no_regions() {
        assert!(speech_regions(&silence(5.0)).is_empty());
    }

    #[test]
    fn errors_are_readable() {
        assert_eq!(
            error_message(br#"{"error":{"message":"Invalid API key","type":"x"}}"#),
            "Invalid API key"
        );
        assert_eq!(error_message(b"Bad Gateway"), "Bad Gateway");
    }

    #[test]
    fn answers_parse() {
        let a: Answer = serde_json::from_str(
            r#"{"text":"Hallo","segments":[{"id":0,"start":0.0,"end":1.5,"text":" Hallo"}]}"#,
        )
        .unwrap();
        assert_eq!(a.segments.unwrap()[0].end, 1.5);
        let a: Answer = serde_json::from_str(r#"{"text":"Hallo"}"#).unwrap();
        assert!(a.segments.is_none());
    }
}
