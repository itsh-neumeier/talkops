# TalkOps 1.8.11 release

## Task state

- Task ID: 20261026-codex-release-1.8.11
- Date: 2026-10-26, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.11
- Base commit: 8f32ec7778ec7a147c09ac98c97c5f1a434e22cc
- Related PR: Pending

## Goal

Integrate all ready PRs and publish the user-selected version 1.8.11.
Preserve source histories, foreign worktrees and unfinished contributions.

## Result

Integrated Claude Code PR80 without conflicts. No ready Codex contribution.
No selected contribution was already in main. Source:
7fcf9af122b1be85080407502e0142a243cb0ed7, claude/wonderful-bohr-tmurvt.
Source CI/E2E has 16 successful checks. No review threads or change requests.
Retain the separate verified 1.8.10 audit through a merge before PR80.
No release tag pushed yet.

## Changes

- PR80: keep lists wide beside side panels, use the panel query container,
  keep settings-search results above content. Add fixed signed-in footer.
- GET /api/v1/system/load requires diagnostics.manage. The footer refreshes
  every 15 seconds while visible. Memory helpers move to talkops-core unchanged.
- Version Cargo workspace, seven workspace lock entries and npm roots with
  package tools. No dependency upgrades. Add English changelog and update
  active EN/DE installation, update, Portainer and reference examples.

## Decisions

- Target main, verified through the GitHub repository API. Merge commits permitted;
  no rulesets or branch protection. Existing release 1.8.10; v1.8.11 unused.
- User chose 1.8.11. The main scope is a layout correction with an additive
  diagnostics footer/API. No published contract removed or changed.
- Keep drafts excluded under the user's established ready-PR policy:
  PR61 16c6c45a61655a9ff215c350ac47d499d15dae9f, review-only proposal;
  PR81 3d11790618ed99c70fc5b7864c5be363e8a969d7, Coral hardware/quality,
  model license and real-host checks pending, proposed next minor;
  PR82 51e7df588ab028a435e79685ff43baeae5a52898, depends on PR81,
  physical Rust-service validation/quality gate pending, next feature release.
- No missing ready handoff. No unfinished Coral feature work in this batch.
- Preserve unmerged branches including claude/nifty-babbage-ymchlw,
  claude/charming-bell-fh6q3y and claude/vigilant-tesla-ef0027.

## Checks

- Passed locally: cargo fmt; 13 docs tests; Wiki check (75 pages, 672 links);
  four Compose variants; provisioning source hashes and both derivative checks.
- Static check: seven version-only Cargo lock changes; npm metadata only;
  all 56 released migrations unchanged, no new IDs; 2,776 matching DE/EN keys
  and placeholders; all 23 browser suites registered once; valid ADR links.
- Manual integration review: permission check before system sampling; API shape
  matches footer; no load request without permission or while hidden; timers
  removed on unmount; memory helper behavior unchanged; CSS panel/footer layers.
- Clean npm ci, lint, check (zero errors/warnings), 83 tests and build passed.
- Full Linux/Rust/services, image smoke and SIPp/browser stack: use final CI.
  No local Linux Docker daemon. No local browser suite claimed. The local preview
  launch was policy-blocked in the previous release; no bypass or retry here.
- CodeRabbit CLI unavailable; manual review used. Full STE standard unavailable;
  repository clarity rules applied, full language review pending.
- Two initial provisioning checks used nonexistent script names and failed before
  running. Correct build-native-library/build-function-catalog checks passed.

## Dependencies and open work

No new migration or package dependency. Update server and worker together.
CPU/load/uptime show the host; memory uses cgroup limits when present.
Initial CPU sample has no value. No live deployment, hardware acceptance or
Wiki publication claimed. Foreign primary HEAD and three untracked files preserved.

## Next action

Finish local web checks, publish integration PR, wait for its exact final checks,
merge and verify exact-main CI/E2E. Push unused v1.8.11 once and verify Release,
GitHub stable release, all three amd64/arm64 images, labels and 1.8/latest tags.
Record final results in the integration PR and separate audit.

## References

- https://github.com/itsh-neumeier/talkops/pull/80
- https://github.com/itsh-neumeier/talkops/pull/81
- https://github.com/itsh-neumeier/talkops/pull/82
- docs/ai-memory/20261024-claude-panel-layout-footer.md
- docs/ai-memory/20261024-codex-release-1.8.10.md
- docs/release-workflow.md
