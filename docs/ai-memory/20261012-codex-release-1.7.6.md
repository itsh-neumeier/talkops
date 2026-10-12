# TalkOps 1.7.6 release

## Task state

- Task ID: 20261012-codex-release-1.7.6
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: In progress. Candidate prepared; developer handoff and CI pending.
- Branch: codex/release-initial-batch
- Base commit: 2fbe3195517b509d2afd3d57247c05a98437f064
- Related PR: [Integration PR #1](https://github.com/itsh-neumeier/talkops/pull/1)

## Goal

Release PR #1 and PR #2 as TalkOps 1.7.6. The user authorized commits, pushes,
merges and publication. A later message said the user will report when the
development agent finishes. Wait for the completed handoff before target merge.

## Result

Updated the existing release worktree from the current remote target.
Preserved PR #1's original head `017476afdb32f6bc03b30c31c87ce789ca98a860`.
Merged PR #2 head `8ae7edf7c0849318ae7ef5f61aa4609552c90a28` with source history.
Prepared version 1.7.6 and English release notes. No target merge or publication.

## Changes

- `Cargo.toml`, `Cargo.lock`, `web/package.json`, `web/package-lock.json`:
  Set release versions to 1.7.6 without dependency upgrades.
- `CHANGELOG.md`: Describe speed controls, spoken totals and shared workflow.
- `docs/releases/1.7.6.md`: Record agreed scope, source heads and release gates.
- This summary: Save release-manager state without editing the developer's memory.

## Decisions

- Use the requested 1.7.6 version. Record the explicit choice for these
  compatible feature additions rather than silently choosing 1.8.0.
- Keep integration PR #1 and branch `codex/release-initial-batch`.
- PR #2 is draft while Linux CI and its developer review are pending.
- No new Claude contribution is assigned. Do not infer hidden chat work.
- Preserve both source histories. No force-push or foreign uncommitted files.

## Checks

- Passed: Rust format, whitespace, source ancestry and version metadata diffs.
- Passed: 29 unique migration IDs; released migrations remain unchanged.
- Passed: DE/EN catalogs contain the same 1,030 keys with no duplicate keys.
- Manual integration review: Checked API/UI speed fields, defaults, parser,
  prompt and greeting cache identities, WAV assembly and test coverage.
- Not run locally: Full Linux CI/E2E; Docker Linux and services unavailable.
- Not run: CodeRabbit CLI review; CLI unavailable.
- Failed then corrected: Git merge file writes failed with Windows file caching
  and automatic CRLF conversion. Restored only this worktree's failed-merge
  files. The merge succeeded with `core.fscache=false` and `core.autocrlf=false`.
- STE review: Applied repository writing rules. The full official standard
  was unavailable; full language review is pending.

## Dependencies and open work

- Migration: 20261026000001_voicebox_speech_speed. Update server and worker together.
- Wait for the completed development handoff. Re-fetch PR #2 if its head changes.
- Verify the final integration CI/E2E, merge PR #1, then verify target CI/E2E.
- Push unused tag `v1.7.6` only on the verified target commit.
- Verify Release workflow, GitHub release, all three multi-architecture images,
  moving tags and runtime versions where executable.
- No tag, release or background automation was created.

## Next action

Record candidate CI links in PR #1. Continue after the development handoff.
Use the PR body and current GitHub checks to recover the latest state.

## References

- [Release ledger](../releases/1.7.6.md)
- [Codex contribution](https://github.com/itsh-neumeier/talkops/pull/2)
- [Source summary](20261012-codex-voicebox-speech-speed.md)
- [Release workflow](../release-workflow.md)
