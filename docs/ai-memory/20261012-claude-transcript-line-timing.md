# Transcript line timing

## Task state

- Task ID: 20261012-claude-transcript-line-timing
- Date: 2026-10-12
- Tool: Claude Code
- Status: In progress
- Branch: claude/vibrant-allen-8j9w71
- Base commit: 4503fb3 (release 1.7.6, remote default branch `ccr-df1c8083-0ce1he`)
- Related commits or PR: Pending

## Goal

Transcript lines must have correct times and a correct order of both
parties. The user supplied a 17-minute stereo call. TalkOps showed blocks of
30 to 45 seconds for one party, before the answers of the other party.

## Result

Local work: the media worker makes short lines from word times and merges
both channels by time. Validation with the sample call is in progress.

## Changes

- `crates/talkops-media-worker/src/utterances.rs`: new line builder from
  timed words, marker removal and removal of overlapping repetitions.
- `crates/talkops-media-worker/src/transcribe.rs`: whisper-cli gets
  `-ml 1 -sow` (one segment per word); the words go to the line builder.
- `crates/talkops-media-worker/src/api.rs`: the API path asks for word
  timestamps and falls back to segments when the server rejects them.
- `docs/en/recording.md`, `docs/de/aufzeichnung.md`: line rules.

## Decisions

- Cause: whisper.cpp VAD joins speech parts with 0.1 s of silence. Whisper
  then decodes up to 30 s as one segment. Only segment times are mapped back
  to the recording, token times are not (whisper.cpp v1.9.4,
  `whisper_full_get_segment_t0_from_state`).
- `-ml 1 -sow` makes each word a segment, so whisper.cpp maps each word
  time back. No new binary and no DTW (DTW turns off flash attention).
- Line rules: new line at a pause of 0.8 s, at a sentence end with 0.4 s
  pause or after 4 s, when the other party speaks in the gap, from 6 s at a
  comma or short pause, at the latest after 10 s. The user agreed to about
  10 s.
- Repetitions are dropped only when the same text of the same party
  overlaps in time. Real repetitions of speakers stay.
- No migration, no API change and no UI change. Existing transcripts stay.

## Checks

- Passed: `cargo test -p talkops-media-worker` with Postgres (23 tests).
- Passed: `cargo clippy -p talkops-media-worker --all-targets -- -D warnings`.
- Not run yet: comparison with the sample call, full CI command set.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- None. Migration IDs: none.

## Next action

Compare old and new output for the sample call, tune the rules, open the PR.

## References

- `docs/adr/0019-transkription-in-zwei-durchgaengen-und-ki-api.md`
