//! Building blocks for interactive calls: play prompt sequences, collect
//! digits, record. Works on any [`Call`], so flows can be tested with a
//! scripted fake instead of FreeSWITCH.

use std::future::Future;
use std::path::{Path, PathBuf};

use talkops_core::prompts;
use talkops_esl::outbound::OutboundSession;
use talkops_esl::{EslError, Event};

/// Control over a call (implemented by the outbound ESL session).
pub trait Call: Send {
    /// Channel variable from when the call was handed over.
    fn var(&self, name: &str) -> Option<String>;
    /// Runs a dialplan application to completion.
    fn execute(
        &mut self,
        app: &str,
        arg: &str,
    ) -> impl Future<Output = Result<Event, EslError>> + Send;
    fn is_hung_up(&self) -> bool;
    /// Hands the call back to FreeSWITCH after a transfer.
    fn release(&mut self) -> impl Future<Output = ()> + Send;
}

impl Call for OutboundSession {
    fn var(&self, name: &str) -> Option<String> {
        OutboundSession::var(self, name).map(str::to_owned)
    }

    fn execute(
        &mut self,
        app: &str,
        arg: &str,
    ) -> impl Future<Output = Result<Event, EslError>> + Send {
        OutboundSession::execute(self, app, arg)
    }

    fn is_hung_up(&self) -> bool {
        OutboundSession::is_hung_up(self)
    }

    fn release(&mut self) -> impl Future<Output = ()> + Send {
        OutboundSession::release(self)
    }
}

/// Short confirmation beep before recordings.
pub const BEEP: &str = "tone_stream://%(500,0,1000)";
const SILENCE: &str = "silence_stream://250";

/// A list of sounds played back to back.
#[derive(Debug, Default, Clone)]
pub struct Seq(Vec<String>);

impl Seq {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// FreeSWITCH file string; `None` if there is nothing to play.
    fn file_string(&self) -> Option<String> {
        match self.0.len() {
            0 => None,
            1 => Some(self.0[0].clone()),
            _ => Some(format!("file_string://{}", self.0.join("!"))),
        }
    }
}

/// Prompt player for one call in one language.
pub struct Ivr<'a, C: Call> {
    pub call: &'a mut C,
    sounds: &'a Path,
    lang: &'static str,
    seq: u32,
}

impl<'a, C: Call> Ivr<'a, C> {
    pub fn new(call: &'a mut C, sounds: &'a Path, lang: &str) -> Self {
        Self {
            call,
            sounds,
            lang: prompts::language(lang),
            seq: 0,
        }
    }

    /// Appends a system prompt (skipped if the media worker has not rendered it).
    pub fn prompt(&self, seq: &mut Seq, key: &str) {
        if let Some(path) = prompts::path(self.sounds, key, self.lang).filter(|p| p.is_file()) {
            seq.0.push(path.to_string_lossy().into_owned());
        }
    }

    /// Appends a number (`n0` … `n99`; larger numbers digit by digit).
    pub fn number(&self, seq: &mut Seq, n: u32) {
        if n <= prompts::MAX_NUMBER {
            self.prompt(seq, &format!("n{n}"));
        } else {
            self.digits(seq, &n.to_string());
        }
    }

    /// Appends digits one by one (phone numbers).
    pub fn digits(&self, seq: &mut Seq, digits: &str) {
        for d in digits.chars().filter(char::is_ascii_digit) {
            self.prompt(seq, &format!("n{d}"));
        }
    }

    pub fn file(&self, seq: &mut Seq, path: &Path) {
        if path.is_file() && !path.to_string_lossy().contains([' ', '!']) {
            seq.0.push(path.to_string_lossy().into_owned());
        }
    }

    pub fn beep(&self, seq: &mut Seq) {
        seq.0.push(BEEP.to_owned());
    }

    /// Builds a sequence from prompt keys.
    pub fn keys(&self, keys: &[&str]) -> Seq {
        let mut seq = Seq::default();
        for key in keys {
            self.prompt(&mut seq, key);
        }
        seq
    }

    pub async fn play(&mut self, seq: &Seq) -> Result<(), EslError> {
        match seq.file_string() {
            Some(file) => self.call.execute("playback", &file).await.map(|_| ()),
            None => Ok(()),
        }
    }

    pub async fn set(&mut self, name: &str, value: &str) -> Result<(), EslError> {
        self.call
            .execute("set", &format!("{name}={value}"))
            .await
            .map(|_| ())
    }

    /// Plays `seq` and collects up to `max` digits matching `regex`
    /// (3 tries, 5 s). `#` ends multi-digit input; for single-digit menus it
    /// is a regular choice. Returns `None` if nothing valid was entered.
    pub async fn ask(
        &mut self,
        seq: &Seq,
        max: u32,
        regex: &str,
    ) -> Result<Option<String>, EslError> {
        self.ask_with(seq, max, regex, 3, 5000).await
    }

    /// [`Ivr::ask`] with explicit tries and timeout (milliseconds).
    pub async fn ask_with(
        &mut self,
        seq: &Seq,
        max: u32,
        regex: &str,
        tries: u32,
        timeout_ms: u32,
    ) -> Result<Option<String>, EslError> {
        self.seq += 1;
        let var = format!("talkops_digits_{}", self.seq);
        let file = seq.file_string().unwrap_or_else(|| SILENCE.to_owned());
        let invalid = self
            .keys(&["vm_invalid"])
            .file_string()
            .unwrap_or_else(|| SILENCE.to_owned());
        let terminators = if max > 1 { "#" } else { "none" };
        let event = self
            .call
            .execute(
                "play_and_get_digits",
                &format!(
                    "1 {max} {tries} {timeout_ms} {terminators} {file} {invalid} {var} {regex} 3000"
                ),
            )
            .await?;
        Ok(event
            .headers
            .get(&format!("variable_{var}"))
            .filter(|d| !d.is_empty())
            .map(str::to_owned))
    }

    /// Records to `path` until `#`, silence or `max_secs`. Returns the
    /// length in seconds of what was recorded (also after a hangup).
    pub async fn record(&mut self, path: &Path, max_secs: i32) -> Result<u32, EslError> {
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        self.set("playback_terminators", "#").await?;
        match self
            .call
            .execute("record", &format!("{} {max_secs} 200 5", path.display()))
            .await
        {
            Ok(_) | Err(EslError::Hangup) => {}
            Err(err) => return Err(err),
        }
        Ok(wav_seconds(path).unwrap_or(0))
    }
}

/// Length of a WAV file in whole seconds.
pub fn wav_seconds(path: &Path) -> Option<u32> {
    let reader = hound::WavReader::open(path).ok()?;
    let rate = reader.spec().sample_rate.max(1);
    Some(reader.duration() / rate)
}

/// Temporary name next to `path`, used while recording greetings.
pub fn temp_path(path: &Path) -> PathBuf {
    path.with_extension("rec.wav")
}
