# Release intake for requested 1.8.12.1

## Task state

- Task ID: 20261027-codex-release-1.8.12.1
- Date: 2026-10-27, Europe/Berlin
- Tool: Codex
- Status: In progress; version decision pending
- Branch: codex/release-1.8.12.1
- Base commit: 3603ed56b0073a1dd23646622cf4fb0284644ab7
- Related PR: https://github.com/itsh-neumeier/talkops/pull/87

## Goal

Integrate all ready open PRs and publish the next release requested as 1.8.12.1.

## Result

Merged the completed 1.8.12 audit and Codex PR86 without rewriting source history.
No release version, main branch or published tag changed. No release published.

## Decisions

- PR86 head: 4cf7a394dc2090d2734c279e5b7cbf6de3f23739.
  Correct call-map marker overlap with accessible bundles that split on zoom.
  Its dependency PR84 is already in main. No migration or dependency changes.
- Claude Code PR61 remains a draft proposal, with no implementation or release
  handoff. Preserve it. No ready Claude Code PR is assigned to this batch.
- Requested 1.8.12.1 has four numeric components. Cargo and the existing
  SemVer image metadata contract require a valid three-component version.
  Asked the user to choose stable patch 1.8.13 or preparation only. Await answer
  before changing release versions, merging main or publishing.
- Default target main and merge-commit method verified. No protection/rulesets
  at intake. Latest stable release is 1.8.12 on the recorded base.
- Primary and all foreign worktrees stay untouched.

## Checks

- Source PR86: all 22 required CI/E2E contexts succeeded. Two branch-only
  trial publication contexts skipped as designed. Source push CI40568968803,
  E2E40568968726; PR CI40568977136, E2E40568977121.
- Local docs: 13 tests passed; Wiki check passed, 75 pages and 688 links.
- Manual review: collision checks include labels and focus outlines, merged
  centroids are checked again, member lists retain access at equal positions
  and maximum zoom. Existing API IDs, scope and filters stay unchanged.
- Both DE/EN locale changes and handbook instructions inspected.
- No SQL migration, Cargo.lock or npm lock change in the source PR.
- Local web: 91 unit tests, lint, type check (zero errors/warnings) and build passed.
  Initial lint failed on Windows checkout line endings. Normalized the seven
  affected files to LF; Git proves no source-content change. Combined CI/E2E
  started on intake revision 932acee; not final versioned release evidence.
- CodeRabbit CLI is absent. No CodeRabbit result claimed; manual review used.
- A local SemVer module probe failed because that optional module was absent.
  No source change resulted. Full STE standard unavailable; language review pending.

## Dependencies and open work

Confirm a valid target version. Draft61 is excluded under the established rule.
Coral remains experimental and off by default; physical acceptance is separate.

## Next action

Receive the version decision; update metadata, validate the final candidate,
merge, validate exact main, tag once, then verify the workflow and all images.

## References

- https://github.com/itsh-neumeier/talkops/pull/86
- https://github.com/itsh-neumeier/talkops/pull/61
- docs/ai-memory/20261027-codex-call-map-clusters.md
- docs/ai-memory/20261027-codex-release-1.8.12.md
- docs/release-workflow.md
