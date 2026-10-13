# TalkOps 1.7.6 release

## Task state

- Task ID: 20261012-codex-release-1.7.6
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: Completed. Release and image verification passed.
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
Annotated tag `v1.7.6` resolves to that exact commit. The Release workflow passed.
[TalkOps 1.7.6](https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.6)
is published with the English changelog and upgrade notes.
All three versioned images contain amd64 and arm64. Moving tags `1.7` and
`latest` select the same architecture manifests. All six image configurations
identify revision `4503fb3` and version `v1.7.6`.

The final audit commits are saved on the integration branch after publication.
They change documentation only. The published tag and target commit remain
unchanged. PR #1 holds the final release result and audit commit identifier.

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
- Passed Release workflow `38260786484`: version check, all six architecture
  builds, three manifest jobs and GitHub release creation.
- Passed tag CI, E2E and FreeSWITCH checks: runs `38260786507`, `38260786504`
  and `38260786467`.
- Passed registry reads: all versioned and moving-tag platforms and all six
  image revision/version labels match the checked release.
- Not run locally: A deployed browser version check or execution of the
  published arm64 images. Docker Linux is unavailable. CI version smoke tests
  report 1.7.6; the UI reads the server's version. The live UI was not observed.
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
- No release work remains. Both agreed PRs are merged and published.
- Integration and target gates passed. Tag `v1.7.6` was new and pushed once.
- Draft softphone PR #3 and guided backup work remain outside this release.
- No background automation was created.

## Next action

None for release 1.7.6. Keep the published tag immutable.

## References

- [Release ledger](../releases/1.7.6.md)
- [Codex contribution](https://github.com/itsh-neumeier/talkops/pull/2)
- [Source summary](20261012-codex-voicebox-speech-speed.md)
- [Release workflow](../release-workflow.md)
