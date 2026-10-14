# Remove the test calls card

## Task state

- Task ID: 20261014-claude-remove-test-calls
- Date: 2026-10-14
- Tool: Claude Code
- Status: Completed
- Branch: claude/elegant-lamport-8tuen0
- Base commit: 7b6e51f (chore(release): finalize TalkOps 1.7.11)
- Related commits or PR: See the PR for this branch

## Goal

Remove the "Test calls" card (sip5060.net targets) from *Settings → System*.
The user reports that the test calls do not work and the function is not
necessary.

## Result

The diagnostics page has no test calls card. The UI strings and the user
documentation for the card are removed. The work is committed and pushed.

## Changes

- `web/src/lib/components/SystemDiagnostics.svelte`: remove the `testCalls`
  list and the card.
- `web/src/lib/i18n/en.ts`, `web/src/lib/i18n/de.ts`: remove the
  `diag.testCalls*` and `diag.test.*` keys.
- `web/src/lib/components/Softphone.svelte`: make the `?dial=` comment generic.
- `docs/en/monitoring.md`, `docs/de/monitoring.md`: remove the test calls item.
- `CHANGELOG.md`: add an "Unreleased → Removed" entry.

## Decisions

- Keep the softphone parameter `/phone?dial=…`. The user approved this.
- Keep "Allow dialing SIP addresses" and its dialplan. It is a separate
  function. Tests that use `test.echo@sip5060.net` as a sample SIP address
  stay as they are.
- Do not change the 1.5.x changelog entry. It records a released version.

## Checks

- Passed: `npm ci`, `npm run lint`, `npm run check` (0 errors, 0 warnings),
  `npm run build`, `npm test` (6 pass) in `web/`.
- Passed: search for `testCall`, `diag.test.` and `sip5060` in `web/src`,
  `docs/en` and `docs/de` gives no result.
- Not run: Rust checks and the SIPp end-to-end test. No Rust code changed.
- STE review: manual review of the new text.

## Dependencies and open work

- None. No migration, no API change.

## Next action

Release chat: review and merge the PR.

## References

- `docs/release-workflow.md`
