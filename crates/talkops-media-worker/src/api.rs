//! Transcription via an OpenAI-compatible API (`/audio/transcriptions`):
//! OpenAI, Groq, Mistral or a self-hosted server.
//!
//! Each channel of a recording is sent on its own (speaker labels as with
//! local Whisper), as 16 kHz mono WAV. Models with timestamps
//! (`verbose_json`, e.g. `whisper-1`) get the channel in chunks of up to
//! ten minutes (upload limits are around 25 MB); word timestamps are asked
//! for too, and give short lines in the correct order of both parties
//! ([crate::utterances]). Models without timestamps
//! (e.g. `gpt-4o-transcribe`) get each stretch of speech on its own, so the
//! conversation can still be put in order; servers that reject the OpenAI
//! options (`prompt`, `response_format`) get only file, model and language. Requests run through curl like
//! the model downloads; the key is passed on stdin, never on the command
//! line.
//!
//! A Coral or CUDA connection (ADR 0046, 0054) gets its `*-auto` model and
//! the tenant language as the preferred one; its answers name the detected
//! language of each segment, which the lines keep.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, bail};
use serde::Deserialize;
use talkops_core::recordings::Segment;
use talkops_core::transcription_api::ApiConfig;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::transcribe::{SAMPLE_RATE, initial_prompt, prepare, tempdir, to_lines, write_wav};
use crate::utterances::Word;

/// Longest chunk sent at once (10 min of 16 kHz 16-bit mono ≈ 19 MB).
const MAX_CHUNK_SECS: usize = 600;
/// Longest stretch of speech sent at once without timestamps.
const MAX_UTTERANCE_SECS: f32 = 30.0;
/// Per request.
const TIMEOUT_SECS: &str = "900";

/// The lines of a transcription and what the server reported.
#[derive(Debug)]
pub struct Transcription {
    pub segments: Vec<Segment>,
    /// Seconds of audio the provider accepted (for the usage estimate).
    pub seconds: f64,
    /// The language with the most speech (Coral only).
    pub language: Option<String>,
}

/// Transcribes a WAV file with the API. Stereo files are split into
/// `caller` and `called`, as with local Whisper.
pub async fn transcribe(
    cfg: &ApiConfig,
    wav: &Path,
    language: &str,
    vocabulary: &str,
) -> anyhow::Result<Transcription> {
    let work = tempdir()?;
    let channels = {
        let (wav, dir) = (wav.to_owned(), work.path.clone());
        tokio::task::spawn_blocking(move || prepare(&wav, &dir)).await??
    };
    let prompt = initial_prompt(language, vocabulary);
    let request = Request {
        cfg,
        language,
        prompt: &prompt,
        dir: &work.path,
        minimal: AtomicBool::new(false),
        no_words: AtomicBool::new(false),
        sent: AtomicUsize::new(0),
        languages: Mutex::new(Vec::new()),
    };
    let mut parties = Vec::new();
    for (i, (speaker, _, samples)) in channels.into_iter().enumerate() {
        let words = match request.timed(&samples, i).await? {
            Some(words) => words,
            // No timestamps from this model: send each stretch of speech.
            None => request.by_utterance(&samples, i).await?,
        };
        parties.push((speaker, words, samples));
    }
    let mut segments = to_lines(&parties);
    let mut language = None;
    if cfg.is_local() {
        let spans = request.languages.into_inner().unwrap_or_default();
        let speakers: Vec<&str> = parties.iter().map(|p| p.0).collect();
        label_languages(&mut segments, &speakers, &spans);
        language = main_language(&spans);
    }
    Ok(Transcription {
        segments,
        seconds: request.sent.load(Ordering::Relaxed) as f64 / f64::from(SAMPLE_RATE),
        language,
    })
}

/// Detected language of a time span of one channel: (channel, start, end, language).
type LanguageSpan = (usize, f32, f32, String);

/// `de`, `de-DE`, `german` -> `de` (OpenAI names languages in English).
pub fn normalize_language(language: &str) -> String {
    let l = language.trim().to_ascii_lowercase();
    match l.as_str() {
        "german" | "deutsch" => "de".into(),
        "english" => "en".into(),
        _ => l.split(['-', '_']).next().unwrap_or_default().to_owned(),
    }
}

/// Gives each line the language of the span of its channel that overlaps
/// it most.
fn label_languages(segments: &mut [Segment], speakers: &[&str], spans: &[LanguageSpan]) {
    for seg in segments.iter_mut() {
        let Some(channel) = speakers.iter().position(|s| *s == seg.speaker) else {
            continue;
        };
        seg.language = spans
            .iter()
            .filter(|s| s.0 == channel)
            .map(|s| {
                (
                    s.1.max(seg.start),
                    s.2.min(seg.end.max(seg.start + 0.01)),
                    &s.3,
                )
            })
            .filter(|(from, to, _)| to > from)
            .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
            .map(|(_, _, l)| l.clone());
    }
}

/// The language with the most speech.
fn main_language(spans: &[LanguageSpan]) -> Option<String> {
    let mut by: Vec<(&str, f32)> = Vec::new();
    for (_, start, end, lang) in spans {
        match by.iter_mut().find(|(l, _)| *l == lang) {
            Some(e) => e.1 += end - start,
            None => by.push((lang, end - start)),
        }
    }
    by.into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(l, _)| l.to_owned())
}

struct Request<'a> {
    cfg: &'a ApiConfig,
    language: &'a str,
    prompt: &'a str,
    dir: &'a Path,
    /// The server rejects `prompt`/`response_format`: send only the basics.
    minimal: AtomicBool,
    /// The server rejects word timestamps: ask for segments only.
    no_words: AtomicBool,
    /// Samples the server accepted.
    sent: AtomicUsize,
    /// Languages the server detected (Coral).
    languages: Mutex<Vec<LanguageSpan>>,
}

/// Which form fields a request carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// Segment and word timestamps (`verbose_json`), with the vocabulary prompt.
    Words,
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
    /// The detected language (Coral: `de`; OpenAI: `german`).
    #[serde(default)]
    language: Option<String>,
    segments: Option<Vec<ApiSegment>>,
    /// With `timestamp_granularities[]=word`; without punctuation.
    #[serde(default)]
    words: Vec<ApiWord>,
}

#[derive(Debug, Deserialize)]
struct ApiSegment {
    start: f32,
    end: f32,
    text: String,
    /// Coral names the language of each segment.
    #[serde(default)]
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiWord {
    start: f32,
    end: f32,
}

/// Words with times and punctuation: each segment's words get the times of
/// the timed words inside it. If the counts differ, the segment stays one
/// piece with its own times.
fn timed_words(segments: Vec<ApiSegment>, words: &[ApiWord], offset: f32) -> Vec<Word> {
    let mut out = Vec::new();
    for seg in segments {
        let texts: Vec<&str> = seg.text.split_whitespace().collect();
        let inside: Vec<&ApiWord> = words
            .iter()
            .filter(|w| (w.start + w.end) / 2.0 >= seg.start && (w.start + w.end) / 2.0 <= seg.end)
            .collect();
        if !texts.is_empty() && texts.len() == inside.len() {
            out.extend(texts.iter().zip(inside).map(|(text, w)| Word {
                start: offset + w.start,
                end: offset + w.end,
                text: (*text).to_owned(),
            }));
        } else {
            out.push(Word {
                start: offset + seg.start,
                end: offset + seg.end,
                text: seg.text.trim().to_owned(),
            });
        }
    }
    out
}

enum Outcome {
    Ok(Answer),
    /// The server rejected the request (HTTP 400/422), e.g. `verbose_json`.
    Rejected(String),
}

impl Request<'_> {
    /// Records the detected languages of one answer.
    fn note_languages(
        &self,
        channel: usize,
        offset: f32,
        samples: usize,
        segs: &[ApiSegment],
        language: Option<&str>,
    ) {
        let Ok(mut spans) = self.languages.lock() else {
            return;
        };
        let before = spans.len();
        for seg in segs {
            if let Some(lang) = &seg.language {
                spans.push((
                    channel,
                    offset + seg.start,
                    offset + seg.end,
                    normalize_language(lang),
                ));
            }
        }
        if spans.len() == before
            && let Some(lang) = language
        {
            let end = offset + samples as f32 / SAMPLE_RATE as f32;
            spans.push((channel, offset, end, normalize_language(lang)));
        }
    }

    /// With timestamps, in chunks. `None` if the model has none.
    async fn timed(&self, samples: &[f32], channel: usize) -> anyhow::Result<Option<Vec<Word>>> {
        let chunk = MAX_CHUNK_SECS * SAMPLE_RATE as usize;
        let mut out = Vec::new();
        for (n, part) in samples.chunks(chunk).enumerate() {
            let offset = (n * MAX_CHUNK_SECS) as f32;
            let file = self.dir.join(format!("api-{channel}-{n}.wav"));
            write_wav(&file, part)?;
            let mut answer = None;
            for form in [Form::Words, Form::Verbose] {
                // Once a server rejected word timestamps, skip them.
                if form == Form::Words && self.no_words.load(Ordering::Relaxed) {
                    continue;
                }
                match self.send(&file, form).await? {
                    Outcome::Ok(a) => {
                        answer = Some(a);
                        break;
                    }
                    Outcome::Rejected(_) if form == Form::Words => {
                        self.no_words.store(true, Ordering::Relaxed)
                    }
                    Outcome::Rejected(_) => return Ok(None),
                }
            }
            let Some(answer) = answer else {
                return Ok(None);
            };
            self.sent.fetch_add(part.len(), Ordering::Relaxed);
            let Some(segs) = answer.segments else {
                // Accepted, but without timestamps.
                return Ok(None);
            };
            self.note_languages(
                channel,
                offset,
                part.len(),
                &segs,
                answer.language.as_deref(),
            );
            out.extend(timed_words(segs, &answer.words, offset));
        }
        Ok(Some(out))
    }

    /// One request per stretch of speech; the stretch gives the time.
    async fn by_utterance(&self, samples: &[f32], channel: usize) -> anyhow::Result<Vec<Word>> {
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
            if let (Some(lang), Ok(mut spans)) = (&answer.language, self.languages.lock()) {
                spans.push((channel, start, end, normalize_language(lang)));
            }
            out.push(Word {
                start,
                end,
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
            Form::Words => {
                cmd.args(["--form-string", &format!("prompt={}", self.prompt)])
                    .args(["--form-string", "response_format=verbose_json"])
                    .args(["--form-string", "timestamp_granularities[]=segment"])
                    .args(["--form-string", "timestamp_granularities[]=word"]);
            }
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

    #[test]
    fn coral_languages_label_the_lines() {
        assert_eq!(normalize_language("German"), "de");
        assert_eq!(normalize_language("en-US"), "en");
        let line = |speaker: &str, start: f32, end: f32| Segment {
            start,
            end,
            speaker: speaker.into(),
            text: "x".into(),
            language: None,
        };
        let mut lines = vec![
            line("caller", 0.0, 2.0),
            line("called", 1.0, 3.0),
            line("caller", 5.0, 6.0),
        ];
        let spans = vec![
            (0, 0.0, 2.5, "de".to_owned()),
            (1, 0.5, 3.5, "en".to_owned()),
            (0, 4.8, 6.5, "en".to_owned()),
        ];
        label_languages(&mut lines, &["caller", "called"], &spans);
        let langs: Vec<Option<&str>> = lines.iter().map(|l| l.language.as_deref()).collect();
        assert_eq!(langs, [Some("de"), Some("en"), Some("en")]);
        assert_eq!(main_language(&spans).as_deref(), Some("en"));
        assert_eq!(main_language(&[]), None);
    }

    #[tokio::test]
    async fn cloud_line_timing_and_usage_survive_timestamp_fallback() {
        use tokio::io::AsyncReadExt;

        for reject_words in [false, true] {
            let work = tempdir().unwrap();
            let wav = work.path.join("stereo.wav");
            let mut writer = hound::WavWriter::create(
                &wav,
                hound::WavSpec {
                    channels: 2,
                    sample_rate: SAMPLE_RATE,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for sample in tone(4.0) {
                writer.write_sample((sample * 32767.0) as i16).unwrap();
                writer.write_sample((sample * 32767.0) as i16).unwrap();
            }
            writer.finalize().unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let cfg = ApiConfig {
                name: "test".into(),
                kind: "compatible".into(),
                url: format!("http://{}/v1", listener.local_addr().unwrap()),
                model: "whisper-1".into(),
                key: None,
            };
            let server = tokio::spawn(async move {
                let requests = if reject_words { 3 } else { 2 };
                for request in 0..requests {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut bytes = Vec::new();
                    let mut buffer = [0; 8192];
                    let (header_end, length) = loop {
                        let n = socket.read(&mut buffer).await.unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&buffer[..n]);
                        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&bytes[..end]);
                            let length = headers
                                .lines()
                                .find_map(|line| {
                                    let (name, value) = line.split_once(':')?;
                                    name.eq_ignore_ascii_case("content-length")
                                        .then(|| value.trim().parse::<usize>().unwrap())
                                })
                                .unwrap();
                            break (end + 4, length);
                        }
                    };
                    while bytes.len() < header_end + length {
                        let n = socket.read(&mut buffer).await.unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&buffer[..n]);
                    }
                    let form = String::from_utf8_lossy(&bytes[header_end..]);
                    let words = !reject_words || request == 0;
                    assert_eq!(form.contains("\r\n\r\nword\r\n"), words);
                    let rejected = reject_words && request == 0;
                    let caller = request == usize::from(reject_words);
                    let mut body = if rejected {
                        serde_json::json!({"error":{"message":"word timestamps unsupported"}})
                    } else if caller {
                        serde_json::json!({"segments":[
                            {"start":0.2,"end":0.7,"text":"Hallo."},
                            {"start":2.2,"end":2.8,"text":"Weiter."}],
                            "words":[{"start":0.2,"end":0.7},{"start":2.2,"end":2.8}]})
                    } else {
                        serde_json::json!({"segments":[{"start":1.4,"end":2.0,"text":"Ja."}],
                            "words":[{"start":1.4,"end":2.0}]})
                    };
                    if reject_words && !rejected {
                        body.as_object_mut().unwrap().remove("words");
                    }
                    let body = body.to_string();
                    let code = if rejected {
                        "400 Bad Request"
                    } else {
                        "200 OK"
                    };
                    let response = format!(
                        "HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(response.as_bytes()).await.unwrap();
                }
            });
            let done = tokio::time::timeout(
                std::time::Duration::from_secs(20),
                transcribe(&cfg, &wav, "de", ""),
            )
            .await
            .unwrap()
            .unwrap();
            let (lines, seconds) = (done.segments, done.seconds);
            assert_eq!(done.language, None, "only Coral reports a language");
            server.await.unwrap();
            assert_eq!(seconds, 8.0);
            assert_eq!(
                lines.iter().map(|s| s.speaker.as_str()).collect::<Vec<_>>(),
                ["caller", "called", "caller"]
            );
            assert_eq!(
                lines.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
                ["Hallo.", "Ja.", "Weiter."]
            );
            assert_eq!(
                lines.iter().map(|s| s.start).collect::<Vec<_>>(),
                [0.2, 1.4, 2.2]
            );
        }
    }

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

    #[test]
    fn word_times_get_the_segment_punctuation() {
        let a: Answer = serde_json::from_str(
            r#"{"text":"Hallo, Anna. Ja.","segments":[
                {"start":0.0,"end":2.0,"text":" Hallo, Anna."},
                {"start":2.0,"end":4.0,"text":" Ja so."}],
              "words":[{"word":"Hallo","start":0.25,"end":0.5},
                {"word":"Anna","start":0.75,"end":1.25},
                {"word":"Ja","start":3.0,"end":3.3}]}"#,
        )
        .unwrap();
        let w = timed_words(a.segments.unwrap(), &a.words, 600.0);
        let got: Vec<_> = w.iter().map(|w| (w.start, w.text.as_str())).collect();
        // The second segment has two words but one timed word: one piece.
        assert_eq!(
            got,
            [(600.25, "Hallo,"), (600.75, "Anna."), (602.0, "Ja so.")]
        );
    }
}
