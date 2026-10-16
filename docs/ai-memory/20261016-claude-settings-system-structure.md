# Settings: System sub-tabs and Backup section

## Task state

- Task ID: 20261016-claude-settings-system-structure
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed
- Branch: claude/settings-system-structure
- Base commit: 0f128ad (main, release 1.7.14)
- Related commits or PR: https://github.com/itsh-neumeier/talkops/pull/37. Stacked follow-ups: claude/storage-targets
  (PR 7a) and claude/audit-export (PR 7b).

## Goal

Settings → System showed the diagnostics overview, SIP capture, channels,
sessions, restart, backups and backup targets on one long page. Make the
structure clear. Change the UI only. Do not change logic or permissions.

## Result

- Section **System** (permission `diagnostics.manage`) has four sub-tabs
  with `Tabs.svelte`: Status, Diagnostics, Sessions and Maintenance.
- Deep links: `/settings?section=<id>` and
  `/settings?section=system&tab=<status|diagnostics|sessions|maintenance>`.
  Old `/settings#<id>` links still work. An old `#system` link opens the
  Backup section when the user has `backups.manage` only.
- The Status tab shows the overview, FreeSWITCH, trunk account states (with
  `trunks.view`), registered devices (with `calls.active`) and the active
  channels. It uses the existing API endpoints only.
- New section **Backup** (permission `backups.manage`) with the backup
  settings, Portainer, restore import and the backup targets UI.
- The log poll now runs only while the Diagnostics tab is open.

## Changes

- `web/src/lib/components/SettingsNav.svelte`: section `backup`; `system`
  needs `diagnostics.manage` only; links use `?section=`.
- `web/src/routes/settings/+page.svelte`: query parameters, tab state, URL
  sync with `replaceState` from `$app/navigation`.
- `web/src/lib/components/SystemDiagnostics.svelte`: now the tab container.
- New components: `SystemStatus.svelte`, `DiagnosticsCapture.svelte`,
  `SystemSessions.svelte`, `SystemMaintenance.svelte`.
- `web/src/lib/diagnostics.ts`: shared types and the uptime formatters.
- `web/src/lib/i18n/en.ts`, `de.ts`: new keys `settings.section.backup`,
  `system.tab.*`, `diag.trunk*`, `diag.reg*`.
- `web/tests/diagnostics.test.ts`, `tests/browser/settings-system.mjs`
  (added to CI), `tests/browser/layout.mjs` (system tabs).
- Docs: `docs/en/monitoring.md`, `docs/de/monitoring.md`, backup, Portainer,
  codec profile guides (en/de), `docs/ui-guidelines.md`.

## Decisions

- Tab IDs are English (`status`, `diagnostics`, `sessions`,
  `maintenance`). German labels: Status, Diagnose, Sitzungen, Wartung.
- The System section no longer shows for `backups.manage` alone. The
  content of each section keeps its old permission, so access is the same.
- No API change. The trunk table reads `GET /trunks`, the device table
  `GET /telephony/status`, both only with their existing permissions.

## Checks

- Passed: `npm run lint`, `npm run check` (0 errors), `npm test` (16 tests),
  `npm run build`.
- Passed: browser checks against `npm run preview` with Playwright 1.56:
  `settings-system.mjs`, `layout.mjs`, `roles.mjs`,
  `transcription-settings.mjs`, `motion.mjs`, `active-calls.mjs`.
- Not run: Rust checks (no Rust change in this PR).
- STE review: Clear English review. Full standard unavailable; review pending.

## Dependencies and open work

- Migration IDs: none.
- PR 7a (storage targets) moves the target management out of the Backup
  section into a new section "Storage".

## Next action

The release manager reviews and merges this PR before PR 7a and PR 7b.

## References

- [Backup targets](20261015-claude-backup-targets.md)
- [ADR 0029 roles](../adr/0029-roles-and-permissions.md)
- [UI guidelines](../ui-guidelines.md)
