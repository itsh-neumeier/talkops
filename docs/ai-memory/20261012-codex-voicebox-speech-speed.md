# Voicebox speech speed and spoken message counts

## Task state

- Task ID: 20261012-codex-voicebox-speech-speed
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: In progress. Implementation complete; validation and PR review pending.
- Branch: codex/voicebox-speech-speed
- Base commit: 2fbe3195517b509d2afd3d57247c05a98437f064
- Related commits or PR: Pending

## Goal

Set the TTS speed for each voicebox and each prompt. Support text such as
`<speed:0.7>Mustertext</speed>`. Use spoken words for message counts.
The user required proposal review before application changes. The user approved
the proposal in this chat before implementation started.

## Result

Implemented box speed, per-prompt speed, inline tags, shared clip previews,
and spoken count sequences. Added database, parser, call-flow, and audio tests.
Added English and German user documentation and an implementation document.
Publication and Linux validation are pending.

The remote default target is `ccr-df1c8083-0ce1he`. Fetch confirmed the base
commit. The original checkout was clean. Created a separate local worktree at
`C:/Users/timo/.codex/worktrees/voicebox-speech-speed/talkops` and the task branch.
The app could not attach the checkout. Git can use the worktree.

## Changes

- `crates/talkops-core/src/speech.rs`: Shared tag parser and speed validation.
- `crates/talkops-core/src/{audio,voicemail,voicemail_config,prompts}.rs`:
  Store speed and select content-addressed audio. Add spoken count fragments.
- `crates/talkops-media-worker/src/{main,tts,tests}.rs`: Generate speed groups,
  join PCM samples, refresh stored announcements, and test worker behavior.
- `crates/talkops-api/src/routes/{audio,voicemail}.rs` and voicemail flows:
  Expose speed, select current audio, and announce position and total.
- `web/src/lib/components/`: Add speed controls and generated clip previews.
- `web/src/lib/i18n/`: Add English and German UI text.
- `docs/en/voicemail.md`, `docs/de/voicemail.md`, and
  `docs/voicebox-speech-speed.md`: User and implementation documentation.

## Decisions

The user approved the scope below. Additional implementation decisions follow it.

- Add a voicebox speed setting and optional prompt speed overrides.
- Define speed as a factor: 1.0 keeps the current voice speed; 0.7 is slower.
  Proposed range: 0.5 to 2.0. An explicit text tag overrides the prompt speed.
  A prompt speed overrides the voicebox speed.
- Add speed and tag help to the shared TTS clip editor. Its preview uses the
  selected speed. Tags use absolute factors. Reject malformed or nested tags.
- Parse tags before Piper receives the text. Render each speed segment with
  Piper and join compatible WAV samples with the existing hound library.
  The pinned Piper binary accepts a process-level length scale, not a
  per-line JSON length scale. Preserve the model's default at speed 1.0.
- Include speed in generated prompt cache keys. Refresh affected generated
  announcements after settings changes. Preserve recordings and uploaded clips.
- For two new messages out of four total, propose: "2 von 4 Sprachnachrichten
  sind neu." During playback, propose: "Nachricht 2 von 4."
  Keep separate correct forms for empty, singular, and all-new cases.
- The current count flow has no literal slash. It speaks new and saved counts.
  The playback flow currently announces only the message position.
- Use separate hashed files for legacy TTS greeting generations as well.
  Preserve the old greeting path as a fallback for existing normal-speed audio.
- Keep box speed when an old API client omits the field. Keep prompt overrides
  of 1.0: they must override a slower box speed rather than disappear.
- Copy generated shared clips inside the box transaction. Keep uploaded and
  recorded clips unchanged.
- Use original stored timestamps when refreshing caller/date announcements.

## Checks

- Passed: git status, branch and worktree inventory, remote HEAD lookup,
  git fetch origin, base commit check, and open PR inventory with gh.
- Passed: Inspected pinned Piper 2023.11.14-2 upstream main.cpp.
- Passed: Initial web build and corrected Svelte check (0 errors, 0 warnings).
- Failed, corrected: Initial Svelte check used an existing playback translation
  key for TTS speed. Added a separate `audio.speechSpeed` key.
- Failed, environment: Local cargo check cannot build ring because gcc.exe is
  absent. Docker's Linux engine is unavailable; no WSL distribution is installed.
- Failed, environment: Initial whole-web lint reported CRLF line endings from
  the Windows checkout. Normalized local web text to LF and repeated the check.
- Not run yet: Full Linux Rust, database, audio, image, and E2E checks.
- External CodeRabbit review: CLI unavailable. Manual diff review in progress.
- STE review: Used the repository writing rules. The full official standard
  was not available. Full language review is pending.

## Dependencies and open work

- No feature PR dependency found. PR 1 tracks the release batch.
- Migration ID: `20261026000001_voicebox_speech_speed`. Run it before the updated
  server and worker. No new runtime dependency or voice model is required.
- Tests must cover speed validation, tag parsing, speed precedence, old data,
  cache changes, audio assembly, API storage, and spoken message sequences.
- Add English user documentation and German translations.
- Commit, push, and PR creation are authorized after the reviewed changes are
  complete. The release chat owns merges, version changes, tags, and publication.

## Next action

Finish local web checks. Commit and push the branch. Create a draft PR and
check the Linux CI results. Resolve task failures, then mark the PR ready.

## References

- [Shared workflow summary](20261012-codex-shared-ai-workflow.md)
- [Release workflow](../release-workflow.md)
- [Voicemail ADR](../adr/0010-voicemail-und-sprachausgabe.md)
- [Audio clip ADR](../adr/0017-audio-clips-und-smart-attendant.md)
- [Pinned Piper source](https://github.com/rhasspy/piper/blob/2023.11.14-2/src/cpp/main.cpp)
