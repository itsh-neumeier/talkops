# Fax over SIP and mail input

## Task state

- Task ID: 20261018-codex-fax-sip-proposal
- Date: 2026-10-18, Europe/Berlin
- Tool: Codex
- Status: Implementation complete. Final readiness and live CI results are in PR #53.
- Branch: `codex/fax-sip-proposal`
- Base: `7beb63d5b8e3917b7621588413afcec2a8893456` (`origin/main`).
- PR: https://github.com/itsh-neumeier/talkops/pull/53 (draft).

## Goal and result

The user approved the reviewed proposal with "mach das so". Implement web fax,
mail attachment fax, incoming dedicated fax routes, saved text and role checks.
The implementation is in this branch. Release actions belong to the release chat.

## Changes and decisions

- Use a separate worktree at `C:/Users/timo/.codex/worktrees/fax-sip-proposal/talkops`.
  Preserve the original checkout, local commit `397fc83` and its untracked files.
- Add durable fax state, queue jobs, per-trunk dispatch, CDR reconciliation,
  result reports, retention, backup media and role-scoped API access.
- Use FreeSWITCH mod_spandsp with G.711 and configurable T.38 modes.
  Use explicit incoming fax destinations. Do not detect fax on voice calls.
- Render the PDF preview and transmitted Group 4 TIFF from the same pages.
  Support mixed PDF, JPEG, PNG, TIFF, BMP and WebP attachments in order.
- Retrieve a dedicated SMTP mailbox over verified IMAPS. The mail server must
  bind authenticated senders. An administrator confirms this restriction.
- Require an exact sender whitelist and a subject containing only an international
  number, such as `+49123456`. Queue rejection notices for invalid submissions.
- Fax only attachments. Ignore body, HTML, inline signatures and quoted content.
  The mail fax header contains only the sender email; the saved footer still applies.
- Do not automatically retry an uncertain transmission. Busy or unanswered calls
  with zero pages can retry up to three attempts. Web sends require preview submission.
- Add English/German UI and user docs, ADR 0040, API/DB tests, Python conversion
  fixtures, browser checks and a real SIP media round-trip CI test.

## Checks

- Passed: verified remote main; original checkout preserved; staged diff check.
- Passed: MSVC cargo check for API and worker before final small changes.
- Passed: seven PostgreSQL fax integration tests, including a local SMTP server,
  own/shared/global number policy, revoked rights, queue limits, cancellation,
  late CDR recovery and duplicate events. Two backup tests passed, including
  replacement and restoration of a fax PDF fixture.
- Passed: ten Python MIME, IMAPS protocol and conversion tests, including PDF
  conversion with Poppler. IMAPS tests check verified TLS, BODY.PEEK[], UIDVALIDITY,
  acknowledgement and rejection of oversized messages without download.
- Passed: web check (zero errors/warnings), 35 web tests, build and final lint.
- Passed: final fax browser check (attachments, preview, send, whitelist, mobile,
  permissions). Shell syntax check passed for the SIP media fixture.
- Local Rust tests use PostgreSQL 17.11 on private port 55436 and MSVC.
- Local limitations: GNU toolchain lacks gcc/dlltool. MSVC is available.
  Full all-targets clippy reports existing Windows-only dead code in worker
  realtime tests. Linux CI must run the exact repository command.
- First full parallel test build failed because Windows exhausted paging resources.
  The single-build-job retry was stopped to run the focused API/backup tests.
  Full Linux workspace checks run in CI; do not claim a full local Windows pass.
- Not run locally: Docker images and live SIP/T.38. No Linux Docker daemon is
  available. The new E2E workflow runs those tests on the test stack.
- The implementation checkpoint passed Linux clippy, web checks, Compose and
  both application image builds. Final CI/E2E on the last revision is the release
  gate. Consult the PR checks for current results; superseded runs can be cancelled.
- CodeRabbit CLI is absent. Manual review checked access, secret handling,
  dispatch/cancel races, partial results and missing report attachments.
  Fixed a FreeSWITCH default-header fallback: use URL-encoded empty text so the
  engine does not add a second header above the rendered sender email.
- Language review used the project clear-English rules. The full official STE
  standard was unavailable; full language review remains pending.

## Dependencies and upgrade

- No dependency on another PR. Recheck migration IDs at integration time.
- Migrations: `20261106000001` (destination enum), `20261106000002` (fax tables).
- Add Python to the server; Pillow, ReportLab, Poppler and DejaVu to the worker.
  Add the shared `fax` volume to all three services and to backup/recovery.
- Fax is disabled by default. Administrators configure numbers, permissions,
  dedicated destinations and optional IMAPS/SMTP before enabling it.
- Apply both migrations before the new binary. Do not downgrade after assigning
  fax destinations. No version bump, release tag or publication in this task.
- Provider interoperability and delivery through the deployment mail server need
  an operator acceptance test with that provider and server.

## Next action

Use PR #53 as the release handoff. It records final head, test results and readiness.
The release chat must wait for required CI/E2E checks on that head, recheck the
target and migration IDs, then select the release. No release action is delegated
to this development branch.

## References

- [Accepted proposal](../fax-sip-proposal.md)
- [ADR 0040](../adr/0040-fax-over-sip-and-mail-input.md)
- [English user guide](../en/fax.md)
- Related existing summaries: roles, mail design, notifications and storage targets.
