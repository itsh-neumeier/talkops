# All-vendor provisioning and phone copy proposal

## Task state

- Task ID: 20261017-codex-provisioning-all-vendors-proposal
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: In progress. Proposal prepared; user review required before implementation.
- Branch: codex/provisioning-all-vendors-proposal
- Base commit: c4765b0495adf004f0b2b0f5099cbd098f384f86
- Related PR: https://github.com/itsh-neumeier/talkops/pull/51 (draft)
- Proposal commit: b2500f7

## Goal

Extend provisioning to all usable FusionPBX vendor templates. Add identical-phone
configuration copy with separate device identities. Present the design for user
review and feedback before runtime changes.

## Result

Prepared and published a proposal and pinned upstream inventory in draft PR 51.
No runtime code, migration, active template or production setting changed.

## Changes

- `docs/provisioning-all-vendors-proposal.md`: Current gaps, complete vendor scope,
  adapter and routing design, copy rules, sequence, acceptance and test plan.
- `docs/research/fusionpbx-all-vendors-inventory.json`: Metadata from the complete
  upstream Git tree at 20143aec4366afc2755f336d3058243479d49d44.
- This file: Review boundary and continuation context.

## Decisions

- The user's explicit review requirement takes precedence over the generic
  implementation instruction. PR 49 approval does not approve this extension.
- Remote default HEAD and fetched origin/main both identify the base commit.
  PR 49 and integration PR 50 are merged. No open PR existed at task start.
- Use a new worktree at
  `C:/Users/timo/.codex/worktrees/provisioning-all-vendors-proposal/talkops`.
  Preserve the older provisioning worktree and the primary checkout's three
  untracked Dahua summaries. Do not import foreign uncommitted changes.
- Recommend same-model snapshot copy, a server allowlist, separate SIP devices,
  explicit extension selection and no automatic resync.
- All 31 source directories remain in scope. Spectralink has only empty.txt.
  Source import alone does not satisfy full vendor provisioning support.

## Checks

- Passed: Remote HEAD verification, fetched target and existing work inspection.
- Passed: Current adapter, catalog, routes, storage, UI, ADR and prior PR review.
- Passed: GitHub tree retrieval; response is not truncated (31 directories,
  1,037 files). Source directory presence is not a capability claim.
- Passed: Independent pinned subtree cross-check for all 31 vendor rows,
  1,037 files, directory lists and filename patterns. Spectralink empty.txt
  is the only file in that directory and has zero bytes.
- Passed: All local Markdown links in the proposal and summary resolve.
- Passed: `git diff --cached --check` after staging the three task files.
- Not run: Runtime tests. This proposal changes documentation and metadata only.
- STE review: Applied repository clarity rules. Full official standard was
  unavailable; full language review remains pending.

## Dependencies and open work

- Required PR dependencies: None. PR 49 is already in the base.
- Migration IDs in this PR: None. Existing schema baseline: 20261105000001.
- New runtime dependencies: None. No release metadata changes.
- App worktree registration was unavailable for this projectless parent path.
  A separate ordinary Git worktree and feature branch were created successfully.
- Implementation, functional tests and real-device verification remain open.

## Next action

Ask the user to review draft PR 51, its scope and copy rules.
Wait for user approval before runtime changes. Then implement the approved
design, update the summary and PR, and run the planned checks.

## References

- docs/provisioning-all-vendors-proposal.md
- docs/ai-memory/20261016-codex-provisioning-multivendor-plan.md
- docs/ai-memory/20261017-codex-release-1.8.2.md
- docs/adr/0038-vendor-provisioning-profiles.md
- docs/release-workflow.md
