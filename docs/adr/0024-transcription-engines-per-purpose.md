# 0024 - Transcription engines per purpose, cloud connections and realtime previews

- Status: Accepted
- Date: 2026-10-13
- Supplements [0019](0019-transkription-in-zwei-durchgaengen-und-ki-api.md)
  and [0022](0022-active-call-identities-and-live-previews.md).
  ADR 0022 sets the live preview to the local `base` model. This ADR lets the
  administrator select a cloud model for the live preview too.

## Context

ADR 0019 added two passes and one OpenAI-compatible API connection. The two
passes apply to call recordings and voicemails together. The live preview of
ADR 0022 always uses the local `base` model.

Operators want a different engine for each purpose. For example: a quick
local model for recordings, an accurate cloud model for voicemails, and a
cloud model for the live preview. OpenAI now supplies `gpt-transcribe` for
recorded audio and `gpt-live-transcribe` for streamed audio through the
Realtime API. Operators also want an estimate of the cloud cost.

## Options

1. Keep one engine pair for recordings and voicemails. Add one setting for
   the live preview.
2. **One profile per purpose** (`live`, `recording`, `recording_refine`,
   `voicemail`, `voicemail_refine`). Each profile selects a local model or a
   model of a cloud connection.
3. One API connection for each tenant (as in ADR 0019).
4. **Several cloud connections** for each tenant, for example OpenAI and an
   own server in the LAN.
5. Live preview in the cloud with 8-second parts through the transcription
   endpoint.
6. **Live preview through a Realtime transcription session** (WebSocket),
   in addition to option 5.

## Decision

- **Options 2, 4, 5 and 6.**
- The table `transcription_providers` stores the connections. The kind
  `openai` has the fixed URL `https://api.openai.com/v1`. The kind
  `compatible` has a URL that the administrator sets. TalkOps encrypts the
  key with `TALKOPS_SECRET_KEY` (ADR 0008). The API never returns the key.
- The table `transcription_profiles` stores one row for each purpose. A
  missing row means the default: `live` uses local `base`, `recording` and
  `voicemail` use local `fast`, and the second passes are `off`. A composite
  foreign key keeps the connection in the same tenant. TalkOps does not
  delete a connection that a profile uses.
- The second pass of recordings and of voicemails is now separate. A second
  pass with the same engine as the first pass does not run (as in ADR 0019).
- The migration copies the old settings: the old API settings become the
  connection `OpenAI` (or `API` for another URL). The old first and second
  pass become the profiles of recordings and voicemails. The old columns in
  `tenant_settings` stay in the database. TalkOps no longer reads them.
- The live preview can use a cloud model in two modes:
  - **Parts:** the existing 8-second snapshots go to `/audio/transcriptions`.
    The delay is about 10 seconds.
  - **Realtime:** the media worker reads the growing recording and streams
    each channel on its own WebSocket to `{url}/realtime?intent=transcription`
    (`session.type = transcription`, PCM16 at 24 kHz, `turn_detection: null`).
    An energy-based detector in the worker finds the turns. The worker sends
    only speech with a 300 ms lead-in, and commits a turn after 700 ms of
    silence or after 15 seconds. The detector also supplies the segment
    times, because realtime models return no timestamps. Text deltas show
    as provisional segments. The completed transcript replaces them.
- A realtime stream runs as a background task of the worker and not in the
  job lane. The columns `stream_owner` and `stream_beat` of
  `live_transcript_previews` give a lease, so only one worker streams a
  preview. The stream stops when the viewer leaves (30 seconds without a
  request), when the recording does not grow for 20 seconds, or after four
  hours. After a failure the worker waits 30 seconds before a new attempt,
  and the preview shows the failed status.
- The worker adds the audio seconds that it sends to a cloud connection to
  `transcription_usage` (per day, purpose and `<connection>/<model>`). The
  cost uses the price of the profile, or the OpenAI list price when the
  profile has no price. The list prices are in
  `talkops_core::transcription_profiles::OPENAI_PRICES`, with the source and
  date. The settings page shows this and the previous month. The hourly
  retention task deletes usage rows that are older than two years.
- The WebSocket client uses `tokio-tungstenite` with rustls (ring) and the
  webpki roots. The workspace already contains these crates.

## Consequences

- Each purpose can have its own engine. Local remains the default. Cloud
  needs an explicit selection, and the UI shows a privacy note. For the live
  preview, the note tells that audio leaves the system during the call.
- The realtime preview has about one second of delay. It costs more per
  minute than file transcription (OpenAI list price 0.017 USD against
  0.0045 USD for `gpt-transcribe`). Only speech is sent.
- The detector does not separate speakers on the same channel. The stereo
  channels still give the caller and the called party.
- The usage table is an estimate. The bill of the provider is authoritative.
  Usage of a deleted connection stays with its old label.
- The realtime connection does not use the `HTTPS_PROXY` variables. File
  transcription uses curl and follows them.
- Servers without the Realtime interface can still use the parts mode.
