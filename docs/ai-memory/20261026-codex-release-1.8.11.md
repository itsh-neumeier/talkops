# TalkOps 1.8.11 release

## Task state

- Task ID: 20261026-codex-release-1.8.11
- Date: 2026-10-26, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.11 (merged and removed)
- Base commit: 8f32ec7778ec7a147c09ac98c97c5f1a434e22cc
- Related PR: https://github.com/itsh-neumeier/talkops/pull/83

## Goal

Integrate all ready PRs and publish the user-selected version 1.8.11.
Preserve source histories, foreign worktrees and unfinished contributions.

## Result

Integrated Claude Code PR80 without conflicts. No ready Codex contribution.
No selected contribution was already in main. Source:
7fcf9af122b1be85080407502e0142a243cb0ed7, claude/wonderful-bohr-tmurvt.
Source CI/E2E has 16 successful checks. No review threads or change requests.
Retain the separate verified 1.8.10 audit through a merge before PR80.
The verified release is published. See the final results below.

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
- Final CI supplied Linux/Rust/services, image smoke and SIPp/browser results.
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

None for this release. Draft PR61/81/82 and their hardware/quality work remain separate.
The final task-only record is archived under audit/release-1.8.11.

## References

- https://github.com/itsh-neumeier/talkops/pull/80
- https://github.com/itsh-neumeier/talkops/pull/81
- https://github.com/itsh-neumeier/talkops/pull/82
- docs/ai-memory/20261024-claude-panel-layout-footer.md
- docs/ai-memory/20261024-codex-release-1.8.10.md
- docs/release-workflow.md

## Candidate correction

Initial candidate f7ccac93138565d0e061e787951f4d8d87b2d92c is superseded.
Push CI40359624923 failed in side-panel.mjs: the initial generic card locator
could match the root skeleton before authentication completed. The card was
removed before boundingBox, so the before width was null. The parallel PR web
job passed, confirming the timing-sensitive path. This failed gate is not waived.
Wait for a loaded data-panel link and select its containing card before measuring.
Keep the overlap and full-width assertions; add a diagnostic null check.
No product change. Node syntax and diff checks passed. The corrected candidate received new complete CI/E2E results before merge.
## Final integration and publication

Initial candidate f7ccac93138565d0e061e787951f4d8d87b2d92c failed push
CI40359624923: the browser width test measured a temporary skeleton card.
The integration correction waits for the loaded list and keeps all overlap
and full-width assertions. The corrected final candidate passed all checks.

- Final candidate: ac34eaee4abf44d1dbef0f6e96a5c4c6172515ec. All 16 checks passed:
  PR CI40361524234/E2E40361524170, push CI40361517433/E2E40361517325.
- PR E2E attempt1 was cancelled during very slow Ubuntu package downloads in
  Playwright setup. Logs show no test failure. Complete attempt2 passed.
  The initial superseded browser failure and this retry remain recorded.
- Integration PR83 and source PR80 merged. Exact-main target: f8bdfb37cf3f7705d7fbbaf63d6bd6148caba400.
  Its tree equals the candidate. Source 7fcf9af122b1be85080407502e0142a243cb0ed7
  is an ancestor. No source history rewritten or ready contribution omitted.
- Exact-main CI40390896849 and E2E40390896741 passed before tag push.
  Rust: 513 passed, one existing ignored model test. Web: 83 tests and all
  23 browser suites. Docs: 13 tests. DB/LDAP, recovery, image/storage/version
  smoke, G.711/T.38 received fax PDFs and SIPp/browser softphone passed.
  Server and worker displayed 1.8.11. No final gate waived.
- Annotated v1.8.11 pushed once on the exact target. Tag object: 00fec99a23de1f0ce4145170490cd4803231ea45.
  No published tag moved. Release workflow 40393450794 passed.
- Stable GitHub release published at 2026-10-26T20:20:27Z. Additional tag CI 40393450593,
  E2E 40393450582 and FreeSWITCH 40393450824 passed.
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.11

## Registry verification

Docker buildx imagetools verified three versioned manifests and six configs.
Every OCI version label equals v1.8.11; every revision label equals the target.
Tags 1.8/latest have the same platform digests as 1.8.11.

| Image | Platform | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:ebb6756e637f0a2c5e21da0b07847ebd8b6df7d34d158647c16f168a144acf0d |
| talkops-server | arm64 | sha256:b92c4ae271624a464b50eb8d2a8b273126825977a3a54efe30c4dede0eb70054 |
| talkops-media-worker | amd64 | sha256:7ee80696d3dd1e154196895a1931254549cb6690eab909e2fb8fba89b69915f9 |
| talkops-media-worker | arm64 | sha256:743ab898b608b428b48abb1c16c27d1e96dd646cf9d7eab56237c6c055014082 |
| talkops-freeswitch | amd64 | sha256:8f45f4b33de6bd0ebedb4c181626ab78d8c26b6f28c56025849430cf045332d0 |
| talkops-freeswitch | arm64 | sha256:1f2b793b78004bef25dfe72ffe6d2d6c0991321ceec3dfaa61bbd97d7d1c7d52 |

## Cleanup and audit

Removed unchanged merged source branch claude/wonderful-bohr-tmurvt and
codex/release-1.8.11 with atomic expected-head guards. Preserve draft PR61/81/82
branches, claude/nifty-babbage-ymchlw, claude/charming-bell-fh6q3y and the
closed unmerged PR36 branch claude/vigilant-tesla-ef0027. Foreign worktrees
and primary files untouched. Only this task record differs from the released
commit in the separate audit. Do not push the merged release branch again.
