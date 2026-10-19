# Blocked call visibility and whitelist proposal

## Task state

- Task ID: 20261019-codex-spam-blocklist-visibility
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: In progress. Proposal prepared; user feedback required.
- Branch: codex/spam-blocklist-visibility
- Base commit: 488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d
- Related commits or PR: proposal commit `660c32e`;
  [draft PR 56](https://github.com/itsh-neumeier/talkops/pull/56).

## Goal

Show block list decisions in the call list and recent calls. Let authorized
users whitelist numbers. The user requires a proposal and feedback before
application changes.

## Result

Inspected current routing, CDR ingestion, call views, permissions and
statistics. Committed and pushed a proposal and test plan. Draft PR 56 is
open for user feedback. No application code or migration changed.
The exclusive worktree is
`C:/Users/timo/.codex/worktrees/spam-blocklist-visibility/talkops`.
The app worktree tool could not find the nested repository. Created the
worktree with Git. Added an exact Git safe-directory exception for this
worktree because its Git metadata is on a network share.

## Changes

- `docs/spam-blocklist-visibility-proposal.md`: behavior, permission, data
  model, test plan, upgrade limits and proposed release notes.
- This task summary: current state and implementation boundary.

## Decisions

- Wait for user feedback before runtime changes. Commit, push and draft PR
  creation are authorized. Keep the PR in draft until implementation is done.
- Propose storing the existing `talkops_blocked` reason in CDRs. It is
  currently lost during parsing. Do not reconstruct old reasons from lists.
- Propose exact local exceptions before local block lists and PhoneBlock.
  Keep the historical decision when a number is later whitelisted.
- Propose `blocking.allowlist`, admin-only by default. An administrator can
  grant it through roles. Preserve tenant and call visibility restrictions.
- Propose consistent result filters and statistics. Known blocked calls
  must not count as missed or answered.
- Preserve the original checkout's local commit and three Dahua summaries.
  PR 55 is the only open contribution found at proposal start; it is unrelated.

## Checks

- Passed: remote default branch verification, fetch, branch and worktree
  inventory, source inspection and open PR inventory.
- Passed: `git diff --cached --check`; all local Markdown links in both
  new documents resolve. The staged change contains only these two files.
- Not run: Rust, web, PostgreSQL, browser, Docker and SIPp suites. This
  revision changes documentation only. The proposal lists required tests
  for implementation.
- STE review: used repository writing rules. The full official standard
  was unavailable; full language review is pending.

## Dependencies and open work

- No migration IDs in this revision. One forward migration is expected
  after feedback. Highest ID at the base: `20261106000002`.
- No new runtime package or external service is expected.
- No dependency on another PR. Recheck remote target and concurrent work
  before implementation. Do not publish release metadata or merge.

## Next action

Collect user feedback on the proposal. Then implement the agreed behavior,
add tests and user documentation, run checks and update the PR and summary.

## References

- [Proposal](../spam-blocklist-visibility-proposal.md)
- [Workflow](../release-workflow.md)
- [Call presentation](20261016-codex-call-flags-skeleton.md)
- [Call log filters](20261015-claude-call-log-filters.md)
