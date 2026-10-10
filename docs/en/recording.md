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

With transcription enabled, the media worker turns every new recording and
voicemail into text:

- Both sides of a call are recognised separately, so the transcript shows who
  said what (*Caller* / *Called party*).
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

**Accuracy** (Settings → Call handling): *Fast* uses the model from
`TALKOPS_WHISPER_MODEL`. *Accurate* (`large-v3-q5_0`, 1.1 GB, ~2 GB RAM) and
*Best* (`large-v3`, 3.1 GB, ~4 GB RAM) use the full large-v3 with a wider
search: clearly fewer wrong words, but two to four times slower. **Names and
terms** (comma-separated) help with names, companies and technical words. To
compare in the container:
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

### Two passes: quick first, accurate later

*Settings → Call handling* offers two passes:

- **First transcription (immediately)** runs right after the call. Its text
  appears first and goes out with the voicemail e-mail – best *Fast*.
- **Second, more accurate transcription (afterwards)** runs afterwards at
  lower priority (new first passes go first) and replaces the first
  transcript once done, e.g. *Best* (large-v3) or the AI API. Until then the
  transcript shows *Preliminary – a more accurate version follows*; the view
  refreshes by itself. If the second pass fails (one retry), the first
  transcript stays.

The transcript shows which model made it (e.g.
`whisper:large-v3-turbo-q5_0` or `api:whisper-1`).

### AI API (OpenAI, Groq, Mistral, own server)

Instead of local Whisper, either pass can use an OpenAI-compatible
transcription API (`POST …/audio/transcriptions`). Pick *AI API*, then enter
provider, URL, model and key and *Test connection* (lists the server's speech
models without sending audio):

| Provider | URL | Models (examples) |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `whisper-1`, `gpt-4o-transcribe`, `gpt-4o-mini-transcribe` |
| Groq | `https://api.groq.com/openai/v1` | `whisper-large-v3-turbo`, `whisper-large-v3` |
| Mistral | `https://api.mistral.ai/v1` | `voxtral-mini-latest` |
| own server (Speaches, LocalAI, …) | e.g. `http://192.168.1.10:8000/v1` | depends on the server |

- **Privacy:** with the AI API, recordings and voicemails go to the provider.
  Inform the other parties and sign a data processing agreement (GDPR); an
  own server in the LAN keeps the data in-house.
- Each side of a call is sent separately (16 kHz mono WAV), long calls in
  chunks of up to 10 minutes. Models with timestamps (`whisper-1`, Groq)
  return sentences with times; for models without (e.g. `gpt-4o-transcribe`)
  TalkOps sends each part of the conversation on its own so the order is
  right. Servers that do not know the OpenAI options get only file, model and
  language.
- The key is stored encrypted (like the SMTP password) and never shown. The
  media worker needs `TALKOPS_SECRET_KEY` for it (set in
  `docker-compose.yml` from 1.5 on).
- Tested against a simulated OpenAI-compatible server; the individual
  providers are implemented following their documentation.

## Search

**Search** finds words in all transcripts you have access to – calls and
voicemails. Syntax as in web search engines: `"exact phrase"`, `-exclude`,
`invoice or reminder`. Hits can be played directly.

## Disk space

One minute of a call takes about 1.9 MB (8 kHz, 16 bit, stereo; more with HD
codecs). Recordings live in the `recordings` volume
(`<tenant>/<year-month>/<call>.wav`) – include it in your backups.
