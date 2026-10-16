# Evaluate Cactus Whistle for local transcription

## Task state

- Task ID: 20261016-claude-cactus-whistle
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed. The result is a rejected ADR. No code changes.
- Branch: claude/vigilant-tesla-ef0027
- Base commit: 0f128ad210028b386081db0bdae90c210ff84523 (origin/main, release 1.7.14)
- Related commits or PR: Pending

## Goal

Find out if TalkOps can use the new speech-to-text model Cactus Whistle.
The user asked for a proposal and a review before any change.

## Result

- The proposal had three phases: phase 0 (a test), phase 1 (Whistle as an
  additional local model) and phase 2 (local stream mode). The user approved
  the proposal: a built binary is acceptable, phase 0 can run in the
  container, Whistle for all purposes, and phase 2 later after a question
  with a summary.
- Phase 0 ran. On German telephone audio, Whistle has 23.2 % WER. Whisper
  `base` has 17.1 % and `large-v3-turbo-q5_0` has 5.1 %. Whistle is not
  faster than `base` on x86, and it is much slower on long audio. English is
  equal to `base`.
- The user selected option A: do not add Whistle now. Record the result.
- `docs/adr/0033-cactus-whistle-evaluation.md` records the test setup, the
  results and the decision.

## Changes

- `docs/adr/0033-cactus-whistle-evaluation.md`: New ADR with status Rejected.
- `docs/adr/README.md`: Index row for ADR 0033.
- `docs/ai-memory/20261016-claude-cactus-whistle.md`: This summary.

## Decisions

- Whistle is not added (no code, image, migration or UI change). Reason:
  worse than Whisper `base` on German, no speed gain on x86.
- Phase 1 and phase 2 do not start.
- A new test is useful when Cactus releases a new Whistle model. The gate is:
  better than Whisper `base` on German telephone audio.

## Checks

- Passed: The `needle` binary ran without a network (`unshare -n`).
  `ldd` shows only libc, libm, libpthread and libdl. `strings` shows no
  HTTP client URL.
- Passed: Benchmark of Whistle, Whisper `base` and `large-v3-turbo-q5_0`
  with whisper.cpp v1.9.4. The ADR has the setup and the numbers.
- Not run: `large-v3-turbo-q5_0` on English, and German clips 23 to 29 with
  all engines. The turbo run was stopped because it was slow on 4 CPU cores.
  The German result was clear without these clips.
- Not run: `cargo`, `npm` and E2E checks. The task changes only Markdown
  files.
- STE review: Repository rules only. The full standard was unavailable;
  full language review is pending.

## Dependencies and open work

- Migration ID: None.
- Dependencies: None.
- Test method for a new test: Download clips with
  `https://datasets-server.huggingface.co/rows` (MLS `german` test,
  LibriSpeech `clean` test). Make a telephone copy with
  `ffmpeg -ar 8000 -acodec pcm_mulaw` and convert it back to 16 kHz PCM.
  Run `needle --model whistle.cact --audio <file> --audio-language <lang>`
  and `whisper-cli -m <model> -f <file> -l <lang> -t 4 -nt -np`. Calculate
  the WER without punctuation and capital letters.
- Useful facts for a later integration: The `needle` JSON output has
  `text`, `language` and, with `--audio-word-timestamps`, `words` with
  `word`, `start`, `end` and `probability`. `--audio-keywords <file>` reads
  one term on each line. `--audio-stream` prints one JSON line for each
  committed part, with `pending` text. The CLI reads PCM WAV files of any
  rate, but not mu-law.

## Next action

None. The release chat can merge the documentation PR.

## References

- ../adr/0033-cactus-whistle-evaluation.md
- ../adr/0024-transcription-engines-per-purpose.md
- 20261013-claude-transcription-engines.md
- https://huggingface.co/Cactus-Compute/whistle
