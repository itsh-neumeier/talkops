# Fax over SIP and mail input

## Task state

- Task ID: 20261018-codex-fax-sip-proposal
- Date: 2026-10-18, Europe/Berlin
- Tool: Codex
- Status: In progress. Implementation complete; final checks in progress.
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
- Passed: first four PostgreSQL fax integration tests; seven real Python MIME and
  conversion tests, including PDF conversion with Poppler.
- Passed: web check (zero errors/warnings), 35 web tests, build and final lint.
- Passed: first fax browser check (attachments, preview, send, whitelist, mobile,
  permissions). A final rerun is in progress.
- In progress: full workspace Rust tests, final five fax API tests and extended
  fax backup restore test. Private PostgreSQL 17.11 uses port 55436.
- Local limitations: GNU toolchain lacks gcc/dlltool. MSVC is available.
  Full all-targets clippy reports existing Windows-only dead code in worker
  realtime tests. Linux CI must run the exact repository command.
- First full parallel test build failed because Windows exhausted paging resources.
  The retry uses one build job and two test threads.
- Not run locally: Docker images and live SIP/T.38. No Linux Docker daemon is
  available. The new E2E workflow runs those tests on the test stack.
- CodeRabbit CLI is absent. Manual review is in progress.
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

Finish local and Linux CI checks, fix failures, update this summary and the PR
with actual results, then mark the PR ready only when the required checks pass.

## References

- [Accepted proposal](../fax-sip-proposal.md)
- [ADR 0040](../adr/0040-fax-over-sip-and-mail-input.md)
- [English user guide](../en/fax.md)
- Related existing summaries: roles, mail design, notifications and storage targets.
