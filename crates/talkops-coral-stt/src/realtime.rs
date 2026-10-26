//! Realtime transcription sessions over WebSocket, compatible with the
//! TalkOps realtime client (`talkops-media-worker/src/realtime.rs`) and the
//! OpenAI Realtime transcription events it uses:
//!
//! - client: `session.update` (PCM16, `turn_detection: null`,
//!   `transcription.model`, `transcription.languages`),
//!   `input_audio_buffer.append`, `.commit` and `.clear`;
//! - server: `session.created`, `session.updated`,
//!   `input_audio_buffer.committed`, `.cleared`,
//!   `conversation.item.input_audio_transcription.delta`, `.completed`
//!   (with the extra field `language`), `.failed` and `error`.
//!
//! The client detects the turns and commits them. The service runs the
//! windows of a turn while the audio arrives and sends the stable text as
//! deltas. Overload or a device failure ends the session with an `error`
//! event (code `overloaded`, `unavailable` or `inference_failed`), so the
//! client can fall back.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};

use crate::audio::{self, MODEL_RATE, StreamResampler};
use crate::engine::{Engine, EngineError, Priority, SessionGuard};
use crate::http::{choice, normalize_language};
use crate::models::AUTO;
use crate::recognizer::{Choice, Turn};

const DEFAULT_RATE: u32 = 24_000;

struct Open {
    item: String,
    turn: Turn,
    /// Text already sent as deltas.
    sent: String,
}

struct Session {
    engine: Arc<Engine>,
    choice: Choice,
    model: String,
    rate: u32,
    resampler: StreamResampler,
    open: Option<Open>,
    items: u64,
    previous: Option<String>,
    stable_preview: bool,
}

fn error_event(code: &str, message: &str) -> Value {
    json!({"type": "error", "error": {"type": "server_error", "code": code, "message": message}})
}

fn engine_code(err: &EngineError) -> &'static str {
    match err {
        EngineError::Overloaded => "overloaded",
        EngineError::Unavailable => "unavailable",
        EngineError::UnknownModel(_) => "model_not_found",
        EngineError::Failed(_) => "inference_failed",
    }
}

fn check_prefix(sent: &str, text: &str) -> Result<(), EngineError> {
    if sent.is_empty()
        || text == sent
        || text
            .strip_prefix(sent)
            .is_some_and(|rest| rest.starts_with(' '))
    {
        Ok(())
    } else {
        Err(EngineError::Failed(
            "a published preview word changed".into(),
        ))
    }
}

impl Session {
    fn new(engine: Arc<Engine>, stable_preview: bool) -> Self {
        let languages = engine.catalog.languages();
        Self {
            engine,
            choice: Choice::Auto(languages),
            model: AUTO.into(),
            rate: DEFAULT_RATE,
            resampler: StreamResampler::new(DEFAULT_RATE, MODEL_RATE),
            open: None,
            items: 0,
            previous: None,
            stable_preview,
        }
    }

    fn config(&self) -> Value {
        let languages = match &self.choice {
            Choice::Fixed(l) => vec![l.clone()],
            Choice::Auto(l) => l.clone(),
        };
        json!({
            "type": "transcription",
            "audio": {"input": {
                "format": {"type": "audio/pcm", "rate": self.rate},
                "transcription": {"model": self.model, "languages": languages,
                                  "stable_preview": self.stable_preview},
                "turn_detection": null,
            }}
        })
    }

    /// Applies `session.update`; returns the answer events.
    fn update(&mut self, v: &Value) -> Vec<Value> {
        let input = &v["session"]["audio"]["input"];
        let mut out = Vec::new();
        if let Some(rate) = input["format"]["rate"].as_u64() {
            match u32::try_from(rate) {
                Ok(rate) if (8000..=48_000).contains(&rate) => {
                    self.rate = rate;
                    self.resampler = StreamResampler::new(rate, MODEL_RATE);
                }
                _ => out.push(error_event(
                    "invalid_value",
                    "audio rate must be 8000 to 48000 Hz",
                )),
            }
        }
        if let Some(format) = input["format"]["type"].as_str()
            && format != "audio/pcm"
        {
            out.push(error_event(
                "invalid_value",
                "only audio/pcm (PCM16) is supported",
            ));
        }
        let t = &input["transcription"];
        let mut languages: Vec<String> = t["languages"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(l) = t["language"].as_str() {
            languages.insert(0, l.to_owned());
        }
        let model = t["model"].as_str().unwrap_or(&self.model).to_owned();
        match choice(&self.engine, &model, &languages) {
            Ok(Choice::Fixed(l)) => {
                self.choice = Choice::Fixed(l);
                self.model = model;
            }
            Ok(Choice::Auto(order)) => {
                // `languages` limits the choice when it names known ones.
                let named: Vec<String> = languages.iter().map(|l| normalize_language(l)).collect();
                let limited: Vec<String> = order
                    .iter()
                    .filter(|l| named.contains(l))
                    .cloned()
                    .collect();
                self.choice = Choice::Auto(if limited.len() >= 2 { limited } else { order });
                self.model = model;
            }
            Err(_) => out.push(error_event(
                "model_not_found",
                &format!("the model `{model}` does not exist"),
            )),
        }
        if !input["turn_detection"].is_null() && input.get("turn_detection").is_some() {
            out.push(error_event(
                "unsupported",
                "server turn detection is not supported; commit each turn",
            ));
        }
        out.push(json!({"type": "session.updated", "session": self.config()}));
        out
    }

    fn new_item(&mut self) -> String {
        self.items += 1;
        format!("item_{:06}", self.items)
    }

    async fn append(&mut self, audio_b64: &str) -> Result<Vec<Value>, EngineError> {
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(audio_b64) else {
            return Ok(vec![error_event("invalid_value", "audio is not base64")]);
        };
        let samples = self.resampler.push(&audio::pcm16(&bytes));
        if self.open.is_none() {
            let item = self.new_item();
            let turn = Turn::new(self.engine.clone(), self.choice.clone(), Priority::Live)?;
            self.open = Some(Open {
                item,
                turn,
                sent: String::new(),
            });
        }
        let Some(open) = self.open.as_mut() else {
            return Ok(Vec::new());
        };
        open.turn.push(&samples).await?;
        let mut out = Vec::new();
        let partial = if self.stable_preview {
            open.turn.stable_partial()
        } else {
            open.turn.partial()
        };
        if self.stable_preview
            && let Some(text) = &partial
        {
            check_prefix(&open.sent, text)?;
        }
        if let Some(text) = partial
            && text.len() > open.sent.len()
            && text.starts_with(&open.sent)
        {
            let delta = if open.sent.is_empty() {
                text.clone()
            } else {
                text[open.sent.len()..].to_owned()
            };
            out.push(json!({
                "type": "conversation.item.input_audio_transcription.delta",
                "item_id": open.item, "content_index": 0, "delta": delta,
            }));
            open.sent = text;
        }
        Ok(out)
    }

    async fn commit(&mut self) -> Result<Vec<Value>, EngineError> {
        // The end of the turn: flush the resampler with a little silence.
        let tail = self.resampler.push(&vec![0.0; self.rate as usize / 50]);
        self.resampler = StreamResampler::new(self.rate, MODEL_RATE);
        let open = match self.open.take() {
            Some(open) => open,
            None => Open {
                item: self.new_item(),
                turn: Turn::new(self.engine.clone(), self.choice.clone(), Priority::Live)?,
                sent: String::new(),
            },
        };
        let mut out = vec![json!({
            "type": "input_audio_buffer.committed",
            "item_id": open.item, "previous_item_id": self.previous,
        })];
        self.previous = Some(open.item.clone());
        let mut turn = open.turn;
        turn.push(&tail).await?;
        let decoded = turn.finish().await?;
        if self.stable_preview {
            check_prefix(&open.sent, &decoded.text)?;
        }
        out.push(json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "item_id": open.item, "content_index": 0,
            "transcript": decoded.text, "language": decoded.language,
        }));
        Ok(out)
    }

    fn clear(&mut self) -> Vec<Value> {
        self.open = None;
        self.resampler = StreamResampler::new(self.rate, MODEL_RATE);
        vec![json!({"type": "input_audio_buffer.cleared"})]
    }

    async fn handle(&mut self, text: &str) -> Result<Vec<Value>, EngineError> {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return Ok(vec![error_event("invalid_json", "the event is not JSON")]);
        };
        match v["type"].as_str().unwrap_or_default() {
            "session.update" | "transcription_session.update" => Ok(self.update(&v)),
            "input_audio_buffer.append" => {
                self.append(v["audio"].as_str().unwrap_or_default()).await
            }
            "input_audio_buffer.commit" => self.commit().await,
            "input_audio_buffer.clear" => Ok(self.clear()),
            other => Ok(vec![error_event(
                "unknown_event",
                &format!("unsupported event type `{other}`"),
            )]),
        }
    }
}

/// Runs one session until the client closes it or an engine error ends it.
pub async fn session(
    socket: WebSocket,
    engine: Arc<Engine>,
    guard: SessionGuard,
    stable_preview: bool,
) {
    let (mut tx, mut rx) = socket.split();
    let mut session = Session::new(engine, stable_preview);
    let created = json!({"type": "session.created", "session": session.config()});
    if tx
        .send(Message::Text(created.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    while let Some(Ok(message)) = rx.next().await {
        let text = match message {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        let (events, fatal) = match session.handle(&text).await {
            Ok(events) => (events, false),
            Err(err) => {
                tracing::warn!(error = %err, "realtime session ended");
                (vec![error_event(engine_code(&err), &err.to_string())], true)
            }
        };
        for event in events {
            if tx
                .send(Message::Text(event.to_string().into()))
                .await
                .is_err()
            {
                return;
            }
        }
        if fatal {
            let _ = tx.send(Message::Close(None)).await;
            break;
        }
    }
    drop(guard);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_preview_requires_complete_unchanged_words() {
        for text in ["", "hallo", "grüße"] {
            assert!(check_prefix("", text).is_ok());
        }
        assert!(check_prefix("grüße", "grüße zurück").is_ok());
        assert!(check_prefix("hallo", "hallo").is_ok());
        for text in ["", "hall", "hallob", "hello welt"] {
            assert!(check_prefix("hallo", text).is_err());
        }
    }
}
