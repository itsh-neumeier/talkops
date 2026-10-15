# Transcribe again with a model choice and the transcription jobs page

## Task state

- Task ID: 20261015-claude-transcription-jobs
- Date: 2026-10-15, Europe/Berlin
- Tool: Claude Code
- Status: Completed. The PR records the readiness.
- Branch: claude/transcription-jobs
- Base commit: 1d9d28f (claude/role-permissions, itsh-neumeier/talkops#30)
  plus a merge of ef1139b (claude/gifted-rubin-my4qq1, itsh-neumeier/talkops#29)
- Related commits or PR: Pending

## Goal

PR 3 of 4 of the approved call log feedback plan (see
`20261015-claude-call-log-filters.md`):

- "Transcribe again" for a recording, with a popup to select the model and
  an estimated time.
- A page for users with the permission: running and waiting jobs with
  progress in percent and ETA, pause, resume, stop, and resource usage.
- User decisions: pause continues at the paused position if possible;
  users see their own jobs, managers see all jobs.

## Result

Implemented, tested and documented on the feature branch.

## Changes

- `migrations/20261101000002_transcription_jobs.sql`: `job_status` values
  `paused`, `cancelled`; job columns `progress`, `started_at`,
  `run_started_at`, `run_progress_start`, `requested_by`, `control`,
  `checkpoint`; tables `transcription_speed`, `worker_status`.
- `crates/talkops-core/src/jobs.rs`: new states, `Control`,
  `report_progress`, `pause_running`, `cancel_running`; claim and fail
  reset the new columns.
- `crates/talkops-core/src/transcription_jobs.rs`: `EngineChoice`,
  `Payload` (shared with the worker), `enqueue_manual`, `list` with
  `Scope`, `estimate`, `act`, `after_cancel`, speeds, worker status.
- `crates/talkops-core/src/roles.rs`: permissions
  `recordings.retranscribe`, `transcriptions.view`, `transcriptions.manage`
  (no defaults for operator/user).
- `crates/talkops-core/src/recordings.rs`: `transcription_stopped`.
- `crates/talkops-media-worker/src/transcribe.rs`: `transcribe_with`
  (progress from `-pp` and from the word lines, pause/stop, checkpoint,
  resume on a shortened WAV with time offset), `Control`, `Checkpoint`,
  `Outcome`, parsers for progress and word lines.
- `crates/talkops-media-worker/src/main.rs`: `JobEnd`, job monitor (1 s),
  `run_engine_controlled` (cloud: pause restarts), resource reports (5 s),
  manual passes (one final pass), speed measurement.
- `crates/talkops-media-worker/src/resources.rs`: CPU and memory of the
  process tree, cgroup v2 memory, load.
- `crates/talkops-api/src/routes/recordings.rs`: `GET/POST
  /recordings/{id}/transcribe`.
- `crates/talkops-api/src/routes/transcription_jobs.rs`:
  `/transcription-jobs`, `/transcription-jobs/{id}`, `/transcription-workers`.
- Web: `RetranscribeDialog.svelte`, `routes/transcriptions/+page.svelte`,
  menu entry, button in the call log, `lib/transcriptions/format.ts`,
  `.progress-running` animation, strings in `en.ts` and `de.ts`.
- Tests: core unit tests (estimates), worker DB test with a fake
  whisper-cli (pause, resume, stop), ignored real-whisper test, parser and
  resource tests, API tests `tests/transcription_jobs.rs`, web unit test
  `transcriptionFormat.test.ts`, browser test `transcriptions.mjs` (CI).
- Docs: `docs/adr/0030-transcription-jobs-control.md`,
  `docs/en/transcriptions.md`, `docs/de/transkriptionen.md`, links in
  recording docs and the permission table.

## Decisions

- Pause of a local run: stop the process, keep the words printed so far
  (stdout word lines, original times) and continue on a shortened WAV of
  the rest. No SIGSTOP (memory and lane stay blocked).
- Cloud runs: no partial result; pause and resume start the request again.
- The checkpoint is stored with its engine key; another engine ignores it.
- Pause and resume do not count as failed attempts.
- Monitor interval 1 s (progress, lease, control in one UPDATE).
- Estimates: speed table per engine key; resumed runs are not measured.
- Stop of an automatic first pass: transcript status `none` (or `done` if
  a transcript exists); a voicemail e-mail is sent without a transcript.
- PR 3 merges PR 1 so that the button is in the new call log.

## Checks

- Passed: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `DATABASE_URL=… cargo test --workspace` with PostgreSQL 16.
- Passed: ignored test `real_whisper_resumes_at_the_paused_position` with
  whisper.cpp v1.9.4 (built locally), model `base`, a 41 s speech sample
  (OpenAI micro-machines sample plus jfk.wav), pause at about 19 s, with and
  without VAD: same text except for a few words after the cut. With a
  sample of six repeated JFK sentences and VAD, Whisper itself dropped or
  added whole sentences after a cut (also when whisper-cli runs directly on
  the cut file); this is a model effect, not a merge error.
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`.
- Passed: browser checks `transcriptions.mjs`, `call-list.mjs`,
  `roles.mjs`, `layout.mjs`, `active-calls.mjs`,
  `transcription-settings.mjs`. `transcriptions.mjs` failed once after a
  cold preview start with a fixed 300 ms wait; replaced with polling, then
  5 of 5 runs passed.
- Not run: Docker/SIPp end-to-end test and a media worker container build.
- STE review: full standard not available; language review pending.

## Dependencies and open work

- Depends on itsh-neumeier/talkops#30 (roles) and #29 (call log); merge
  them first.
- Migration ID `20261101000002`, ADR 0030.
- Open: PR 4 (design review: skeleton loading, animations, fades).

## Next action

Release manager: integrate after #29 and #30. Claude Code: PR 4.

## References

- `docs/ai-memory/20261013-claude-transcription-engines.md`
- `docs/ai-memory/20261015-claude-roles-permissions.md`
- `docs/adr/0030-transcription-jobs-control.md`
