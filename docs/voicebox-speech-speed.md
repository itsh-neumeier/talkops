# Voicebox speech speed

## Data and API

Migration `20261026000001_voicebox_speech_speed` adds `speech_speed` to
`voicemail_boxes` and `audio_clips`. Both fields use a default of 1.0 and a
database constraint of 0.5 to 2.0. Existing rows keep the previous speed.
This migration follows the existing `20261025` migrations to keep their order.

The voicebox update API accepts an optional `speech_speed`. If a client omits
the field, TalkOps keeps the stored speed. The TTS clip API uses 1.0 if a
client omits the field. Clip and voicebox responses include the stored speed.

`tenant_settings.voicemail_config.speeds` stores prompt speed overrides:

```json
{"de": {"vm_main_menu": 0.8, "vm_info_date": 0.7, "vm_numbers": 1.0}}
```

An override uses an absolute factor. Without an override, a prompt uses the
voicebox speed. An inline `<speed:0.7>text</speed>` tag takes precedence over
both. The parser rejects nested, incomplete, empty, or invalid tags. It limits
each render item to 64 segments. Speech speed applies during generation.
Recorded messages and uploaded audio keep their original samples.

## Audio generation

The worker uses the existing pinned Piper binary and the existing `hound`
library. It groups segments by speed. Each group uses one Piper process.
The process receives `--length_scale` equal to the model's configured default
divided by the speed factor. At 1.0, the worker does not override that default.
See the [pinned Piper source](https://github.com/rhasspy/piper/blob/2023.11.14-2/src/cpp/main.cpp).

The worker joins compatible 16-bit PCM segments in text order. It writes
temporary files in a unique render directory. A completed file replaces its
destination with a rename. A failure before an item's publication leaves its
existing destination unchanged.

Effective speed tags are part of each prompt's text hash. Untagged text at
1.0 keeps its existing prompt path. Box speed changes queue `tts_prompts`.
The worker renders each distinct box speed and refreshes stored message
announcements. Caller and date audio uses a hash of the current voice and
effective text. Call flows select that same hash. Old jobs cannot overwrite
the current announcement. A missing announcement is queued when listening
starts. Deleting a message also deletes its announcement variants.

Generated greeting clips are immutable. If a voicebox needs another speed,
TalkOps creates a separate clip in the same transaction as the box update.
Other references to the original clip keep their original audio.

Legacy TTS greeting jobs also use a separate file for each generation. The
call flow selects the current text, language, and speed. Existing normal-speed
audio at `greeting.wav` remains available as a fallback after an upgrade.

## Spoken counts

Mixed boxes use `vm_of`, `vm_messages_are`, and `vm_new`. For example:
"2 von 4 Sprachnachrichten sind neu." The total includes new and saved
messages. Playback announces the position and total: "Nachricht 2 von 4."
The existing empty, singular, new-only, and saved-only forms remain available.

## Upgrade

Run the new migration before starting the updated server and worker.
Deploy the server and worker together. No new dependency or voice model is
required. A release manager selects the release version. Existing prompt
files remain valid at speed 1.0. New prompt keys are generated at worker start.
