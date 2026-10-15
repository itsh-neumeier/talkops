# Transcription jobs

## Transcribe a recording again

1. Open the **Call log** and click the **Recording** button of a call.
2. Click **Transcribe again …**.
3. Select a model. The dialog shows for each model:
   - the estimated processing time (**measured** on this server or
     **estimated** from defaults),
   - the estimated cost of a cloud model,
   - the wait for the jobs before it in the queue.
4. Click **Start**.

The current transcript stays visible until the new transcript is complete.
Only one manual transcription of a recording can wait or run at a time.
You need the permission **Transcribe again** and access to the recording.

## Transcriptions page

The menu entry **Transcriptions** shows the transcription jobs. You need the
permission **Own transcription jobs** (jobs that you started or of your own
extensions) or **All transcription jobs**.

The tab **Running and waiting** shows for each job:

- the call, the length of the audio and the model,
- the state: running, waiting, paused, pausing or stopping,
- a progress bar with the percent value,
- the remaining time (running) or the expected end (waiting).

Buttons:

| Button | Effect |
| --- | --- |
| Pause | A waiting job stays in the queue. A running local job stops within about one second and keeps its position. |
| Resume | The job goes back into the queue. A local job continues at the paused position. A cloud job starts again. |
| Stop | The job ends. A manual or second pass keeps the current transcript. A stopped first pass leaves no transcript; a voicemail e-mail is sent without it. |
| Move up | The waiting job goes to the front of the queue. |

The tab **History** shows the jobs of the last 7 days that finished, failed
or stopped.

## Resources

With the permission **All transcription jobs**, the page also shows each
media worker:

- **CPU**: the CPU time of the worker and its Whisper processes. 100 % is one
  core.
- **Memory**: the memory use and limit of the container (or of the host).
- **Worker and Whisper**: the memory of the worker processes.
- **Load**: the load average of the last minute.

The page updates every 2 seconds.

## Estimates

The worker measures each complete run: processing time for each second of
recording, for each model. The estimate uses the mean of the first runs and
then follows changes slowly. Resumed runs are not measured.

## API

- `GET /api/v1/recordings/{id}/transcribe`: models with estimates.
- `POST /api/v1/recordings/{id}/transcribe` with
  `{"engine": {"kind": "local", "model": "accurate"}}` or
  `{"engine": {"kind": "cloud", "provider_id": "…", "model": "gpt-transcribe"}}`.
- `GET /api/v1/transcription-jobs` (`?history=true` for the history).
- `POST /api/v1/transcription-jobs/{id}` with
  `{"action": "pause" | "resume" | "stop" | "prioritize"}`.
- `GET /api/v1/transcription-workers`: resource usage.
