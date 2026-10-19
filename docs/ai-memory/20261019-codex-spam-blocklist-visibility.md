# Blocked call visibility and whitelist proposal

## Task state

- Task ID: 20261019-codex-spam-blocklist-visibility
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: In progress. User approved implementation; verification in progress.
- Branch: codex/spam-blocklist-visibility
- Base commit: 488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d
- Related commits or PR: proposal commit `660c32e`;
  [draft PR 56](https://github.com/itsh-neumeier/talkops/pull/56).

## Goal

Show block list decisions in the call list and recent calls. Let authorized
users whitelist numbers. The user reviewed the proposal and approved implementation on 2026-10-19
with "ok mach das so".

## Result

Implemented stored CDR block reasons, an exact local whitelist, the separate
role permission, call view actions and settings, result filters and statistics.
Added core, API, parser, browser, backup and SIPp tests. Added English and German
user documentation. Verification is in progress; the PR remains a draft.
The exclusive worktree is
`C:/Users/timo/.codex/worktrees/spam-blocklist-visibility/talkops`.
The app worktree tool could not find the nested repository. Created the
worktree with Git. Added an exact Git safe-directory exception for this
worktree because its Git metadata is on a network share.

## Changes

- `docs/spam-blocklist-visibility-proposal.md`: behavior, permission, data
  model, test plan, upgrade limits and proposed release notes.
- Core/API: CDR reason, local exceptions, atomic audit, role permission,
  filters, scoped membership and statistics.
- Web: shared status and confirmation components, both call views, whitelist
  settings, role labels, result filters and softphone classification.
- Migration, backend tests, browser tests, SIPp scenario and CI browser step.
- English/German call log, routing and role documentation.

## Decisions

- The user approved the complete proposal. Commit, push and PR updates are
  authorized. Keep the PR in draft until implementation and checks are done.
- Store the existing `talkops_blocked` reason in CDRs. The old parser lost
  the reason. Do not reconstruct old reasons from lists.
- Use exact local exceptions before local block lists and PhoneBlock.
  Keep the historical decision when a number is later whitelisted.
- Use `blocking.allowlist`, admin-only by default. An administrator can
  grant it through roles. Preserve tenant and call visibility restrictions.
- Use consistent result filters and statistics. Known blocked calls
  must not count as missed or answered.
- Preserve the original checkout's local commit and three Dahua summaries.
  PR 55 is the only open contribution found at proposal start; it is unrelated.

## Checks

- Passed: remote target and open PR verification; source and diff inspection.
- Passed: core PostgreSQL tests in `call_allowlist` (3 tests), including audit
  rollback, concurrent duplicate creation, scope, filtering and statistics.
- Passed: web lint, type check (zero errors/warnings), unit tests (38), build.
- Passed: `tests/browser/call-blocking.mjs`, `call-list.mjs`,
  `call-presentation.mjs`, `roles.mjs` and `tests/e2e/softphone-ui.mjs`.
  New browser checks cover two languages, both themes, narrow layouts,
  historical decisions, errors, whitelist management and restricted roles.
- Pending: API/FreeSWITCH/backup/role regressions, clippy and workspace tests.
- Local setup: PostgreSQL 17.11 uses an exclusive data directory at
  `C:/Users/timo/.codex/tmp/spam-blocking-pg` on port 55438. Verified with
  `SHOW data_directory`. Rust uses the installed MSVC toolchain because
  the GNU linker setup failed. Moved only this task's build cache to
  `A:/aislop/codex/talkops/spam-blocking-target` after a disk-full failure.
- Corrected during checks: Svelte pattern quoting and the expected filter
  order in a unit test. Converted local web checkout text to LF for lint;
  no unrelated content changes are intended. Restored a document whose
  write failed when the disk filled, then applied its intended addition.
- Not run locally: Docker/FreeSWITCH/SIPp stack. Docker daemon is unavailable.
  The new SIPp case is wired into the existing E2E workflow.
- STE review: used repository writing rules. Full official standard
  unavailable; full language review pending.

## Dependencies and open work

- Migration ID: `20261106000003` (nullable CDR reason, tenant whitelist).
  Existing rows retain a null reason. No automatic non-admin permission grant.
- No new runtime package, external service or dependency on another PR.
- Complete verification and review, then commit, push and update PR 56.
  The release manager owns integration, versions, tags and publication.

## Next action

Finish Rust/API checks, inspect final diff and migration IDs, record results,
commit and push, and update the PR description and readiness.

## References

- [Proposal](../spam-blocklist-visibility-proposal.md)
- [Workflow](../release-workflow.md)
- [Call presentation](20261016-codex-call-flags-skeleton.md)
- [Call log filters](20261015-claude-call-log-filters.md)
