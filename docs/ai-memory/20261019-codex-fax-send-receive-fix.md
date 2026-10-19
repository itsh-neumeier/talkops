# Fax send and receive repair

## Task state

- Task ID: 20261019-codex-fax-send-receive-fix
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: In progress. Proposal awaits user review before implementation.
- Branch: `codex/fax-send-receive-fix`
- Base commit: `488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d`
- Related PR: Pending

## Goal

Repair web fax send failures and incoming calls that do not reach the fax inbox.
The user explicitly requires a proposal and feedback before implementation.

## Result

Read project and release instructions. Verified remote `main` and merged PR 53.
Inspected upload, worker conversion, incoming routing, receiver, settings UI,
Compose mounts, and existing tests. No production logs or settings are available.
Created a proposal. No runtime code or live settings changed.

## Changes

- `docs/fax-send-receive-fix-proposal.md`: evidence, hypotheses, proposed repair,
  acceptance tests, documentation, and the user review boundary.
- This summary: continuation and release handoff context.

## Decisions

- Use `C:/Users/timo/.codex/worktrees/fax-send-receive-fix/talkops`.
  The app worktree tool failed because the chat root is outside the repository.
  Native `git worktree add` created the dedicated checkout and feature branch.
- Preserve the original checkout's local commit and three untracked summaries.
  Preserve the earlier fax worktree and all other checkouts.
- An old stack definition without the fax volume can explain an upload 500.
  This is a hypothesis, not a confirmed cause in the user's deployment.
- A global sender number does not assign an incoming fax destination.
  Missing or disabled routing and failure before answer need separate diagnosis.
- Ask for installed version, updated stack status, number assignment, and the
  relevant safe server log. Continue source inspection while the answer is pending.
- Submit documentation as a draft PR. Wait for feedback before implementation.

## Checks

- Passed: remote default branch and target SHA; clean new worktree; merged PR 53.
- Passed: `git diff --cached --check`; only the proposal and this summary are staged.
- Source inspection: current Compose mounts fax in all three services; upload
  filesystem and database errors can return the generic 500 response.
- Not run: runtime suites. This checkpoint changes documentation only.
- Not run: provider send/receive and production checks. No production access.
- STE review: repository clear-English rules applied. Full standard unavailable;
  full language review remains pending.

## Dependencies and open work

- Need user feedback on the proposal and runtime evidence to identify the cause.
- No new dependencies or migrations planned. Existing fax migrations are
  `20261106000001` and `20261106000002`; do not edit released files.
- Recheck remote target before implementation. Release actions stay separate.

## Next action

Wait for the user's review. Then reproduce the failed stage, implement the
approved repair, add tests and documentation, and update the same draft PR.

## References

- [Proposal](../fax-send-receive-fix-proposal.md)
- [Earlier fax task](20261018-codex-fax-sip-proposal.md)
- [Fax decision](../adr/0040-fax-over-sip-and-mail-input.md)
- https://github.com/itsh-neumeier/talkops/pull/53
