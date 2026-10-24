# UniFi Talk style side panel for phones, extensions and users

## Task state

- Task ID: 20261024-claude-side-panel
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: `claude/side-panel` (worktree `../talkops-panel`)
- Base commit: `a97e918` (head of `claude/phone-network-stats`, PR #74)
- Related commits or PR: Pending

## Goal

PR 5 of 6 from one user request: improve the display like UniFi Talk
(reference: UniFi Talk device list with an inspector and a "Reassign Device"
dialog). The user accepted: side panel for phones, extensions and users;
ignore the second reference image (Linkvil phone web UI).

## Result

- `SidePanel.svelte`: right panel (26 rem on `lg`, full screen below),
  icon tabs (ARIA tabs with arrow keys), Escape, focus return.
- `Collapsible.svelte`, `PanelAction.svelte` (text action + info hint),
  `InfoList.svelte`, `Combobox.svelte` (select with search, ARIA combobox),
  `panelIcons.ts`, `#lib/panel.ts` (`?panel=<id>`, row click helper).
- Phones: `PhoneInspector.svelte` (Overview, Network if switched on,
  Settings with General/Manage: Reassign, Resync, Reboot, Delete). The
  reassign dialog has a combobox, "replace current assignment", a warning
  when all accounts are in use and "resync after the change".
- Extensions: `ExtensionInspector.svelte` (Overview, Devices, Settings).
- Users: `UserInspector.svelte` (Overview, Extensions, Settings; actions
  reuse the dialogs of the users page).
- List pages: row click and the name link open the panel; the page gets
  `lg:pr-[26rem]` while it is open; the selected row is marked.

## Decisions

- Base on PR #74, so the phone panel has the Network tab
  (`PhoneNetworkCard`).
- The panel shows and manages; detailed editors stay on the full pages
  ("Open all settings"). Renaming a phone needs the full `PhoneInput`
  (keys, provisioning), so the panel does not edit it.
- Reassign: when all accounts are in use, the old devices of the phone are
  removed before the new assignment (`DELETE /devices/{id}`, then
  `POST /extensions/{id}/devices`), then an optional resync.
- Ctrl/Cmd-click on the name link still opens the full page in a new tab.
- `Combobox` folds accents itself (no dependency on PR #76).

## Checks

- Passed: `npm run lint`, `npm run check`, `npm test` (71), `npm run build`.
- Passed: browser `side-panel.mjs` (new; 1280 px and 390 px: phone panel,
  reassign with search and API order, Escape, deep link, extension and user
  panels), `phone-network.mjs`, `layout.mjs`, `roles.mjs`,
  `provisioning.mjs`, `manufacturer-provisioning.mjs`, `phone-keys.mjs`.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Not run: Rust checks (no Rust change in this PR), Docker stack, SIPp/E2E.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- Depends on PR #74 (and #73 below it). No migration.
- Possible follow-up: the same panel for door stations or trunks.

## Next action

None.

## References

- `docs/ai-memory/20261024-claude-phone-network-stats.md`
- `docs/ui-guidelines.md` (section "Side panel")
