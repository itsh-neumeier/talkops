# Fax send and receive repair

## Task state

- Task ID: 20261019-codex-fax-send-receive-fix
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: In progress. User approved implementation on 2026-10-19.
- Branch: `codex/fax-send-receive-fix`
- Base commit: `488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d`
- Related PR: https://github.com/itsh-neumeier/talkops/pull/55 (draft)

## Goal

Repair web fax send failures and incoming calls that do not reach the fax inbox.
The user required a proposal before implementation and has now approved it.
The user assigned spam exceptions and blocking changes to a separate PR.

## Result

Read project and release instructions. Verified remote `main` and merged PR 53.
Inspected upload, worker conversion, incoming routing, receiver, settings UI,
Compose mounts, and existing tests. The user later supplied settings and logs.
Committed the reviewed proposal in draft PR 55. Implemented safe upload storage
errors, an administrator storage check, translated UI messages, regression tests,
image smoke checks, and troubleshooting documentation. Verification is in progress.
No direct production access is available. No live settings changed.

## Changes

- `docs/fax-send-receive-fix-proposal.md`: evidence and approved repair scope.
- API fax storage helper, upload handler and typed errors: real write/space
  checks, safe HTTP 503/507 responses, and incomplete upload cleanup.
- Fax settings and send UI: administrator check and translated storage errors.
- API/storage/browser tests, image smoke fixture and E2E: failure and recovery.
- `docs/en/fax.md`, `docs/de/fax.md`: mount repair and rejection diagnosis.
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
- Keep PhoneBlock and all spam changes outside this PR at the user's request.
  Document the confirmed rejection only. No code dependency on the separate PR.
- Do not change T.38 settings to address these failures. The supplied logs
  do not show that transmission negotiation or conversion has started.
- The user approved implementation. No further confirmation is required for
  code, tests, documentation, commit, push, or the existing PR update.
- The repository Compose mounts are already correct. Do not weaken the root
  filesystem protection. Diagnose the deployed mount separately; its exact
  definition is still pending. All three services need the same writable volume.

## Checks

- Passed: remote default branch and target SHA; clean new worktree; merged PR 53.
- Passed: `git diff --cached --check`; only the proposal and this summary are staged.
- Source inspection: current Compose mounts fax in all three services; upload
  filesystem and database errors can return the generic 500 response.
- Passed: matched supplied PhoneBlock and call rejection logs to the dialplan;
  checked spam threshold semantics and the available blocking settings.
- Passed: web check (zero errors/warnings), 35 web tests, production build, and
  lint after normalizing local checkout line endings. No unrelated web content changed.
- Passed: fax browser fixture with retained files, both storage errors, repair
  and retry, German/English messages, settings recheck, mobile layout and roles.
- Passed: all ten Python fax conversion/MIME/IMAPS tests; shell syntax checks.
- Passed: MSVC `cargo check -p talkops-api --tests --locked`, Rust formatting,
  production/development Compose validation, and final whitespace check.
- In progress: focused database/storage tests and final CI/E2E validation.
- Review: CodeRabbit CLI is not installed. Manual diff review checks permissions,
  safe responses, probe cleanup, partial-upload cleanup, and preserved spam scope.
- Not run: provider send/receive and production checks. No production access.
- STE review: repository clear-English rules applied. Full standard unavailable;
  full language review remains pending.

## Dependencies and open work

- Need effective stack mounts and fax path to identify the exact deployment
  correction. Requested only volume sections and optional TALKOPS_FAX_DIR.
- Retest reception with an allowed caller after the separate spam decision.
- No new dependencies or migrations planned. Existing fax migrations are
  `20261106000001` and `20261106000002`; do not edit released files.
- Recheck remote target before implementation. Release actions stay separate.

## Next action

Complete Rust and CI checks, review the final diff, commit and push the repair,
and update PR 55 with verification results and deployment limits.

## References

- [Proposal](../fax-send-receive-fix-proposal.md)
- [Earlier fax task](20261018-codex-fax-sip-proposal.md)
- [Fax decision](../adr/0040-fax-over-sip-and-mail-input.md)
- https://github.com/itsh-neumeier/talkops/pull/53
