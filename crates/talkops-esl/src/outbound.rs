//! Outbound mode: FreeSWITCH connects to TalkOps for a single call (dialplan
//! application `socket <addr> async full`) and hands over control of the
//! channel. Used for interactive flows such as voicemail and IVRs.
//!
//! The session runs applications one after another with `sendmsg`
//! (`event-lock`, tagged with an `Event-UUID`) and waits for the matching
//! `CHANNEL_EXECUTE_COMPLETE` event. `linger` keeps the socket open after the
//! caller hangs up, so the result of the application that was running (e.g. a
//! recording) still arrives.

use std::future::Future;
use std::time::Duration;

use percent_encoding::percent_decode_str;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::EslError;
use crate::frame::{self, Event, Frame, Headers};

/// How long command replies may take.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// After a hangup, how long to wait for the running application to report back.
const AFTER_HANGUP: Duration = Duration::from_secs(5);

/// Control over one call handed to TalkOps by FreeSWITCH.
pub struct OutboundSession {
    channel: Headers,
    writer: OwnedWriteHalf,
    replies: mpsc::UnboundedReceiver<Frame>,
    events: mpsc::UnboundedReceiver<Event>,
    hung_up: bool,
    seq: u64,
}

impl OutboundSession {
    /// Performs the outbound handshake (`connect`, `myevents`, `linger`).
    pub async fn accept(stream: TcpStream) -> Result<Self, EslError> {
        stream.set_nodelay(true)?;
        let (read_half, writer) = stream.into_split();
        let (reply_tx, replies) = mpsc::unbounded_channel();
        let (event_tx, events) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(BufReader::new(read_half), reply_tx, event_tx));

        let mut session = Self {
            channel: Headers::default(),
            writer,
            replies,
            events,
            hung_up: false,
            seq: 0,
        };
        let reply = session.command("connect").await?;
        // Channel data in the connect reply is URL-encoded like event headers.
        let mut channel = Headers::default();
        for (name, value) in reply.headers.iter() {
            channel.push(
                name.to_owned(),
                percent_decode_str(value).decode_utf8_lossy().into_owned(),
            );
        }
        session.channel = channel;
        session.command("myevents").await?;
        session.command("linger").await?;
        Ok(session)
    }

    /// Channel data FreeSWITCH sent on connect (`Unique-ID`, `Caller-…`, `variable_…`).
    pub fn channel(&self) -> &Headers {
        &self.channel
    }

    /// A channel variable as it was when the call was handed over.
    pub fn var(&self, name: &str) -> Option<&str> {
        self.channel
            .get(&format!("variable_{name}"))
            .filter(|v| !v.is_empty())
    }

    pub fn uuid(&self) -> &str {
        self.channel.get("Unique-ID").unwrap_or_default()
    }

    /// True once the caller hung up; further applications are not executed.
    pub fn is_hung_up(&self) -> bool {
        self.hung_up
    }

    async fn command(&mut self, command: &str) -> Result<Frame, EslError> {
        self.writer
            .write_all(format!("{command}\n\n").as_bytes())
            .await?;
        let reply = tokio::time::timeout(REPLY_TIMEOUT, self.replies.recv())
            .await
            .map_err(|_| EslError::Timeout)?
            .ok_or(EslError::Hangup)?;
        // The `connect` reply carries channel data and is URL-encoded
        // throughout, including `Reply-Text` (`%2BOK`).
        let text = percent_decode_str(reply.headers.get("Reply-Text").unwrap_or_default())
            .decode_utf8_lossy()
            .into_owned();
        if text.starts_with("+OK") {
            Ok(reply)
        } else {
            Err(EslError::CommandFailed(text.trim_end().to_owned()))
        }
    }

    /// Executes a dialplan application and waits until it has finished.
    /// Returns the `CHANNEL_EXECUTE_COMPLETE` event (with the channel
    /// variables), or [`EslError::Hangup`] if the call ended before.
    pub async fn execute(&mut self, app: &str, arg: &str) -> Result<Event, EslError> {
        if self.hung_up {
            return Err(EslError::Hangup);
        }
        if [app, arg].iter().any(|s| s.contains(['\n', '\r'])) {
            return Err(EslError::Protocol(
                "application and argument must be single-line".into(),
            ));
        }
        self.seq += 1;
        let id = format!("{}-{}", self.uuid(), self.seq);
        let mut msg = format!(
            "sendmsg\ncall-command: execute\nexecute-app-name: {app}\nevent-lock: true\nEvent-UUID: {id}"
        );
        if !arg.is_empty() {
            msg.push_str(&format!("\nexecute-app-arg: {arg}"));
        }
        tracing::debug!(app, arg, id = %id, "outbound execute");
        self.command(&msg).await?;

        loop {
            let next = if self.hung_up {
                match tokio::time::timeout(AFTER_HANGUP, self.events.recv()).await {
                    Ok(ev) => ev,
                    Err(_) => return Err(EslError::Hangup),
                }
            } else {
                self.events.recv().await
            };
            let Some(event) = next else {
                self.hung_up = true;
                return Err(EslError::Hangup);
            };
            tracing::debug!(
                event = ?event.name(),
                application = ?event.headers.get("Application"),
                application_uuid = ?event.headers.get("Application-UUID"),
                waiting_for = %id,
                "outbound event"
            );
            match event.name() {
                Some("CHANNEL_EXECUTE_COMPLETE")
                    if event.headers.get("Application-UUID") == Some(id.as_str()) =>
                {
                    return Ok(event);
                }
                Some("CHANNEL_HANGUP") | Some("CHANNEL_HANGUP_COMPLETE") => self.hung_up = true,
                _ => {}
            }
        }
    }

    /// Sets a channel variable.
    pub async fn set(&mut self, name: &str, value: &str) -> Result<(), EslError> {
        self.execute("set", &format!("{name}={value}"))
            .await
            .map(|_| ())
    }

    /// Hangs up (no-op if the caller is already gone).
    pub async fn hangup(&mut self, cause: &str) {
        match self.execute("hangup", cause).await {
            Ok(_) | Err(EslError::Hangup) => {}
            Err(err) => tracing::debug!(error = %err, "hangup failed"),
        }
        self.hung_up = true;
    }
}

async fn read_loop<R>(
    mut reader: R,
    replies: mpsc::UnboundedSender<Frame>,
    events: mpsc::UnboundedSender<Event>,
) where
    R: tokio::io::AsyncBufRead + Unpin,
{
    loop {
        let frame = match frame::read_frame(&mut reader).await {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(err) => {
                tracing::debug!(error = %err, "outbound ESL read ended");
                break;
            }
        };
        match frame.content_type() {
            Some("command/reply") | Some("api/response") => {
                let _ = replies.send(frame);
            }
            Some("text/event-plain") => match frame.to_event() {
                Ok(event) => {
                    let _ = events.send(event);
                }
                Err(err) => tracing::warn!(error = %err, "malformed ESL event"),
            },
            // With `linger` the socket stays open after the notice; keep
            // reading until FreeSWITCH closes it.
            Some("text/disconnect-notice") => {}
            other => tracing::debug!(content_type = ?other, "ignoring ESL frame"),
        }
    }
}

/// Accepts outbound connections forever and runs `handler` for each call.
pub async fn serve<H, F>(listener: TcpListener, handler: H)
where
    H: Fn(OutboundSession) -> F + Send + Sync + Clone + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::warn!(error = %err, "outbound ESL accept failed");
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
        };
        let handler = handler.clone();
        tokio::spawn(async move {
            match OutboundSession::accept(stream).await {
                Ok(session) => handler(session).await,
                Err(err) => tracing::warn!(%peer, error = %err, "outbound ESL handshake failed"),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncBufReadExt;

    fn event(body: &str) -> String {
        format!(
            "Content-Type: text/event-plain\nContent-Length: {}\n\n{body}",
            body.len()
        )
    }

    /// Plays FreeSWITCH: connects, answers the handshake and every
    /// `sendmsg`; the `record` application ends with a hangup.
    async fn fake_freeswitch(addr: std::net::SocketAddr) {
        let stream = TcpStream::connect(addr).await.unwrap();
        let (r, mut w) = stream.into_split();
        let mut r = BufReader::new(r);
        loop {
            let mut block = String::new();
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                if line == "\n" {
                    break;
                }
                block.push_str(&line);
            }
            let header = |name: &str| {
                block
                    .lines()
                    .find_map(|l| l.strip_prefix(&format!("{name}: ")))
                    .unwrap_or_default()
                    .to_owned()
            };
            let ok = "Content-Type: command/reply\nReply-Text: +OK\n\n";
            let out = match block.lines().next().unwrap() {
                // As FreeSWITCH 1.10 sends it: every value URL-encoded.
                "connect" => "Content-Type: command/reply\nReply-Text: %2BOK%0A\n\
                    Unique-ID: abc\nCaller-Caller-ID-Name: M%C3%BCller\n\
                    variable_talkops_app: vm_deposit\n\n"
                    .to_string(),
                "myevents" | "linger" => ok.to_string(),
                "sendmsg" => {
                    let id = header("Event-UUID");
                    match header("execute-app-name").as_str() {
                        "record" => format!(
                            "{ok}{}{}Content-Type: text/disconnect-notice\n\n",
                            event("Event-Name: CHANNEL_HANGUP\n\n"),
                            event(&format!(
                                "Event-Name: CHANNEL_EXECUTE_COMPLETE\nApplication-UUID: {id}\n\
                                 variable_record_ms: 4200\n\n"
                            ))
                        ),
                        "play_and_get_digits" => format!(
                            "{ok}{}{}",
                            event("Event-Name: DTMF\nDTMF-Digit: 1\n\n"),
                            event(&format!(
                                "Event-Name: CHANNEL_EXECUTE_COMPLETE\nApplication-UUID: {id}\n\
                                 variable_digits: 1\n\n"
                            ))
                        ),
                        _ => format!(
                            "{ok}{}",
                            event(&format!(
                                "Event-Name: CHANNEL_EXECUTE_COMPLETE\nApplication-UUID: {id}\n\n"
                            ))
                        ),
                    }
                }
                _ => "Content-Type: command/reply\nReply-Text: -ERR\n\n".to_string(),
            };
            w.write_all(out.as_bytes()).await.unwrap();
        }
    }

    #[tokio::test]
    async fn runs_applications_until_hangup() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(fake_freeswitch(addr));
        let (stream, _) = listener.accept().await.unwrap();
        let mut s = OutboundSession::accept(stream).await.unwrap();

        assert_eq!(s.uuid(), "abc");
        assert_eq!(s.var("talkops_app"), Some("vm_deposit"));
        assert_eq!(s.channel().get("Caller-Caller-ID-Name"), Some("Müller"));

        s.execute("answer", "").await.unwrap();
        let ev = s
            .execute("play_and_get_digits", "1 1 3 5000 # a.wav b.wav digits \\d")
            .await
            .unwrap();
        assert_eq!(ev.headers.get("variable_digits"), Some("1"));
        assert!(s.execute("playback", "x\ny").await.is_err());

        // The recording still reports back although the caller hung up.
        let ev = s.execute("record", "/tmp/x.wav 180").await.unwrap();
        assert_eq!(ev.headers.get("variable_record_ms"), Some("4200"));
        assert!(s.is_hung_up());
        assert!(matches!(
            s.execute("playback", "bye.wav").await,
            Err(EslError::Hangup)
        ));
        s.hangup("NORMAL_CLEARING").await;
    }
}
