# TalkOps 1.8.13 release (initial request 1.8.12.1)

## Task state

- Task ID: 20261027-codex-release-1.8.12.1
- Date: 2026-10-27, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.12.1
- Base commit: 3603ed56b0073a1dd23646622cf4fb0284644ab7
- Related PR: https://github.com/itsh-neumeier/talkops/pull/87

## Goal

Integrate all ready open PRs and publish the next release requested as 1.8.12.1.

## Result

Merged the completed 1.8.12 audit and Codex PR86 without rewriting source history.
Updated Cargo workspace and npm roots to 1.8.13. PR87 merged on 2026-10-28.
Exact main: 4b44cd68580908fb54407898aa01b0dcc4f80954. Stable v1.8.13 is published and verified.

## Decisions

- PR86 head: 4cf7a394dc2090d2734c279e5b7cbf6de3f23739.
  Correct call-map marker overlap with accessible bundles that split on zoom.
  Its dependency PR84 is already in main. No migration or dependency changes.
- Claude Code PR61 remains a draft proposal, with no implementation or release
  handoff. Preserve it. No ready Claude Code PR is assigned to this batch.
- Requested 1.8.12.1 has four numeric components. Cargo and the existing
  SemVer image metadata contract require a valid three-component version.
  On 2026-10-28 the user selected 1.8.13. Do not add fourth-component support.
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

Target version confirmed: 1.8.13. Draft61 is excluded under the established rule.
Coral remains experimental and off by default; physical acceptance is separate.

## Next action

None for this release. Preserve draft61 and all other unmerged work.

## References

- https://github.com/itsh-neumeier/talkops/pull/86
- https://github.com/itsh-neumeier/talkops/pull/61
- docs/ai-memory/20261027-codex-call-map-clusters.md
- docs/ai-memory/20261027-codex-release-1.8.12.md
- docs/release-workflow.md

## Final candidate and merge

- Candidate ab0291248f62fc321ab3cbae671aef2af25b5760: 22 required checks passed.
  Only two branch-only trial publication contexts skipped as designed.
- PR CI40636023198/E2E40636023232 and push CI40636017716/E2E40636017714 passed.
- Rust: 558 passed, two model/runtime tests ignored. Web: 91 unit tests and
  all browser checks passed. Server/worker/Coral smoke reported 1.8.13.
- Static metadata comparison: only eight workspace and npm root versions changed.
  All 59 migrations unchanged; 2,829 matching EN/DE keys and placeholders.
- Fresh premerge confirmed unchanged main, source head, scope and no protection/rulesets.
- Merge tree equals the tested candidate; source86 is an ancestor.
- Exact-main CI/E2E must pass before tag creation. No publication yet.

## Publication checkpoint

- Exact-main CI40638257180 and E2E40638257242 passed before publication.
- New annotated v1.8.13 was pushed once on 4b44cd68580908fb54407898aa01b0dcc4f80954.
- Tag object: bb676ce7596d88eb7ef17129a506f4174104f7e0. No published tag moved.
- Release workflow and registry verification pending. Do not report completion yet.

## Published artifact checkpoint

- Stable GitHub release published at 2026-10-28T06:01:09Z; Release40640523707 passed.
- Registry checks passed for all four images and eight platform configs.
  All version labels are v1.8.13 and revision labels match exact main.
  Tags 1.8 and latest have the same platform digests as 1.8.13.
- Tag CI40640523653 and FreeSWITCH40640523632 passed.
  Additional tag E2E40640523691 is running; no completion claim for that run.
- Curated English release notes published. Final audit and merged-branch cleanup pending.

## Verified publication

- Exact-main CI40638257180 and E2E40638257242 passed before the tag push.
- New annotated v1.8.13 points to exact main 4b44cd68580908fb54407898aa01b0dcc4f80954. No tag moved.
- Release workflow 40640523707, tag CI 40640523653, tag E2E 40640523691 and
  FreeSWITCH 40640523632 passed. Stable GitHub release published at 10/28/2026 06:01:09.
- Four manifests, eight platform configs and moving tags 1.8/latest verified.
  OCI version v1.8.13 and revision labels match the release commit.
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.13
- Removed only the two unchanged fully merged source/integration remote branches.
  Preserve draft61, unmerged branches and every foreign worktree. No deployment.
- Primary checkout remained at 397fc83 with the three untracked Dahua summaries.
- Physical Coral hardware/quality acceptance remains pending. Coral is opt-in.

| Image | Architecture | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:c9d4f7495a98babe386450afc256b979f7c0ae1985fed25d624696484efc3c5c |
| talkops-server | arm64 | sha256:f2a9bc3842caa05453407458a8dc78cfcfa5f22a600840f46351e536c93dabea |
| talkops-media-worker | amd64 | sha256:8afb9ce73d9af7c5d7e0fcff839aea6e0f241ef82fe12e206c2e94e0f3581cd5 |
| talkops-media-worker | arm64 | sha256:52de207369334309bd8ac3273e899e666d3b0f77698f4de12f229e76949778e7 |
| talkops-freeswitch | amd64 | sha256:9b6577d3c1d45e3553789e28f74ac0df67412c11384bb6ad83d776f82c429217 |
| talkops-freeswitch | arm64 | sha256:9033cf001aadeeaaadd84dee7727059bca1dddd8a2a42028d16d8ae816fcd6a3 |
| talkops-coral-stt | amd64 | sha256:1ca9ccd5a88c0c784bb45640f40b6ec7e0642ec6247529e8b45aab6a675a6d78 |
| talkops-coral-stt | arm64 | sha256:2c9c9d9a8f3ff35b6e8934f0364c8bcb2e22e5381a8762b3c26b6bc8e9319faf |
