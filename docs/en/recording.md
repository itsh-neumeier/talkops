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

Per extension (**Extensions → Edit → Call recording**):

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

**Accuracy** (Settings → Recording): *Fast* uses the model from
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

## Search

**Search** finds words in all transcripts you have access to – calls and
voicemails. Syntax as in web search engines: `"exact phrase"`, `-exclude`,
`invoice or reminder`. Hits can be played directly.

## Disk space

One minute of a call takes about 1.9 MB (8 kHz, 16 bit, stereo; more with HD
codecs). Recordings live in the `recordings` volume
(`<tenant>/<year-month>/<call>.wav`) – include it in your backups.
