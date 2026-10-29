# Manufacturer names and badges in the phone UI

## Task state

- Task ID: 20261029-claude-vendor-names-badges
- Date: 2026-10-29
- Tool: Claude Code
- Status: Completed
- Branch: claude/vendor-names-badges
- Base commit: 145f3bc2699f2dbb84f2a9ea1b21dcafd6365597 (main, v1.8.15)
- Related commits or PR: Pending

## Goal

The user asked for correctly capitalized manufacturer names and for small
manufacturer marks of one size instead of the phone drawings in the phone UI.

## Result

- `web/src/lib/vendors.ts` maps the lowercase catalog vendor ids to the
  manufacturers' own spelling (for example `Cisco`, `Yealink`, `2N`, `ATCOM`,
  `VTech`). Unknown ids get a capital first letter. `modelLabel` joins vendor
  and model name and does not repeat a vendor that the model name already
  contains. `sortModels` sorts by manufacturer, then by model name.
- `VendorBadge.svelte` is a fixed-size text badge (`h-6 w-24`).
- The phone list, the add-phone dialog, the phone detail page and the side
  panel show the badge instead of `PhoneGraphic`. Model selects show
  "Manufacturer Model" and are sorted. The clone dialog, the vendor settings
  heading and the template page use the same labels.
- The key editor keeps `PhoneGraphic`, because it shows the key layout.

## Decisions

- Text badges instead of logos. Manufacturer logos are protected artwork.
  Only 6 of 30 catalog vendors have CC0 marks in Simple Icons (Cisco, Poly,
  Linksys, Linphone, Panasonic, Zoiper); Yealink has none. The user chose
  uniform badges for all manufacturers.

## Checks

- Passed (node:22 container): `svelte-check` 0 errors and 0 warnings;
  `npm test` 93 tests, plus the new vendor tests; `prettier --check` on all
  changed files.
- Not representative locally: `npm run lint` on the Windows checkout reports
  CRLF files that this change does not touch. CI uses LF.
- Not run locally: browser tests. CI runs them.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- None. The Cisco Enterprise ringtone, directory, key and locale work is a
  separate task.

## Next action

Review and merge the PR.

## References

- https://simpleicons.org
