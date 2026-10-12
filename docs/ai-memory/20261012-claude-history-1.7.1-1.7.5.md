# Claude Code work from 1.7.1 to 1.7.5 (backfill)

## Task state

- Task ID: 20261012-claude-history-1.7.1-1.7.5
- Date: 2026-10-12
- Tool: Claude Code
- Status: Completed
- Branch: `claude/ai-memory-backfill`
- Base commit: `4503fb3` (release 1.7.6 on `ccr-df1c8083-0ce1he`)
- Related commits or PR: commits `b5c0ca3` to `1a77879` (listed below); PR Pending

## Goal

Record the Claude Code work that was done before the shared task memory
existed. Other chats and models can then continue this work without the old
chat history.

## Result

This file records the facts of 15 commits between tags `v1.7.1` and
`v1.7.5`. All commits are on the target branch `ccr-df1c8083-0ce1he` and in
the releases 1.7.4, 1.7.5 and 1.7.6. This task changes no code.

## Changes

This task adds only this file. The earlier work changed these areas:

### Phone book CSV import (`b5c0ca3`, release 1.7.4)

- `crates/talkops-core/src/contact_import.rs`: CSV parser. It detects the
  delimiter, reads UTF-8 and Windows-1252, and maps Outlook and Google
  Contacts headers.
- `POST /api/v1/contacts/import` with `dry_run` for a preview.
- Sample files: `web/static/telefonbuch-beispiel.csv`,
  `web/static/phonebook-sample.csv`. The numbers are fictional numbers of
  the Bundesnetzagentur.
- Contact numbers keep their written form. `+49 (0)89 …` is accepted.
  TalkOps does not change a contact number to E.164.

### Yealink AX83H/AX86R (`b2f1594`, `44ea6d7`, release 1.7.4)

- `presets/phones/yealink.yaml`: AX models have 16 line keys (firmware
  180.87.0.15 or later), custom ringtone (WAV, 8 MB) and wallpaper.
- AX models have no speed dial key type. The API refuses `speed_dial` for
  the `wifi` family. The BLF pickup code goes to `linekey.X.extension`
  (desk phones: `linekey.X.pickup_value`).
- Comfort settings: a catalog in `settings:` of `presets/phones/yealink.yaml`
  (key tone, charging tone, vibration, cradle answer and hang-up, call
  waiting, backlight, brightness, 12/24 h, notices, noise filter, acoustic
  shield). Values for all phones are in `tenant_settings.phone_settings`
  (`GET/PUT /api/v1/phone-settings`). Values for one phone are in
  `phones.settings`. A phone value overrides the value for all phones.
  An unset key writes no parameter.
- Source: Yealink AX86R documentation on support.yealink.com and the
  Yealink template `AX8X_Autop_Template_180.87.0.5`.

### Yealink dial now (`9e9f38e`, `b5c390e`, `36a73d9`)

- `crates/talkops-provisioning/src/dial_now.rs`: up to 20
  `dialplan.dialnow.rule.X` rules. The rules contain emergency numbers,
  fixed feature codes (`numbering::FIXED_FEATURE_CODES`) and internal
  numbers. Numbers that differ only in the last digit become one rule,
  for example `2[0,1,2]`.
- A number does not get a rule when a longer dialable number starts with
  it. Internal numbers that start with `11` do not get a rule.
- When an area code is set, internal numbers get no rule, because local
  numbers without the area code are dialable.
- System codes `*1` to `*99` have priority over `*<internal number>`.
- `36a73d9` fixed HTTP 500 for all phone configurations when a ring group,
  time condition, menu or queue had no number.

### Internal numbers start at 100 (`a961160`, `19737ed`)

- `numbering::ensure_free` refuses a new internal number below 100
  (`MIN_INTERNAL_NUMBER`). An existing 2-digit number stays valid when it
  does not change.
- `web/src/lib/components/InternalNumberInput.svelte` shows the rule at
  each number field.
- Test data and `tests/e2e/run.sh` use 3-digit numbers (200, 210, 500 …).

### Calls from a trunk to phones (`3e8974b`)

- Inbound calls export `nolocal:absolute_codec_string=G722,PCMA,PCMU,OPUS`
  in `crates/talkops-api/src/fsxml/dialplan.rs`. Before this change, the
  phone leg got only the trunk codec. A Yealink AX86R replied
  `488 Not Acceptable Here`.

### Softphone (`0882c6d`)

- The softphone accepts a call only once and holds only an established
  call. This stops the error `Invalid session state Establishing`.

### Resync and restart (`642b5e2`, release 1.7.5)

- The common file sets `sip.notify_reboot_enable = 0`. Resync sends
  `check-sync;reboot=false`. The phone loads its configuration and does not
  restart. The Restart button sends `check-sync;reboot=true`.
- `Telephony::check_sync` sends an ESL `NOTIFY` event. It first checks the
  registration with `sofia_contact`.

### Web UI (`5393161`, release 1.7.5)

- Phone page: assign an extension (next free account) and remove it.
  The add-phone dialog can assign an extension.
- Phone list: extensions column and one filter for each column.
  `GET /api/v1/phones` returns `extensions` for each phone.
- Menu name: "Phones (provisioning)". Door station setup moved to
  `/door-stations` under Settings. The Door menu item shows only when a
  door station exists.

### Several extensions for one number (`1ce2748`, release 1.7.5)

- `numbers.extra_extensions` (UUID array). The number destination is the
  main extension. Its ring time applies. Unanswered calls go to its
  voicemail. DND extensions and extensions without devices are skipped.
- UI: on the trunk and number pages ("Ring more extensions …") and in the
  extension edit dialog ("Incoming numbers that ring here").
- When an extension is deleted, `trunks::forget_extension` removes it from
  `extra_extensions`.

## Decisions

- Phone parameters come only from official Yealink documentation or the
  official Yealink template. Parameters with unclear values were left out:
  `phone_setting.sleep_time` and the "Off" value of `screensaver.wait_time`.
- Comfort settings apply only to the `wifi` family. Desk phone parameters
  need a separate check.
- Resync must not restart a phone. Restart is a separate action.
- The main extension of a number keeps the old behavior (forwarding,
  voicemail). The extra extensions only ring.

## Checks

- Passed: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  -- -D warnings` and `cargo test --workspace` before each push.
- Passed: `npm run lint` and `npm run check` before each push.
- Passed: local tests with FreeSWITCH 1.10.12 (Docker image) and SIPp:
  codec offer to the phone, check-sync Event header, UDP phone leg.
- Passed: browser checks with Playwright for the new UI pages.
- Failed: CI run for `642b5e2` failed on Prettier in `web/src/lib/i18n/en.ts`.
  Commit `5393161` fixed it.
- Not run: `tests/e2e/run.sh` locally. It needs the full Docker stack.
  CI runs it.
- Not run: tests with real Yealink phones and a real trunk.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- Migrations: `20261025000001_v18_phone_settings.sql`,
  `20261025000002_v18_number_extensions.sql`. Both are released.
- Open: the AX86R `488` reply. Yealink documents G722 as enabled by default.
  The reason for the `488` is not confirmed. A new SIP trace after an update
  is necessary if the problem continues.
- Open: dial now, comfort settings and several extensions per number have no
  test with real phones.
- Open: comfort settings for desk phones (T4x/T5x).
- Handed over: phone book entries for internal numbers dial with area code
  and country code. Codex took this task (branch
  `codex/yealink-internal-phonebook`). Claude Code did not change code for it.

## Next action

None.

## References

- `docs/release-workflow.md`
- `docs/ai-memory/20261012-codex-release-1.7.6.md`
- `docs/de/yealink.md`, `docs/en/yealink.md`
- `CHANGELOG.md` sections 1.7.4 and 1.7.5
