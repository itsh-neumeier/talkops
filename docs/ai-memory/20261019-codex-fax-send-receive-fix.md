# Fax send and receive repair

## Task state

- Task ID: 20261019-codex-fax-send-receive-fix
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: In progress. Proposal awaits user review before implementation.
- Branch: `codex/fax-send-receive-fix`
- Base commit: `488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d`
- Related PR: https://github.com/itsh-neumeier/talkops/pull/55 (draft)

## Goal

Repair web fax send failures and incoming calls that do not reach the fax inbox.
The user explicitly requires a proposal and feedback before implementation.

## Result

Read project and release instructions. Verified remote `main` and merged PR 53.
Inspected upload, worker conversion, incoming routing, receiver, settings UI,
Compose mounts, and existing tests. The user later supplied settings and logs.
Created a proposal. No runtime code or live settings changed.
Committed and pushed the proposal. Draft PR 55 is available for user feedback.
Updated the proposal with a filesystem error and a PhoneBlock rejection from
the supplied logs. No direct production access is available.

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
  The logs now confirm `Read-only file system (os error 30)` with three HTTP
  500 events. They do not identify the failing path or effective mounts.
- A global sender number does not assign an incoming fax destination.
  The user confirms configuration. Do not assume the incoming route is missing.
- The incoming test is blocked with `phoneblock:4`, followed by zero duration
  and `CALL_REJECTED`. The dialplan returns SIP 603 before fax reception.
  This is the confirmed rejection reason for that supplied test call.
- Startup confirms version 1.8.4 and current schema. FreeSWITCH connects before
  the incoming test. Do not treat the earlier startup retries as its cause.
- Current PhoneBlock has no per-caller allowlist. Propose a controlled operator
  retest with its switch disabled, then restore it. Keep live spam settings
  unchanged. A permanent exception needs a separate product decision.
- Do not change T.38 settings to address these failures. The supplied logs
  do not show that transmission negotiation or conversion has started.
- Submit documentation as a draft PR. Wait for feedback before implementation.

## Checks

- Passed: remote default branch and target SHA; clean new worktree; merged PR 53.
- Passed: `git diff --cached --check`; only the proposal and this summary are staged.
- Source inspection: current Compose mounts fax in all three services; upload
  filesystem and database errors can return the generic 500 response.
- Passed: matched supplied PhoneBlock and call rejection logs to the dialplan;
  checked spam threshold semantics and the available blocking settings.
- Not run: runtime suites. This checkpoint changes documentation only.
- Not run: provider send/receive and production checks. No production access.
- STE review: repository clear-English rules applied. Full standard unavailable;
  full language review remains pending.

## Dependencies and open work

- Need user feedback before implementation. Need effective stack mounts and
  fax path to identify the exact filesystem correction. Retest reception after
  an operator-approved PhoneBlock configuration change.
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
