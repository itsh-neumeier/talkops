# Transcript line timing

## Task state

- Task ID: 20261012-claude-transcript-line-timing
- Date: 2026-10-12
- Tool: Claude Code
- Status: Completed
- Branch: claude/vibrant-allen-8j9w71
- Base commit: 4503fb3 (release 1.7.6); `main` at 21f5a51 (release 1.7.7)
  merged in. The old target branch `ccr-df1c8083-0ce1he` was removed; the
  target branch is `main`.
- Related commits or PR: Pending

## Goal

Transcript lines must have correct times and a correct order of both
parties. The user supplied a 17-minute stereo call. TalkOps showed blocks of
30 to 45 seconds for one party, before the answers of the other party.

## Result

Local work and the pushed branch: the media worker makes short lines from
exact word times and merges both channels by time. The sample call was
transcribed with the old and the new code (whisper.cpp v1.9.4,
`large-v3-turbo-q5_0`, Silero VAD, 4 CPU threads).

- Old: 334 lines. One line of the called party covers 1.28 s to 90.12 s.
- New: about 460 lines, at most one line longer than 8 s. Example:
  "Habt ihr schon bestellt oder was?" is at 35.6 s (old: inside the block
  that starts at 1.28 s).
- Run time is the same: 1691 s old, 1661 s new for a 1026 s call.

## Changes

- `crates/talkops-media-worker/src/utterances.rs`: new line builder from
  timed words, marker removal and removal of overlapping repetitions.
- `crates/talkops-media-worker/src/transcribe.rs`: whisper-cli gets
  `-ml 1 -sow -ojf` and runs without `-np`. The worker reads the VAD time
  map (`vad_segment_info`) from the log and maps token times back to the
  recording. Without the map it uses segment times.
- `crates/talkops-media-worker/src/api.rs`: the API path asks for word
  timestamps (`timestamp_granularities[]=word`), gives each word the
  punctuation of its segment, and asks again without words when the
  server rejects them.
- `docs/en/recording.md`, `docs/de/aufzeichnung.md`: line rules.

## Decisions

- Cause: whisper.cpp VAD joins speech parts with 0.1 s of overlap and 0.1 s
  of silence. Whisper then decodes up to 30 s as one segment. whisper.cpp
  maps only segment times back to the recording, not token times
  (`whisper_full_get_segment_t0_from_state` in v1.9.4).
- With `-ml 1`, each word is a segment, but a word segment starts where
  the previous one ended. A word after a pause got the whole pause
  ("Habt" 14.77 s to 25.36 s). The token times are exact in the shortened
  audio, so the worker maps them with the logged VAD map.
- DTW (`-dtw large.v3.turbo`) gave no token times with VAD (`t_dtw` = -1).
  It is not used.
- Line rules: new line at a pause of 0.8 s; at a sentence end with 0.4 s
  pause or after 4 s; when the other party speaks in a pause of 0.5 s or
  after a comma; from 6 s at a comma or 0.25 s pause; at the latest after
  10 s. One or two words of a new sentence before a pause go to the next
  line. The user agreed to about 10 s.
- Repetitions are dropped only when the same text of the same party
  overlaps in time. Real repetitions of speakers stay.
- `-mc 0` is not used: the sample showed no repetition loops, and the
  context helps accuracy.
- The echo filter (proposal D) is not built: the sample has no cross-talk.
  Two pairs of similar lines are normal speech. A filter without a test
  case can delete real speech.
- No migration, no API change and no UI change. Existing transcripts stay.

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Passed: `cargo test --workspace --locked` with local PostgreSQL 16, also
  after the merge of `main` 21f5a51.
  The LDAP server of CI was not started; tests that need it were not
  checked.
- Passed: sample call, old and new code, results above.
- Not run: web checks (no web change), Docker image build, E2E (no change
  to images or call flow), real API servers for word timestamps.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- None. Migration IDs: none.
- The live transcription from 1.7.7 (`live.rs`) uses the same
  `Whisper::transcribe` and gets the new lines too.
- Possible later work: an echo filter if a recording with cross-talk shows
  the need.

## Next action

Release manager: review and integrate the PR.

## References

- `docs/adr/0019-transkription-in-zwei-durchgaengen-und-ki-api.md`
