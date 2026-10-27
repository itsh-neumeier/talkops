# Coral production trial and diagnostic handoff

Status: optional trial. The user owns the real production test. The quality
gate has not passed. Do not describe these images as a stable release.

## Images and data

The CI push workflow on `codex/coral-stable-preview` can publish three
linux/amd64 trial images after the Rust, web, documentation, tool and image
checks pass. The tag is `coral-test-` plus the first 12 characters of the
source commit. Use the exact tag from the successful workflow summary.
Never use a guessed tag or a failed workflow's commit.

The trial does not publish `latest`, `1.8` or a release tag. FreeSWITCH stays
at 1.8.11. No model weights or private audio are published in these images.
The operator must supply compiled models in `models/coral/de/` and optionally
`en/` and `langid/`. Each directory needs `model_edgetpu.tflite` and
`model.json`. Start the German trial with `coral-ctc-de` to isolate language
selection. Model hashes in the report identify the loaded bytes.

Before deployment, make a TalkOps backup and save the current Portainer
stack and environment. Two forward migrations are new relative to 1.8.11:
`20261112000001_coral_transcription` and
`20261112000002_coral_stable_preview`. A full rollback to old binaries needs
the matching database backup. Do not delete migration records or run a
down migration.

Use `docker-compose.yml` with `docker-compose.coral-test.yml`, or copy the
three overridden image references into the existing Portainer stack. Set:

```dotenv
TALKOPS_VERSION=1.8.11
TALKOPS_CORAL_TEST_TAG=coral-test-<verified-source-commit-prefix>
COMPOSE_PROFILES=turn,coral
TALKOPS_CORAL_PORT=8096
```

Keep the profiles that the existing installation uses. Port 8096 is an
example that avoids a TalkOps web listener at 8095. Set the Coral connection
URL to `http://127.0.0.1:8096/v1` for that port. Verify that the port is free.
Keep server, media worker and Coral on the same trial tag. The override has
no `build:` directive and is suitable for a prebuilt-image stack.

For PCIe/M.2, pass `/dev/apex_0` to `coral-stt`. Use the device's actual group
from `stat -c 'GID=%g mode=%a' /dev/apex_0` as `TALKOPS_CORAL_GID`. For a
device owned by GID 0 with mode 660, the supplemental group must be 0.
The service still runs as UID 10001. Do not add privileged mode. For USB,
retain the USB tree bind and character-device rule from the main Compose.

Deploy during a suitable maintenance window. This guide does not restart
or modify the production host automatically.

## Test procedure

1. Add or test the Coral connection in transcription settings.
2. Confirm a ready TPU and the expected model IDs.
3. Select Coral Realtime with `coral-ctc-de` for the live profile.
4. Enable **Publish complete words (experimental)** and save the engines.
5. Open **Settings → System → Diagnostics**.
6. Select the Coral connection and start a five-minute Coral capture.
7. Optionally enable conversation text before starting the capture.
8. Place a test call and open its live preview.
9. Speak on both sides, with short and long pauses and a longer sentence.
10. Note perceived delay, missing text, readability and any fallback notice.
11. Download the report while the preview is still open.
12. Stop the capture after the test.

Run a second call with complete-word mode off to compare behavior. Use the
same model and comparable speech. Do not infer WER from automatic text.
A quality measurement needs a corrected reference transcript.

The buffer stores at most 5,000 events or 2 MiB across the service. Older
events can be discarded. The report identifies discarded-event counts.
Capture stops automatically after the chosen 1–30 minutes. Events expire
after one hour. Restarting Coral loses the in-memory buffer. Preview cleanup
can remove the fallback snapshot, so export before closing the popup.
If Coral is unavailable, the report still downloads with the TalkOps state
and a `coral_service_error` field. Service-side events are then unavailable.

## Return these artifacts

- `talkops-coral-diagnostics.json` from the UI.
- The exact trial image tag and source commit from the CI summary.
- Test start time, call direction, spoken language and perceived delay.
- A description of missing text, word changes or fallback notices.
- Optional recording audio from the existing call-recording download.
- Optional corrected transcript, clearly identified as a human reference.

The report includes app/service versions and trial build revisions, worker
revision in session events, model hashes and shapes, platform,
TPU readiness/load, queue state, tenant profile settings, approximate
processing/first-output timings, errors and live fallback state. It does not
contain credentials or audio. Text is excluded by default. Text capture and
export both require explicit selection and all-live-transcript permission.
These timestamps are not human-aligned word latencies. WER is not calculated.

## Stop the trial

Select local Whisper in the transcription profiles and save. Close the
preview, then open it again. The optional Coral service can then be stopped.
This stops use of Coral without reverting database schema. To restore all
1.8.11 binaries, follow the normal backup restoration procedure with the
pre-trial database and stack. Preserve the downloaded diagnostic report.

Full ASD-STE100 language review is pending; the standard was unavailable.
