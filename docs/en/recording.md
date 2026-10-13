# Call recording and transcription

TalkOps can record calls, turn recordings and voicemails into text locally
(Whisper) and search that text. No audio leaves the server.

> **Legal note:** in many countries (Germany: § 201 StGB) recording a call
> without the consent of everyone involved is a criminal offence. Keep the
> announcement switched on, inform your staff and clarify the use within your
> organisation (works council, data protection) beforehand.

## Setup

**Settings → Call recording and transcription** (admin):

- **Record calls** – separately for incoming, outgoing and internal calls.
  This is the default for all extensions.
- **Announce the recording** – both parties hear "This call is being
  recorded." when the call is answered (default: on). In queues the caller
  hears it before waiting.
- **Keep recordings (days)** – older recordings and their transcripts are
  deleted automatically (checked hourly). `0` = forever. Default: 90.
- **Transcribe recordings and voicemails** – switches Whisper on.

Per extension (**Settings → Extensions → Edit → Call recording**):

| Setting | Effect |
|---|---|
| Default (settings) | the per-direction setting applies |
| Always record | calls of this extension are always recorded |
| Never record | never – even if the other side is set to "always" |

Only answered calls are recorded. A call that is not answered and goes to
voicemail produces no call recording (the voicemail is stored as usual). In
queues the recording starts when the caller enters the queue, so it includes
the waiting time.

## Listening to recordings

In the **Call log**, recorded calls have a **Recording** button: play,
download (WAV, stereo: caller left, called party right) and – if available –
the transcript with speaker and time. Only admins can delete recordings.

Who may access what?

- Users: recordings and transcripts of calls of their own extensions.
- Admins: everything; listening to other people's calls, reading their
  transcripts and deleting are written to the audit log.
- Operators see all calls in the call log, but recordings only like users.

## Transcription

The dashboard has an optional [live preview](active-calls.md) for calls that
are already recorded. Final transcription remains separate.

With transcription enabled, the media worker turns every new recording and
voicemail into text:

- Both sides of a call are recognised separately, so the transcript shows who
  said what (*Caller* / *Called party*).
- Each line of the transcript is one short part of a turn. Whisper gives a
  time for each word. TalkOps starts a new line at the end of a sentence, at
  a pause, when the other party speaks, and after 10 seconds at the latest.
  Then it puts the lines of both parties in the order of their times.
  Existing transcripts do not change.
- Language: the voicemail box language or the default language from the
  settings.
- The voicemail e-mail waits for the transcript and contains the text
  (on failure it is sent without text).
- The first time, the worker downloads the speech model (about 550 MB) and
  the voice activity model (Silero VAD, about 1 MB) from the official
  whisper.cpp repositories and verifies their checksums; afterwards it works
  offline.
- Voice activity detection (VAD) cuts out silence and hold music before
  Whisper transcribes. This prevents invented sentences in pauses and speeds
  recognition up. Turn it off with `TALKOPS_WHISPER_VAD=false`.

Choosing a model (`.env`):

**Local models** (Settings → Call handling → *Transcription engines*):
*Fast* uses the model from `TALKOPS_WHISPER_MODEL`. *Accurate*
(`large-v3-q5_0`, 1.1 GB, ~2 GB RAM) and *Best* (`large-v3`, 3.1 GB, ~4 GB
RAM) use the full large-v3 with a wider search: clearly fewer wrong words,
but two to four times slower. *Base* (~150 MB) is only for the live preview.
**Names and terms** (comma-separated) help with names, companies and
technical words. To compare in the container:
`talkops-media-worker transcribe --quality accurate --vocabulary "Name, Company" file.wav`.

| `TALKOPS_WHISPER_MODEL` | Size | Note |
|---|---|---|
| `large-v3-turbo-q5_0` (default) | ~550 MB | very accurate, also at phone quality; ~2 GB RAM |
| `large-v3-turbo-q8_0` | ~870 MB | slightly more accurate |
| `large-v3-turbo` | ~1.6 GB | full accuracy, needs a lot of RAM |
| `medium-q5_0` / `medium` | ~540 MB / 1.5 GB | alternative to turbo |
| `small` | ~490 MB | for weak hardware (e.g. Raspberry Pi) |
| `base` | ~150 MB | fast but much less accurate (default up to 1.2) |

On a current 4-core x86 server `large-v3-turbo-q5_0` needs about a third to
half of the call duration. If you set `base` explicitly, remove or change
that line in your `.env`. Place other models as `ggml-<name>.bin` in the
`models` volume yourself and set their name. Further option:
`TALKOPS_WHISPER_THREADS` (CPU threads per transcription, default up to 4).

### Engines per purpose

*Settings → Call handling → Transcription engines* selects an engine for each
purpose ([ADR 0024](../adr/0024-transcription-engines-per-purpose.md)):

| Purpose | When it runs | Default |
|---|---|---|
| Live preview | During an active, recorded call ([active calls](active-calls.md)) | Local *Base* |
| Recording – fast | Right after the call. Its text appears first. | Local *Fast* |
| Recording – slow, accurate | Optional. Afterwards, at lower priority. | Off |
| Voicemail – fast | Right after the message. The e-mail contains its text. | Local *Fast* |
| Voicemail – slow, accurate | Optional. Afterwards, at lower priority. | Off |

Each purpose uses **Local** (a Whisper model on this server) or **Cloud** (a
model of a cloud connection). A second pass replaces the first transcript
when it is complete. Until then, the transcript shows *Preliminary – a more
accurate version follows*, and the view refreshes automatically. If the
second pass fails (one retry), the first transcript stays. A second pass with
the same engine as the first pass does not run.

The transcript shows the model that made it, for example
`whisper:large-v3-turbo-q5_0` or `api:OpenAI/gpt-transcribe`.

An update from 1.7 or earlier copies the old settings: the old AI API becomes
the connection *OpenAI* (or *API* for another URL). The old first and second
pass apply to recordings and voicemails.

### Cloud connections (OpenAI, Groq, Mistral, own server)

*Cloud connections* holds one or more connections with the OpenAI
transcription interface (`POST …/audio/transcriptions`). Select **Add
connection**, select the type, and enter the key. For an own server, also
enter the URL. **Test connection** lists the speech models of the server
without sending audio. It also names the models of your engines that the
server does not offer.

| Type | URL | Models (examples) |
|---|---|---|
| OpenAI | fixed: `https://api.openai.com/v1` | `gpt-transcribe`, `gpt-4o-mini-transcribe`, `gpt-4o-transcribe`, `whisper-1`; live: `gpt-live-transcribe` |
| Groq | `https://api.groq.com/openai/v1` | `whisper-large-v3-turbo`, `whisper-large-v3` |
| Mistral | `https://api.mistral.ai/v1` | `voxtral-mini-latest` |
| Own server (Speaches, LocalAI, …) | for example `http://192.168.1.10:8000/v1` | depends on the server |

- **Privacy:** with a cloud model, recordings and voicemails go to the
  provider. With a cloud live preview, audio goes to the provider during the
  call. Inform the other parties and sign a data processing agreement
  (GDPR). An own server in the LAN keeps the data in-house.
- Each side of a call goes separately (16 kHz mono WAV), long calls in parts
  of up to 10 minutes. Models with timestamps (`whisper-1`, Groq) return
  sentences with times. TalkOps also asks for word timestamps to make short
  lines in the correct order of both parties. For models without timestamps (for example
  `gpt-transcribe`), TalkOps sends each part of the conversation separately,
  so that the order is correct. Servers that do not know the OpenAI options
  get only file, model and language.
- TalkOps stores the key encrypted (like the SMTP password) and never shows
  it. The media worker needs `TALKOPS_SECRET_KEY` for it (set in
  `docker-compose.yml` from 1.5).
- You cannot delete a connection that an engine uses. Set these engines to
  another connection or to local first.

### Live preview in the cloud

For the live preview, select **Cloud**. Two modes are available:

- **Without streaming:** TalkOps sends 8-second parts of the recording. The
  text has a delay of about 10 seconds. All connection types support this.
- **Stream live (Realtime API):** the media worker sends the speech of each
  side over a WebSocket (`…/realtime?intent=transcription`). Text appears
  while people speak, with a delay of about one second. For OpenAI, use
  `gpt-live-transcribe`. The worker sends only speech and stops the stream
  when the popup closes, when the call ends, or after four hours. The
  WebSocket connection does not use `HTTPS_PROXY`.

### Cloud usage and cost

*Cloud usage* shows the audio minutes that TalkOps sent to each cloud model
this month and last month, with an estimated cost. The estimate uses the
price per minute of the engine. Without an own price, it uses the OpenAI list
price (checked 2026-10-13, for example `gpt-transcribe` 0.0045 USD,
`gpt-live-transcribe` 0.017 USD per minute). An asterisk marks audio without a
known price, for example from an own server. The bill of the provider is
authoritative.

## Search

**Search** finds words in all transcripts you have access to – calls and
voicemails. Syntax as in web search engines: `"exact phrase"`, `-exclude`,
`invoice or reminder`. Hits can be played directly.

## Disk space

One minute of a call takes about 1.9 MB (8 kHz, 16 bit, stereo; more with HD
codecs). Recordings live in the `recordings` volume
(`<tenant>/<year-month>/<call>.wav`) – include it in your backups.
