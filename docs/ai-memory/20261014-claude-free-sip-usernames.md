# Free SIP user names for devices

## Task state

- Task ID: 20261014-claude-free-sip-usernames
- Date: 2026-10-14
- Tool: Claude Code
- Status: Completed (PR submitted for review)
- Branch: `ccr-4c0c75c4-dn8jle`
- Base commit: `7b6e51f` (origin/main, TalkOps 1.7.11)
- Related commits or PR: see the PR of branch `ccr-4c0c75c4-dn8jle`

## Goal

Dahua VTO and VTH devices work best with SIP user names that have digits
only. The VTH sends its Room No. as the SIP user name. TalkOps required the
extension number as the start of the name (`8000vto`). The user wants free
SIP user names: extension `8000` can use `6578` or `123`. The only check is
that each name is unique.

## Result

- The SIP user name of a device is free: 3 to 64 characters of
  `a-z A-Z 0-9 . _ -`. It does not depend on the extension number.
- TalkOps checks that the name is unique (all tenants, upper-case and
  lower-case letters are the same) before it writes. The error is
  `409 conflict` with the text `SIP username <name> is already in use`.
  The unique index `devices_sip_username_idx` stays as the final check.
- `web-` (any case) is reserved for browser devices.
- New devices without a name get a random name: 6 characters from
  `abcdefghijkmnpqrstuvwxyz23456789` with at least one letter (`28x1js`).
  Before: `<ext>-<n>` and `<ext>vto` for door stations. Existing devices keep
  their names.
- Active call overview: a raw number first matches an extension number.
  A device SIP user name is the fallback. Before, a device name that looked
  like another extension number could select the wrong extension (list
  order).
- UI: one free input field for the full SIP user name (before: a fixed
  extension-number prefix plus an ending). New hint text in EN and DE.
- Dahua setup guide (UI and docs) uses numeric names (`8000`, `8001`,
  `9902`).

## Changes

- `crates/talkops-core/src/crypto.rs`: `random_lowercase`, shared
  `random_string`.
- `crates/talkops-core/src/extensions.rs`: `validate_sip_username(username)`
  without the extension prefix rule, `ensure_username_free`, random
  `default_username`; create and rename use the uniqueness check.
- `crates/talkops-api/src/active_calls.rs`: extension number before device
  name; unit test `extension_numbers_come_before_device_usernames`.
- Tests: `crates/talkops-core/tests/domain.rs`,
  `crates/talkops-api/tests/{http,doors,freeswitch,phones}.rs`. Fixtures that
  check dialplan strings now set explicit names (`200-1`, `210-1`, `220-1`).
- `web/src/lib/components/DeviceList.svelte`, `web/src/lib/i18n/{en,de}.ts`.
- `docs/en/door-station.md`, `docs/de/tuersprechstelle.md`,
  `docs/en/door-station-dahua.md`, `docs/de/tuersprechstelle-dahua.md`.

## Decisions

- User decision (variant A): a SIP user name can be the number of a
  different extension. Only uniqueness is checked. Calls still go to the
  extension number. The directory sets the caller ID to the extension number.
- User decision: generated names are random and not derived from the
  extension number.
- No migration: the database check
  (`^[a-zA-Z0-9._-]{3,64}$`) and the unique index already match the new rule.
- Generated names have at least one letter, so that they do not look like
  an extension number. Look-alike characters (`l`, `o`, `0`, `1`) are not
  used, because admins type the name into devices.
- No ADR: the change does not change an architecture decision.

## Checks

- Passed: `cargo fmt --all -- --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with local PostgreSQL 16.
- Passed: `npm run lint`, `npm run check`, `npm run build` in `web/`.
- Not run: `tests/e2e/run.sh` (Docker/SIPp stack).
- Not run: hardware test with Dahua VTO and VTH and numeric names.
- STE review: manual review against `docs/documentation-style.md`.

## Dependencies and open work

- Migration IDs: none.
- Upgrade: existing devices keep their SIP user names. No action is
  necessary. To use numeric names on Dahua devices, rename the device
  (**Edit**) and enter the same name on the device.

## Next action

Release chat: review and integrate the PR. User: rename the VTO and VTH
devices to numeric names and test the registration.

## References

- `docs/ai-memory/20261013-claude-door-sip-names-dtmf.md`
- `docs/ai-memory/20261013-claude-dahua-vth-setup.md`
- `docs/en/door-station-dahua.md`
