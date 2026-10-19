# Fax storage repair and incoming call diagnosis

## Task state

- Task ID: 20261019-codex-fax-send-receive-fix
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: Implementation complete. PR 55 records final readiness and check results.
- Branch: `codex/fax-send-receive-fix`
- Base commit: `488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d`
- Validated runtime commit: `cb18ceb25e4c5c7191520f07b67cb175a5312141`
- PR: https://github.com/itsh-neumeier/talkops/pull/55

## Goal and result

Repair fax uploads that return HTTP 500. Diagnose incoming fax calls that appear
only as rejected calls. The user reviewed the proposal and approved implementation.
The user assigned spam exceptions and blocking to a separate PR.

The implementation checks actual storage access and free space before upload.
It returns safe HTTP 503/507 errors, removes incomplete uploads, and shows translated
messages while keeping attachments for retry. Fax settings include a permission-
protected server storage check. Tests cover failure, cleanup, and recovery.

The supplied logs confirm a read-only filesystem error during failed requests.
They also identify PhoneBlock as the reason for the rejected incoming test.
No production settings changed. The user will wait for the release and apply the
stack update then. The exact deployed mount definition was not supplied.

## Changes and decisions

- Use the dedicated worktree at
  `C:/Users/timo/.codex/worktrees/fax-send-receive-fix/talkops`.
  Preserve all foreign checkouts, the original local commit and untracked summaries.
- Add `crates/talkops-api/src/fax/storage.rs`. Use unique temporary probes and the
  existing 100 MiB free-space requirement. Keep internal paths in server logs.
- Update the upload handler and typed API errors. Add
  `GET /api/v1/settings/fax/storage` for the `fax.manage` permission.
- Update Fax settings, the send page, shared fax types and DE/EN translations.
- Extend API/database and browser tests. Add image smoke checks for read-only
  roots with and without a writable fax mount. Check storage in the media E2E.
- Update `docs/en/fax.md` and `docs/de/fax.md` with stack repair instructions.
  The repository Compose mounts are already correct. Preserve the read-only
  root filesystem and existing fax data when updating a deployed stack.
- Keep spam implementation, routing, T.38 settings, migrations, dependencies,
  versions, tags, and publication outside this change.
- The closing documentation revision does not change runtime code after
  `cb18ceb`. Consult the PR checks for its final head.

## Checks

- Passed locally: MSVC `cargo check -p talkops-api --tests --locked`; Rust format;
  web lint, check with zero errors/warnings, 35 tests, and production build.
- Passed locally: extended fax browser fixture, including both storage errors,
  retained files, repair/retry, DE/EN messages, admin recheck, mobile and roles.
- Passed locally: all 10 Python MIME/IMAPS/document conversion fixtures, shell
  syntax, production/development Compose validation, whitespace and scope checks.
- Passed on `cb18ceb`: full Linux CI, including strict workspace clippy and
  405 Rust tests. One existing model-dependent test was ignored. Both new
  storage unit tests and both database regressions passed, including Unix
  read-only directory rejection. Web/browser, Compose, server and worker image
  builds and both new storage image smoke checks passed.
  CI: https://github.com/itsh-neumeier/talkops/actions/runs/39231757301
- Passed on `cb18ceb`: the two-page PDF/image G.711 and negotiated T.38 fax
  media fixture step. The PR records the final complete E2E workflow result.
  E2E: https://github.com/itsh-neumeier/talkops/actions/runs/39231757214
- Local limitation: Rust test and later clippy builds failed before test
  execution when the Windows drive ran out of space (OS error 112). Do not
  report a local Rust test pass. `cargo clean` removed only this task's target
  artifacts (4.1 GiB). Private PostgreSQL and preview services were stopped.
- Not run locally: container execution and SIP media. No local Linux Docker
  daemon is available. No live deployment or real-provider acceptance test ran.
- Manual review checked permissions, safe responses, probe cleanup, upload
  cleanup and scope. CodeRabbit CLI is absent; no CodeRabbit review ran.
- STE: repository clear-English rules applied. Full standard unavailable;
  full language review remains pending.

## Dependencies and upgrade

- No code dependency on another PR. Earlier fax PR 53 is merged.
- The separate spam PR owns exceptions and blocking. Real reception from the
  previously blocked test sender needs an allowed-call policy and a retest.
- No new migration. Existing IDs `20261106000001` and `20261106000002` are unchanged.
- Deploy the updated stack definition with the images. API, worker and
  FreeSWITCH need the same writable fax volume and path, with UID/GID 10001
  access. An image update alone does not repair old volume definitions.

## Next action

The release chat must inspect PR 55 and its final checks, select the release,
and perform integration and publication. The operator must apply the documented
stack correction and test real outbound and inbound delivery after deployment.
Do not send another chat a message without explicit user authorization.

## References

- [Approved scope and evidence](../fax-send-receive-fix-proposal.md)
- [Earlier fax task](20261018-codex-fax-sip-proposal.md)
- [Fax decision](../adr/0040-fax-over-sip-and-mail-input.md)
