# Provisioning profiles and complete Yealink template research

## Task state

- Task ID: 20261016-codex-provisioning-multivendor-plan
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: In progress. Proposal prepared; application changes require user review.
- Branch: codex/provisioning-multivendor-plan
- Base commit: 86772ec0c31990f7a29a639ffb90db6ebef217e2
- Related PR: https://github.com/itsh-neumeier/talkops/pull/49 (draft)
- Initial proposal commit: 7610aa5

## Goal

Complete T41P key provisioning. Use the entire FusionPBX Yealink template
library as the source for adaptation. Prepare forms for more than one vendor.
Investigate T41P provisioning through DHCP option 66 and HTTPS.
The user explicitly requires a proposal before implementation.

## Result

Prepared a review proposal, a complete directory matrix and a source inventory.
Scanned all 433 files in all 70 Yealink directories at FusionPBX commit
be0d0e3220e6b3666775fa46db6f1accc60c79c6. Recorded 49 vendor function entries.
Created a local interactive T41P form proposal and a 70-directory source viewer.
No application, phone, DHCP or release setting changed.
Committed and pushed the proposal branch. Created draft PR 49 for review.

The worktree is C:/Users/timo/.codex/worktrees/provisioning-multivendor-plan/talkops.
The app worktree tool returned `Not a git repository` because the chat root is
the parent of the repository. Used `git worktree add` with an explicit branch.
Added only this new worktree path to Git safe.directory for its UNC metadata.
The original checkout has a local commit and three untracked Dahua summaries.
These foreign changes were preserved. No uncommitted changes were copied.
The remote target was verified as main at the base commit. No PR was open.

## Changes

- docs/provisioning-multivendor-proposal.md: Scope, findings, design, transport
  diagnosis, storage implications, proposed checks and release notes.
- docs/research/yealink-template-inventory.md: Matrix and individual sections
  for all 70 source directories, with pinned source links.
- docs/research/yealink-template-inventory.json: All 433 Git blobs, parameter
  patterns, first source lines, key loops, literal slots and comment references.
- docs/research/survey-yealink-templates.cjs: Reproducible, read-only source scan.
  Requires the pinned FusionPBX checkout. It does not execute upstream code.
- Local review preview outside the repository:
  C:/Users/timo/.codex/visualizations/2026/10/16/01a1466e-ddf1-7fa1-8f12-a6babaace19c/yealink-provisioning-review.html.

## Decisions

- Proposal only until user feedback. Commit/push/PR authorization permits
  publishing the proposal, but does not authorize the requested runtime changes
  before review. The release chat owns target merges and publication.
- PR 47 is already merged in 1.8.1. Reuse its line-key editor and 21 desk models.
- The T41P needs programmable keys and separate call-state softkey layouts.
  Map idle softkeys to IDs 1-4, arrows 5-8, OK/cancel 9/10, mute 13,
  redial 17 and message 18. Verify the profile on the actual firmware.
- The user reports T41P firmware 36.83.0.120. The screenshots show 13 controls.
- Templates contain copied blocks. T41P source has 27 literal line slots and
  expansion/memory blocks; these do not prove hardware capability. Some
  programmable fields incorrectly reference key 1 inside the loop.
- Numeric function IDs differ between the FusionPBX registry, template
  comments and current Yealink guidance. Use model/firmware profiles.
- Keep upstream Smarty sources separate from active MiniJinja templates.
  Plan an adapter interface, vendor/model capability forms, safe template
  cloning/versioning and explicit device provisioning endpoints.
- Both reported URLs used /provisioning/. The first chat spelling was a typo.
  Do not diagnose that typo as the incident cause.
- Manual LAN HTTP works on T41P. AX86R works with HTTPS through DHCP option 66.
  Compare device DHCP responses, auth and TLS. Next useful T41P test: manual
  HTTPS on the same endpoint. Do not assume the firmware lacks HTTPS support.
- DHCP option 66 is address delivery, not proof of a TFTP transfer.

## Checks

- Passed: Source scan assertions: pinned revision, 433 files, 70 directories,
  function registry and T41P programmable/call-state blocks.
- Passed: Independent comparison against all 433 original Git blobs. Checked
  3,779 parameter references, directory sections, file counts and local links.
- Passed: node --check docs/research/survey-yealink-templates.cjs.
- Passed: git diff --cached --check before the proposal commit.
- Passed: Interactive proposal in headless Edge/Playwright. Selected every
  source directory, checked 13 T41P programmable IDs, 15 line keys, conditional
  fields, line-position limits, draft changes and call-state selection.
- Passed: Preview at 320/736 px in light/dark appearance. No horizontal
  overflow or page errors. Inspected representative screenshots.
- Passed: Read-only HEAD requests to HTTPS and internal HTTP T41P common CFG
  endpoints returned 401 with the TalkOps Basic-auth challenge. HTTPS completed
  with normal computer certificate validation. Local DNS resolved the domain
  to the internal server address. No authenticated configuration was fetched.
- Not run: Rust/web application suites and hardware tests. No runtime code
  changed; user review and phone-level diagnosis remain pending.
- STE review: Used repository clarity rules. The full official standard was
  unavailable. Full language review is pending.

## Dependencies and open work

- Required PR dependencies: None. Migration IDs: None. Package changes: None.
- Future schema changes need fresh, unique migration IDs after approval.
  The proposal does not reserve an ID or change the database.
- Confirm whether both T41P attempts used the same provisioning credentials.
- Run manual HTTPS on T41P and compare DHCP replies. TLS/firmware is a hypothesis,
  not a confirmed root cause. The AX86R firmware has not been recorded.
- Review template-editor scope, key groups and delivery order with the user.
- Proposed release notes describe future implementation, not this docs-only PR.

## Next action

Present draft PR 49 and the preview. Request feedback on the concrete proposal.
Implement only after the user reviews the concrete proposal. Preserve the task
branch and refresh the verified target before starting implementation.

## References

- docs/ai-memory/20261016-codex-yealink-t41p-dss.md
- docs/ai-memory/20261012-codex-yealink-internal-phonebook.md
- docs/provisioning-multivendor-proposal.md
- docs/release-workflow.md
- https://github.com/itsh-neumeier/talkops/pull/47
