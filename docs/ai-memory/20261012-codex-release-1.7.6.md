# TalkOps 1.7.6 release

## Task state

- Task ID: 20261012-codex-release-1.7.6
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: In progress. Target merged and tagged; release verification pending.
- Branch: codex/release-initial-batch
- Base commit: 2fbe3195517b509d2afd3d57247c05a98437f064
- Related PR: [Integration PR #1](https://github.com/itsh-neumeier/talkops/pull/1)

## Goal

Release PR #1 and PR #2 as TalkOps 1.7.6. The user authorized commits, pushes,
merges and publication. The user confirmed the completed voicebox handoff and
asked the release manager to continue.

## Result

Updated the existing release worktree from the current remote target.
Preserved PR #1's original head `017476afdb32f6bc03b30c31c87ce789ca98a860`.
Merged PR #2 final head `9a844f38bfc8b7486fc8579c941b8dcc668385a6` with source history.
Prepared version 1.7.6 and English release notes. PR #1 and PR #2 are merged.
Release commit: `4503fb3b96167aa77320cc936597fc6147b392c3`.
Annotated tag `v1.7.6` resolves to that exact commit. The Release workflow is
running. Published artifacts have not yet passed final verification.

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
- PR #2 is ready. CI and full E2E passed on its final head.
- New draft softphone PR #3 is outside the agreed scope of PR #1 and PR #2.
- No new Claude contribution is assigned. Do not infer hidden chat work.
- Preserve both source histories. No force-push or foreign uncommitted files.

## Checks

- Passed: Rust format, whitespace, source ancestry and version metadata diffs.
- Passed: 29 unique migration IDs; released migrations remain unchanged.
- Passed: DE/EN catalogs contain the same 1,030 keys with no duplicate keys.
- Passed source CI/E2E at `9a844f3`: Actions runs `38256624961` and
  `38256624940`. The final source adds a real-Piper speed and WAV test.
- Passed final integration CI/E2E at `0e776a0`: runs `38258187142` and
  `38258187128`. All push checks also passed.
- Passed target CI/E2E at `4503fb3`: runs `38259479755` and `38259479919`.
  Server and worker version smoke output reports 1.7.6. The target tree equals
  the final integration candidate tree.
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
- Re-fetch PR #2 if its head changes. The development handoff is complete.
- Integration and target gates passed. Tag `v1.7.6` was new and pushed once.
- Verify Release workflow, GitHub release, all three multi-architecture images,
  moving tags and runtime versions where executable.
- Release workflow: `38260786484`. Final publication checks remain pending.
- No background automation was created.

## Next action

Verify Release workflow `38260786484`, release `v1.7.6` and registry images.
Save the final result in PR #1 and this summary. Keep the tag immutable.

## References

- [Release ledger](../releases/1.7.6.md)
- [Codex contribution](https://github.com/itsh-neumeier/talkops/pull/2)
- [Source summary](20261012-codex-voicebox-speech-speed.md)
- [Release workflow](../release-workflow.md)
