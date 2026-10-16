# 0030 - Transcription jobs: progress, pause, resume and manual runs

- Status: Accepted
- Date: 2026-10-15
- Amends: [ADR 0005](0005-job-queue-postgres.md),
  [ADR 0024](0024-transcription-engines-per-purpose.md)

## Context

Users want to transcribe a recording again with a selected model, with a
time estimate. Users with the permission want a page with the running and
waiting transcriptions: progress in percent, the remaining time, pause,
resume and stop, and the resource usage of the media worker. A paused job
must continue at the paused position if possible.

whisper.cpp has no checkpoint. A running `whisper-cli` process can only
continue if it is frozen (SIGSTOP). A frozen process keeps about 2 GB of
memory and blocks the transcription lane.

## Decision

- **Job states**: `job_status` gets `paused` and `cancelled`. The `jobs`
  table gets `progress`, `started_at`, `run_started_at`,
  `run_progress_start`, `requested_by`, `control` and `checkpoint`.
- **Control**: the API changes queued and paused jobs at once. For a
  running job it sets `control` (`pause` or `stop`). The worker reports the
  progress every second (`jobs::report_progress`), which also renews the
  lease, and reads `control` in the same statement.
- **Progress**: the worker starts `whisper-cli` with `-pp` and reads the
  progress from its log. The progress of each channel is weighted by its
  length.
- **Resume at the paused position**: the worker reads the word segments
  that `whisper-cli` prints while it runs. On pause, the worker stops the
  process and stores a checkpoint: the words of the finished channels, the
  words of the current channel and the time of the last word. On resume, the
  worker transcribes only the rest of the current channel (a shortened WAV
  file), moves its times by the offset and joins the words. A checkpoint is
  only used with the same engine. A cloud request has no partial result; a
  paused cloud job starts again.
- **Manual runs**: `POST /recordings/{id}/transcribe` with an engine
  choice (local model or cloud connection and model) adds one final pass.
  The current transcript stays until the new one is saved. Only one manual
  job of a recording can wait or run.
- **Estimates**: the table `transcription_speed` keeps the processing time
  for each second of recording for each engine key (`local:fast`,
  `cloud:<model>`). The value is the mean of the first four runs, then an
  exponential average. Resumed runs are not measured. Without measurements
  the API uses fixed defaults. The ETA of a running job comes from its own
  progress rate; queued jobs add the jobs before them.
- **Resources**: each worker writes CPU time and memory of its process tree,
  the container memory (cgroup v2, else `/proc/meminfo`) and the load to
  `worker_status` every 5 seconds.
- **Permissions** (ADR 0029): `recordings.retranscribe`,
  `transcriptions.view` (own jobs: started by the user or of the user's
  extensions) and `transcriptions.manage` (all jobs and the resources). The
  built-in roles other than admin do not get them by default.

## Consequences

- Positive: users see what the worker does and can control long jobs. A
  pause frees CPU and memory at once.
- Positive: the estimates adapt to the hardware.
- Negative: the words before a pause have word times from the segment
  output, not from the token times. Their turn detection can be less exact.
- Negative: Whisper recognizes the words near the resume point with less
  context. Single words there can differ from a run without a pause. A
  check with whisper.cpp v1.9.4, model `base`, a 41 s speech sample and a
  pause at 19 s gave the same text except for a few words after the cut,
  with and without VAD (ignored test
  `real_whisper_resumes_at_the_paused_position`).
- Negative: a stop of an automatic first pass leaves the recording without
  a transcript (`none`); a voicemail e-mail is sent without a transcript.
