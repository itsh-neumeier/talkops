# PDF fax over SIP proposal

## Task state

- Task ID: 20261018-codex-fax-sip-proposal
- Date: 2026-10-18, Europe/Berlin
- Tool: Codex
- Status: In progress. Proposal prepared; user feedback required before code.
- Branch: `codex/fax-sip-proposal`
- Base commit: `7beb63d5b8e3917b7621588413afcec2a8893456`
- Related commits or PR: proposal commit `f58ed84`,
  [draft PR #53](https://github.com/itsh-neumeier/talkops/pull/53).

## Goal

Propose PDF fax send and receive over SIP, saved header/footer, role checks,
own/global numbers and mail-to-fax. The user requires review before changes.

## Result

Prepared and pushed a proposal based on the verified remote `main` and
current code. Draft PR #53 presents it for user review.
No feature code or migration is implemented. No fax was sent.
Commit, push and a draft PR are authorized. Feature implementation needs
the user's response to the proposal. Release actions belong to the release chat.

## Changes

- `docs/fax-sip-proposal.md`: scope, UI, permissions, sender and receive
  routing, header/footer rendering, mail transport options, restart handling,
  test plan, dependencies and proposed future release notes.
- This task summary: durable review boundary and continuation instructions.

## Decisions

- Separate worktree at `C:/Users/timo/.codex/worktrees/fax-sip-proposal/talkops`.
  The app worktree tool could not resolve the parent folder as a repository;
  `git worktree add` created the checkout from `origin/main`.
- Preserve source checkout commit `397fc83` and the three untracked
  Dahua task summaries. Do not use the stale local `main` as a base.
- Recommend existing FreeSWITCH `mod_spandsp`, existing queue and mail renderer.
- Recommend SMTP submission to an existing secured mailbox, retrieved over
  IMAPS. Direct SMTP gateway is an alternative for user review.
- Recommend strict E.164 subject and explicit fax receive routes. Clarify
  the literal caret and shared voice/fax behavior before implementation.
- This is a documentation proposal, not an accepted ADR or a ready feature PR.

## Checks

- Passed: remote default branch and target commit verified with `ls-remote`;
  `git fetch origin --prune`; source status, local-only commit and worktrees
  inspected. `gh pr list --state open` returned no open PRs at task start.
- Passed: current code and applicable task summaries inspected for fax,
  routing, roles, outgoing SMTP, mail templates, notifications and backups.
- Passed: local Markdown link and task date checks for both new files.
  The remote target and unchanged source checkout were verified again.
- Passed: `git diff --cached --check`; staged scope contains only the
  proposal and this task summary (two Markdown files).
- Passed: feature branch pushed and draft PR #53 created against `main`.
- Not run: Rust, web, database, image, SIP and mail tests. No runtime change.
- STE review: clear English review using project guidance. Full official
  standard unavailable; full language review pending.

## Dependencies and open work

- No migration ID or new runtime dependency in this contribution.
- Future migrations must follow the latest verified sequence. Current maximum
  is `20261105000001`. Allocate unique IDs at implementation time.
- User feedback needed: scope, mail transport, caret in subject, dedicated
  incoming number versus voice/fax detection.
- Future tests need Linux, PostgreSQL, FreeSWITCH, a test mail server,
  a fax peer, and provider access for a real trunk interoperability check.

## Next action

Wait for user feedback on draft PR #53 or in this chat.
After feedback, refresh the remote target, record the accepted behavior,
then implement and test. Keep the PR a draft until the feature is complete.

## References

- [Proposal](../fax-sip-proposal.md)
- `20261015-claude-roles-permissions.md`
- `20261016-claude-mail-design-editor.md`
- `20261016-claude-notifications.md`
- `20261016-claude-storage-targets.md`
- `docs/release-workflow.md`
