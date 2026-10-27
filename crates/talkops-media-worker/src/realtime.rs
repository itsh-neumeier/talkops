//! Live preview over a Realtime transcription session (ADR 0024).
//!
//! The worker follows the growing recording and streams each channel over
//! its own WebSocket (speaker labels as with Whisper). A small energy-based
//! detector finds turns: only speech is sent (with a short lead-in), and the
//! end of a turn commits it. The detector also gives the times, because
//! realtime models return no timestamps. Text deltas show as provisional
//! segments until the completed transcript replaces them.
//!
//! A stream runs in the background, not in the job lane, and ends when the
//! viewer leaves, the recording stops growing, or after four hours. A
//! database lease keeps it on one worker.
//!
//! A Coral connection (ADR 0046) gets the tenant language first and then
//! the other supported language; its completed transcripts name the
//! detected language. Its audio is not added to the cloud usage.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::live_transcripts;
use talkops_core::recordings::Segment;
use talkops_core::tenant::TenantId;
use talkops_core::transcription_api::ApiConfig;
use talkops_core::transcription_profiles::{self as profiles, Purpose};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{self, Message};
use uuid::Uuid;

use crate::live::read_pcm;
use crate::transcribe::{initial_prompt, resample};

/// Audio format of the session: PCM16 mono.
const RATE: u32 = 24_000;
/// Detector frame.
const FRAME_SECS: f32 = 0.02;
/// Audio before the detected start of speech, sent with the turn.
const LEAD_FRAMES: usize = 15;
/// Silence that ends a turn.
const END_FRAMES: u32 = 35;
/// Shorter turns are noise (a click) and are dropped.
const MIN_SPEECH_FRAMES: u32 = 10;
/// Longer turns are committed in parts, so text arrives in time.
const MAX_TURN_SECS: f32 = 15.0;
const POLL: Duration = Duration::from_millis(200);
const SAVE_EVERY: Duration = Duration::from_secs(1);
const BEAT_EVERY: Duration = Duration::from_secs(5);
/// No new audio for this long: the call has ended.
const IDLE_END: Duration = Duration::from_secs(20);
const MAX_STREAM: Duration = Duration::from_secs(4 * 3600);
/// Wait this long for outstanding transcripts at the end.
const DRAIN: Duration = Duration::from_secs(5);
const SPEAKERS: [&str; 2] = ["caller", "called"];

/// Previews this worker streams now.
#[derive(Debug, Clone, Default)]
pub struct Streams(Arc<Mutex<HashSet<Uuid>>>);

impl Streams {
    fn insert(&self, id: Uuid) -> bool {
        self.0.lock().is_ok_and(|mut s| s.insert(id))
    }

    fn remove(&self, id: Uuid) {
        if let Ok(mut s) = self.0.lock() {
            s.remove(&id);
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.0.lock().is_ok_and(|s| s.is_empty())
    }
}

/// What a stream needs; it outlives the job that starts it.
pub struct Start {
    pub pool: PgPool,
    pub worker_id: String,
    pub streams: Streams,
    pub tenant: TenantId,
    pub preview: Uuid,
    pub source: PathBuf,
    pub cfg: ApiConfig,
    pub language: String,
    pub vocabulary: String,
    pub price: Option<f64>,
    pub coral_stable_preview: bool,
}

/// Starts a stream unless one already runs for this preview.
pub async fn start(s: Start) -> anyhow::Result<()> {
    if !s.streams.insert(s.preview) {
        return Ok(());
    }
    match live_transcripts::claim_stream(&s.pool, s.tenant, s.preview, &s.worker_id).await {
        Ok(true) => {}
        other => {
            s.streams.remove(s.preview);
            other?;
            return Ok(());
        }
    }
    tokio::spawn(async move {
        let result = stream(&s).await;
        if let Err(err) = &result {
            tracing::warn!(preview = %s.preview, error = %format!("{err:#}"), "realtime transcription failed");
        }
        if let Err(err) = live_transcripts::release_stream(
            &s.pool,
            s.tenant,
            s.preview,
            &s.worker_id,
            result.is_err(),
        )
        .await
        {
            tracing::warn!(error = %err, "releasing realtime stream failed");
        }
        s.streams.remove(s.preview);
    });
    Ok(())
}

/// What the detector asks the session to do.
#[derive(Debug, PartialEq)]
enum Action {
    /// Send audio (source rate) of the open turn.
    Append(Vec<f32>),
    /// End the turn; its start and end in seconds of the recording.
    Commit(f32, f32),
    /// Drop the audio of a turn that was only noise.
    Clear,
}

/// Energy-based turn detector for one channel.
struct Turns {
    rate: u32,
    /// Incomplete frame.
    rest: Vec<f32>,
    lead: VecDeque<Vec<f32>>,
    /// Seconds of the recording at the start of `rest`.
    time: f32,
    turn_start: Option<f32>,
    speech_frames: u32,
    silent_frames: u32,
    /// Slowly follows the quiet level of the line.
    floor: f32,
}

impl Turns {
    fn new(rate: u32, time: f32) -> Self {
        Self {
            rate,
            rest: Vec::new(),
            lead: VecDeque::new(),
            time,
            turn_start: None,
            speech_frames: 0,
            silent_frames: 0,
            floor: 0.002,
        }
    }

    fn push(&mut self, samples: &[f32]) -> Vec<Action> {
        let frame = (self.rate as f32 * FRAME_SECS) as usize;
        self.rest.extend_from_slice(samples);
        let mut out = Vec::new();
        let mut append = Vec::new();
        let mut at = 0;
        while self.rest.len() - at >= frame {
            let f = self.rest[at..at + frame].to_vec();
            at += frame;
            let rms = (f.iter().map(|s| s * s).sum::<f32>() / frame as f32).sqrt();
            let speech = rms >= (self.floor * 4.0).max(0.006);
            if !speech {
                self.floor = (self.floor * 0.98 + rms * 0.02).min(0.05);
            }
            let end = self.time + FRAME_SECS;
            match self.turn_start {
                None if speech => {
                    let lead = self.lead.len() as f32 * FRAME_SECS;
                    self.turn_start = Some(self.time - lead);
                    self.speech_frames = 1;
                    self.silent_frames = 0;
                    append.extend(self.lead.drain(..).flatten());
                    append.extend_from_slice(&f);
                }
                None => {
                    self.lead.push_back(f);
                    if self.lead.len() > LEAD_FRAMES {
                        self.lead.pop_front();
                    }
                }
                Some(start) => {
                    append.extend_from_slice(&f);
                    if speech {
                        self.speech_frames += 1;
                        self.silent_frames = 0;
                    } else {
                        self.silent_frames += 1;
                    }
                    if self.silent_frames >= END_FRAMES || end - start >= MAX_TURN_SECS {
                        if !append.is_empty() {
                            out.push(Action::Append(std::mem::take(&mut append)));
                        }
                        if self.speech_frames >= MIN_SPEECH_FRAMES {
                            out.push(Action::Commit(start, end));
                        } else {
                            out.push(Action::Clear);
                        }
                        // A long turn goes on as a new one; a pause ends it.
                        self.turn_start = (self.silent_frames < END_FRAMES).then_some(end);
                        self.speech_frames = 0;
                        self.silent_frames = 0;
                    }
                }
            }
            self.time = end;
        }
        self.rest.drain(..at);
        if !append.is_empty() {
            out.push(Action::Append(append));
        }
        out
    }

    /// Ends an open turn (at the end of the stream).
    fn finish(&mut self) -> Option<Action> {
        let start = self.turn_start.take()?;
        Some(if self.speech_frames >= MIN_SPEECH_FRAMES {
            Action::Commit(start, self.time)
        } else {
            Action::Clear
        })
    }
}

/// The transcript built from the session events of both channels.
#[derive(Default)]
struct Transcript {
    done: Vec<Segment>,
    /// Text deltas of turns in progress, by item.
    partial: HashMap<String, Segment>,
    /// Turn times by item, once committed.
    times: HashMap<String, (f32, f32)>,
    /// Committed turns per channel that have no item yet.
    committed: [VecDeque<(f32, f32)>; 2],
    /// Start of the open turn per channel.
    open: [Option<f32>; 2],
    /// Latest audio time.
    now: f32,
    /// Committed turns without a final transcript.
    pending: usize,
    changed: bool,
}

impl Transcript {
    fn new(done: Vec<Segment>) -> Self {
        Self {
            done,
            ..Default::default()
        }
    }

    fn commit(&mut self, channel: usize, start: f32, end: f32) {
        self.committed[channel].push_back((start, end));
        self.open[channel] = None;
        self.pending += 1;
    }

    fn handle(&mut self, channel: usize, event: &str) -> anyhow::Result<()> {
        let v: Value = serde_json::from_str(event).context("invalid realtime event")?;
        let item = || v["item_id"].as_str().unwrap_or_default().to_owned();
        match v["type"].as_str().unwrap_or_default() {
            "input_audio_buffer.committed" => {
                let time = self.committed[channel]
                    .pop_front()
                    .unwrap_or((self.now, self.now));
                if let Some(p) = self.partial.get_mut(&item()) {
                    (p.start, p.end) = time;
                }
                self.times.insert(item(), time);
            }
            "conversation.item.input_audio_transcription.delta" => {
                let item = item();
                let (start, end) = self
                    .times
                    .get(&item)
                    .copied()
                    .unwrap_or((self.open[channel].unwrap_or(self.now), self.now));
                let segment = self.partial.entry(item).or_insert_with(|| Segment {
                    start,
                    end,
                    speaker: SPEAKERS[channel].into(),
                    text: String::new(),
                    language: None,
                });
                segment
                    .text
                    .push_str(v["delta"].as_str().unwrap_or_default());
                self.changed = true;
            }
            "conversation.item.input_audio_transcription.completed" => {
                let item = item();
                let partial = self.partial.remove(&item);
                let (start, end) = self
                    .times
                    .remove(&item)
                    .or(partial.map(|p| (p.start, p.end)))
                    .unwrap_or((self.now, self.now));
                let text = v["transcript"].as_str().unwrap_or_default().trim();
                if !text.is_empty() {
                    self.done.push(Segment {
                        start,
                        end,
                        speaker: SPEAKERS[channel].into(),
                        text: text.to_owned(),
                        language: v["language"].as_str().map(crate::api::normalize_language),
                    });
                }
                self.pending = self.pending.saturating_sub(1);
                self.changed = true;
            }
            "conversation.item.input_audio_transcription.failed" => {
                self.partial.remove(&item());
                self.times.remove(&item());
                self.pending = self.pending.saturating_sub(1);
                self.changed = true;
                tracing::warn!(error = %v["error"], "realtime turn not transcribed");
            }
            "error" => {
                let msg = v["error"]["message"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v["error"].to_string());
                bail!("realtime API: {msg}");
            }
            _ => {}
        }
        Ok(())
    }

    /// Final and provisional segments in time order.
    fn segments(&self) -> Vec<Segment> {
        let mut all: Vec<Segment> = self
            .done
            .iter()
            .chain(self.partial.values().filter(|p| !p.text.trim().is_empty()))
            .cloned()
            .collect();
        all.sort_by(|a, b| a.start.total_cmp(&b.start));
        all
    }
}

/// Languages of the Coral models (ADR 0046).
const CORAL_LANGUAGES: [&str; 2] = ["de", "en"];

/// The session configuration. Newer models take keywords and a language
/// list; older ones a prompt and one language. Coral takes the language
/// list in order of preference: the tenant language, then the other one.
fn session_update(
    model: &str,
    language: &str,
    vocabulary: &str,
    coral: bool,
    stable_preview: bool,
) -> Value {
    let mut transcription = json!({ "model": model });
    if coral {
        let mut languages = vec![language];
        languages.extend(CORAL_LANGUAGES.iter().filter(|l| **l != language));
        transcription["languages"] = json!(languages);
        transcription["stable_preview"] = json!(stable_preview);
    } else if model.starts_with("gpt-live-transcribe") || model.starts_with("gpt-transcribe") {
        transcription["languages"] = json!([language]);
        let keywords: Vec<String> = vocabulary
            .split(',')
            .map(|w| w.replace(['<', '>', '\r', '\n'], "").trim().to_owned())
            .filter(|w| !w.is_empty())
            .collect();
        if !keywords.is_empty() {
            transcription["keywords"] = json!(keywords);
        }
    } else {
        transcription["language"] = json!(language);
        transcription["prompt"] = json!(initial_prompt(language, vocabulary));
    }
    json!({
        "type": "session.update",
        "session": {
            "type": "transcription",
            "audio": {
                "input": {
                    "format": { "type": "audio/pcm", "rate": RATE },
                    "transcription": transcription,
                    "turn_detection": null,
                }
            }
        }
    })
}

/// Base64 PCM16 at 24 kHz.
fn encode(samples: &[f32], rate: u32) -> String {
    let bytes: Vec<u8> = resample(samples, rate, RATE)
        .into_iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn tls() -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth(),
    )
}

async fn connect(s: &Start, channel: usize) -> anyhow::Result<Socket> {
    let mut request = s.cfg.realtime_endpoint().into_client_request()?;
    if let Some(key) = &s.cfg.key {
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {key}"))?,
        );
    }
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        tokio_tungstenite::connect_async_tls_with_config(
            request,
            None,
            false,
            Some(tokio_tungstenite::Connector::Rustls(tls())),
        ),
    )
    .await
    .context("realtime API: connection timed out")?
    .map_err(|err| match err {
        tungstenite::Error::Http(res) => anyhow::anyhow!(
            "realtime API: HTTP {}: {}",
            res.status(),
            String::from_utf8_lossy(res.body().as_deref().unwrap_or_default())
                .chars()
                .take(300)
                .collect::<String>()
        ),
        other => anyhow::anyhow!("realtime API not reachable ({}): {other}", s.cfg.url),
    })?;
    let mut update = session_update(
        &s.cfg.model,
        &s.language,
        &s.vocabulary,
        s.cfg.is_coral(),
        s.coral_stable_preview,
    );
    if s.cfg.is_coral() {
        let transcription = &mut update["session"]["audio"]["input"]["transcription"];
        transcription["diagnostic_scope"] = json!(s.tenant.to_string());
        transcription["diagnostic_preview"] = json!(s.preview.to_string());
        transcription["diagnostic_channel"] = json!(channel);
        transcription["diagnostic_worker_revision"] = json!(option_env!("TALKOPS_BUILD_REVISION"));
    }
    socket
        .send(Message::Text(update.to_string().into()))
        .await?;
    Ok(socket)
}

async fn stream(s: &Start) -> anyhow::Result<()> {
    let work = live_transcripts::work(&s.pool, s.tenant, s.preview)
        .await?
        .context("preview ended")?;
    let mut saved = work.cursor_frames;
    let mut cursor = work.cursor_frames;
    let mut transcript = Transcript::new(work.segments.0);
    let (mut tx0, rx0) = connect(s, 0).await?.split();
    let (mut tx1, rx1) = connect(s, 1).await?.split();
    // The stream runs: the text comes from this engine (no fallback).
    live_transcripts::set_engine(
        &s.pool,
        s.tenant,
        s.preview,
        &format!("api:{}", s.cfg.label()),
        None,
    )
    .await?;
    let mut events =
        futures_util::stream::select(rx0.map(|m| (0usize, m)), rx1.map(|m| (1usize, m)));
    let mut turns: Option<[Turns; 2]> = None;
    let mut tick = tokio::time::interval(POLL);
    let started = Instant::now();
    let (mut last_audio, mut last_save, mut last_beat) = (started, started, started);
    let mut ending: Option<Instant> = None;
    // Samples (source rate) sent and already added to the usage.
    let (mut sent, mut billed) = (0usize, 0usize);
    let mut rate = 8000;
    loop {
        tokio::select! {
            event = events.next() => match event {
                Some((ch, Ok(Message::Text(text)))) => transcript.handle(ch, &text)?,
                Some((_, Ok(Message::Close(frame)))) => {
                    bail!("realtime API closed the session: {}", frame.map(|f| f.reason.to_string()).unwrap_or_default())
                }
                Some((_, Ok(_))) => {}
                Some((_, Err(err))) => return Err(err).context("realtime API connection"),
                None => bail!("realtime API closed the connection"),
            },
            _ = tick.tick() => {
                if ending.is_none() {
                    let (source, at) = (s.source.clone(), cursor);
                    let pcm = tokio::task::spawn_blocking(move || read_pcm(&source, at, 0.1, 2.0)).await??;
                    if let Some(pcm) = pcm {
                        rate = pcm.rate;
                        let turns = turns.get_or_insert_with(|| {
                            let time = pcm.start as f32 / pcm.rate as f32;
                            [Turns::new(pcm.rate, time), Turns::new(pcm.rate, time)]
                        });
                        cursor = pcm.end as i64;
                        transcript.now = pcm.end as f32 / pcm.rate as f32;
                        last_audio = Instant::now();
                        for (ch, tx) in [(0, &mut tx0), (1, &mut tx1)] {
                            let mono: Vec<f32> = pcm.samples.iter().skip(ch).step_by(2)
                                .map(|&v| f32::from(v) / 32768.0).collect();
                            for action in turns[ch].push(&mono) {
                                sent += send(tx, &mut transcript, ch, action, pcm.rate).await?;
                            }
                            transcript.open[ch] = turns[ch].turn_start;
                        }
                    }
                    if last_audio.elapsed() >= IDLE_END || started.elapsed() >= MAX_STREAM {
                        ending = Some(Instant::now());
                        if let Some(turns) = turns.as_mut() {
                            for (ch, tx) in [(0, &mut tx0), (1, &mut tx1)] {
                                if let Some(action) = turns[ch].finish() {
                                    send(tx, &mut transcript, ch, action, rate).await?;
                                }
                            }
                        }
                    }
                }
                if last_beat.elapsed() >= BEAT_EVERY {
                    last_beat = Instant::now();
                    if !live_transcripts::beat_stream(&s.pool, s.tenant, s.preview, &s.worker_id).await? {
                        break; // the viewer left
                    }
                }
                let done = ending.is_some_and(|t| transcript.pending == 0 || t.elapsed() >= DRAIN);
                if (transcript.changed || cursor != saved) && (last_save.elapsed() >= SAVE_EVERY || done) {
                    last_save = Instant::now();
                    let seconds = cursor.max(0) as f32 / rate as f32;
                    if !live_transcripts::save(&s.pool, s.tenant, s.preview, saved, cursor, seconds, transcript.segments()).await? {
                        break; // the viewer left or another worker took over
                    }
                    saved = cursor;
                    transcript.changed = false;
                    billed += bill(s, sent - billed, rate).await;
                }
                if done {
                    break;
                }
            }
        }
    }
    bill(s, sent - billed, rate).await;
    let _ = tx0.close().await;
    let _ = tx1.close().await;
    Ok(())
}

/// Sends one action; returns the samples sent.
async fn send<S>(
    tx: &mut S,
    transcript: &mut Transcript,
    channel: usize,
    action: Action,
    rate: u32,
) -> anyhow::Result<usize>
where
    S: futures_util::Sink<Message, Error = tungstenite::Error> + Unpin,
{
    let (event, samples) = match action {
        Action::Append(audio) => (
            json!({"type": "input_audio_buffer.append", "audio": encode(&audio, rate)}),
            audio.len(),
        ),
        Action::Commit(start, end) => {
            transcript.commit(channel, start, end);
            (json!({"type": "input_audio_buffer.commit"}), 0)
        }
        Action::Clear => (json!({"type": "input_audio_buffer.clear"}), 0),
    };
    tx.send(Message::Text(event.to_string().into())).await?;
    Ok(samples)
}

/// Adds sent audio to the usage; returns the samples it covered.
async fn bill(s: &Start, samples: usize, rate: u32) -> usize {
    // Coral runs on this server: nothing to bill.
    if samples == 0 || s.cfg.is_coral() {
        return samples;
    }
    let seconds = samples as f64 / f64::from(rate);
    match profiles::record_usage(
        &s.pool,
        s.tenant,
        Purpose::Live,
        &s.cfg.label(),
        seconds,
        s.price,
    )
    .await
    {
        Ok(()) => samples,
        Err(err) => {
            tracing::warn!(error = %err, "recording transcription usage failed");
            0
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const R: u32 = 8000;

    pub(crate) fn tone(secs: f32) -> Vec<f32> {
        (0..(secs * R as f32) as usize)
            .map(|i| (i as f32 * 0.3).sin() * 0.3)
            .collect()
    }

    pub(crate) fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (secs * R as f32) as usize]
    }

    fn commits(actions: &[Action]) -> Vec<(f32, f32)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Commit(s, e) => Some((*s, *e)),
                _ => None,
            })
            .collect()
    }

    fn sent(actions: &[Action]) -> usize {
        actions
            .iter()
            .map(|a| match a {
                Action::Append(v) => v.len(),
                _ => 0,
            })
            .sum()
    }

    #[test]
    fn detects_turns_with_lead_in_and_times() {
        let mut t = Turns::new(R, 10.0);
        let mut audio = silence(1.0);
        audio.extend(tone(2.0));
        audio.extend(silence(1.0));
        // Pushed in uneven pieces, as read from the growing file.
        let mut actions = Vec::new();
        for piece in audio.chunks(1234) {
            actions.extend(t.push(piece));
        }
        let c = commits(&actions);
        assert_eq!(c.len(), 1, "{actions:?}");
        assert!((c[0].0 - 10.7).abs() < 0.03, "{c:?}");
        assert!((c[0].1 - 13.7).abs() < 0.03, "{c:?}");
        // Silence before the lead-in is not sent.
        assert!(sent(&actions) < (3.1 * R as f32) as usize);
        assert!(t.finish().is_none());
    }

    #[test]
    fn splits_long_speech_and_drops_clicks() {
        let mut t = Turns::new(R, 0.0);
        let actions = t.push(&tone(40.0));
        assert_eq!(commits(&actions).len(), 2, "{actions:?}");
        assert!(matches!(t.finish(), Some(Action::Commit(..))));

        let mut t = Turns::new(R, 0.0);
        let mut audio = tone(0.06);
        audio.extend(silence(1.0));
        let actions = t.push(&audio);
        assert!(commits(&actions).is_empty());
        assert!(actions.contains(&Action::Clear), "{actions:?}");
    }

    #[test]
    fn builds_segments_from_events() {
        let mut t = Transcript::new(vec![]);
        t.now = 5.0;
        t.open[0] = Some(1.0);
        t.handle(0, r#"{"type":"conversation.item.input_audio_transcription.delta","item_id":"a","delta":"Hal"}"#).unwrap();
        t.handle(0, r#"{"type":"conversation.item.input_audio_transcription.delta","item_id":"a","delta":"lo"}"#).unwrap();
        assert_eq!(t.segments()[0].text, "Hallo");
        assert_eq!(t.segments()[0].start, 1.0);
        t.commit(0, 1.2, 4.0);
        t.commit(1, 2.0, 3.0);
        t.handle(
            0,
            r#"{"type":"input_audio_buffer.committed","item_id":"a"}"#,
        )
        .unwrap();
        t.handle(
            1,
            r#"{"type":"input_audio_buffer.committed","item_id":"b"}"#,
        )
        .unwrap();
        assert_eq!(t.pending, 2);
        t.handle(1, r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"b","transcript":" Ja? "}"#).unwrap();
        t.handle(0, r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"a","transcript":"Hallo, hier ist Anna."}"#).unwrap();
        let s = t.segments();
        assert_eq!(t.pending, 0);
        assert_eq!(s.len(), 2);
        assert_eq!(
            (s[0].start, s[0].end, s[0].speaker.as_str()),
            (1.2, 4.0, "caller")
        );
        assert_eq!(
            (s[1].text.as_str(), s[1].speaker.as_str()),
            ("Ja?", "called")
        );
        assert!(
            t.handle(0, r#"{"type":"error","error":{"message":"Invalid key"}}"#)
                .is_err()
        );
    }

    #[test]
    fn configures_the_session() {
        let live = session_update(
            "gpt-live-transcribe",
            "de",
            "TalkOps, <Yealink>, ",
            false,
            false,
        );
        let tr = &live["session"]["audio"]["input"]["transcription"];
        assert_eq!(tr["languages"], json!(["de"]));
        assert_eq!(tr["keywords"], json!(["TalkOps", "Yealink"]));
        assert!(tr.get("prompt").is_none() && tr.get("language").is_none());
        assert_eq!(live["session"]["type"], "transcription");
        assert!(live["session"]["audio"]["input"]["turn_detection"].is_null());
        assert_eq!(live["session"]["audio"]["input"]["format"]["rate"], 24000);
        let old = session_update("gpt-4o-transcribe", "en", "", false, false);
        let tr = &old["session"]["audio"]["input"]["transcription"];
        assert_eq!(tr["language"], "en");
        assert!(tr["prompt"].as_str().unwrap().contains("phone call"));
        // Coral: the tenant language first, then the other; no prompt.
        let coral = session_update("coral-auto", "en", "TalkOps", true, false);
        let tr = &coral["session"]["audio"]["input"]["transcription"];
        assert_eq!(tr["model"], "coral-auto");
        assert_eq!(tr["languages"], json!(["en", "de"]));
        assert!(tr.get("prompt").is_none() && tr.get("keywords").is_none());
        let coral = session_update("coral-auto", "de", "", true, false);
        assert_eq!(
            coral["session"]["audio"]["input"]["transcription"]["languages"],
            json!(["de", "en"])
        );
        let coral = session_update("coral-auto", "de", "", true, true);
        assert_eq!(
            coral["session"]["audio"]["input"]["transcription"]["stable_preview"],
            true
        );
        assert!(
            live["session"]["audio"]["input"]["transcription"]
                .get("stable_preview")
                .is_none()
        );
    }

    #[test]
    fn encodes_pcm16_at_24k() {
        let b64 = encode(&tone(1.0), R);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        assert_eq!(bytes.len(), 24_000 * 2);
    }
}
