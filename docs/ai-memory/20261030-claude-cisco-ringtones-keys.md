# Cisco Enterprise ringtones, speed dial keys and locale per phone

## Task state

- Task ID: 20261030-claude-cisco-ringtones-keys
- Date: 2026-10-30
- Tool: Claude Code
- Status: In progress
- Branch: claude/cisco-ringtones-keys
- Base commit: 5e13f5d (main, v1.8.22)
- Related commits or PR: to be added

## Goal

The user asked for Cisco Enterprise ringtones, a key layout and the uploaded
Cisco locales in the phone language block.

## Result

- Ringtones: the host had no uploaded ringtones and no selected ringtone. The
  exporter added the ring list only with a selected ringtone. It now adds all
  ringtones; a selected ringtone stays first. TalkOps does not ship Cisco
  ringtones; the user uploads WAV files.
- Keys: the 7821, 7841 and 7861 Enterprise profiles have the key group `line`.
  Button 1 is the SIP line; keys 2 and higher render as speed dials
  (`featureID` 21, as in the FusionPBX 79x1 reference). BLF keys also render as
  speed dials: the busy lamp needs CUCM presence. Line buttons now use
  `loop.index`.
- Locale per phone: vendor setting `cisco_enterprise_locale` in all four
  Enterprise profiles. `select_cisco_locale(locales, chosen, language)` uses
  the chosen name, else the language. The UI shows a select in
  `DeviceLocaleFields` for Cisco Enterprise phones.

## Decisions

- Phone language stays `en`/`de` for all vendors; Cisco packages for other
  languages are selected per phone.
- No BLF lamp support without CUCM.

## Checks

- Passed (rust:1-trixie + Postgres 17): `cargo fmt`, `cargo clippy -p
  talkops-provisioning -p talkops-api --all-targets -D warnings`,
  `cargo test -p talkops-provisioning`, `cargo test -p talkops-api --test
  phones --test multi_vendor`.
- Passed (node:22): `npm run check`, `npm test`, `npm run build`, Prettier on
  changed files.
- Not done: device test of speed dials and ringtones.

## Dependencies and open work

- Release together with PR113 (directory menu) and PR114 (provisioning files).
- The DE hint text points to the Cisco Enterprise tab from PR114.

## Next action

Open the PR, then release 1.8.23 with PR113 and PR114.

## References

- docs/ai-memory/20261030-claude-cisco-locale.md
- docs/ai-memory/20261030-claude-provisioning-files.md
