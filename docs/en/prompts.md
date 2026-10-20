# Prompts and audio

[Handbook](index.md) · [Deutsch](../de/ansagen.md)

TalkOps uses audio for voicemail greetings, Smart Attendant steps, queue
greetings and hold music. The audio picker appears inside the feature that
uses the prompt. There is no separate global prompt-library page.

## Before you start

Use a supported browser. Microphone recording requires permission and a secure
browser context, normally HTTPS. Local speech generation requires a running
media worker and its voice models. Own voicemail greetings follow mailbox access;
routing audio needs `routing.manage`, global hold music needs `settings.manage`,
and system voicemail texts need `voicemail.settings`.

## Generate speech

1. Open the greeting or audio control in the target feature.
2. Select **Generate**, then choose German or English and a voice.
3. Enter the text and set speech speed where offered.
4. Generate the audio and wait until it is ready.
5. Listen with the player and save the containing form.

The control offers two voices per language. A speech speed of `1.0` is normal;
the supported range is `0.5` to `2.0`. Changing text does not change an existing
clip until it is generated again. See [voicemail speech speed](voicemail.md#speech-speed)
for speed controls and inline text tags.

## Record or upload

1. Select **Record** and permit microphone access, or select **Upload**.
2. Stop the recording, or select an audio file the browser can decode.
3. Listen to the result and check volume and the start and end of the clip.
4. Save the containing form.

The browser converts recorded and uploaded audio to 16 kHz mono, 16-bit PCM WAV.
Selecting a file is not proof that the browser can decode its codec. If loading
fails, export the source as WAV and try again. **Default** and **None** appear
only where the target feature supports those choices.

## Select the correct use

| Use | Where to configure it |
| --- | --- |
| Personal mailbox greeting | Voicemail; see [voicemail](voicemail.md). |
| System voicemail voice and texts | Settings → Voicemail. Preserve required placeholders. |
| Attendant greeting or spoken step | Settings → Call routing → Smart Attendant. |
| Queue greeting and music | The queue editor; see [call routing](call-routing.md). |
| Global hold music | Settings → Call handling. Select a built-in track or own audio. |
| Phone ringtone or wallpaper | Settings → Phones; see [Yealink](yealink.md). |

## Verify and troubleshoot

Call the affected test destination and listen to the saved prompt. Confirm
language, volume and the next routing action. Browser playback alone does not
verify that the audio is assigned to the intended call flow.

- Generation pending or failed: check worker health and logs; allow the model download to finish.
- Microphone blocked: check HTTPS, browser permissions and the selected input.
- Old text still plays: generate again and save the containing form.
- Built-in music unavailable: confirm FreeSWITCH has started and provided its tracks.

Use [troubleshooting](troubleshooting.md) for service checks and
[call routing](call-routing.md) for the action that follows the prompt.
