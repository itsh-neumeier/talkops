# Outbound dialing permissions

## Task state

- Task ID: 20261023-claude-outbound-dialing-permissions
- Date: 2026-10-23
- Tool: Claude Code
- Status: Completed
- Branch: `claude/confident-ride-y5zf5z`
- Base commit: `efe4b79` (origin/main, release 1.8.8)
- Related commits or PR: Pending

## Goal

Add dialing permissions for outbound calls: tenant-wide dialing profiles
with country, prefix and number rules, a default profile, a profile per
extension, enforcement on every path out of TalkOps, an API, a web UI,
backup coverage, tests and documentation. Emergency calls must never be
blocked. An update must not change the behavior of an existing system.

## Result

Local work and tests are complete in one change set (ADR, core, dialplan,
API, UI, documentation). The task did not need a split into two PRs.

- ADR 0045 records the model, the evaluation, the profile for each path,
  the blind-transfer marking and the upgrade behavior.
- Migration `20261110000001_dialing_permissions.sql` adds
  `dialing_profiles`, `dialing_rules`, `extensions.dialing_profile_id` and
  extends `cdr_blocked_reason_valid` with outbound `dialing:*` reasons. It
  seeds five profiles per tenant.
- `talkops_core::dial_permissions`: pure `evaluate`, `check`,
  `number_to_check`, storage functions with audit entries, `PREMIUM_RULES`.
- Dialplan enforcement: direct dialing, SIP addresses, `*72`, forwarding
  (`forward_external`), blind transfer (`transfer_external`), conference
  loopback legs (through `plan_internal`). Fax: dispatch and submit.
  Web call settings: forwarding target check.
- API: `/api/v1/dialing-profiles` (CRUD, `/{id}/default`, `/test`,
  `/premium-rules`) and `PUT /api/v1/extensions/{id}/dialing-profile`.
  New permission `dialing.manage` (routing area, sensitive).
- Web: tab **Call routing → Dialing permissions**, profile editor with
  country flags, premium-rate button, number test, profile select in the
  extension dialog, outbound block reasons in the call list.

## Changes

- `docs/adr/0045-outbound-dialing-permissions.md`, `docs/adr/README.md`: ADR.
- `migrations/20261110000001_dialing_permissions.sql`: schema and seeds.
- `crates/talkops-core/src/dial_permissions.rs`: model, evaluation, storage, unit tests.
- `crates/talkops-core/src/{lib,extensions,roles,prompts}.rs`: module, extension field, permission, prompt `dialing_blocked`.
- `crates/talkops-api/src/fsxml/dialplan.rs`: enforcement, transfer marks (`xfer_mark`), queue mark reset.
- `crates/talkops-api/src/fsxml/cdr.rs`: accept outbound `dialing:*` reasons.
- `crates/talkops-api/src/routes/dialing.rs`, `routes/mod.rs`, `lib.rs`, `routes/fs.rs`: API and routing context (`geo`).
- `crates/talkops-api/src/routes/extensions.rs`: forwarding target check.
- `crates/talkops-api/src/fax.rs`, `routes/fax.rs`: fax check.
- `crates/talkops-api/tests/dialing.rs` (new), `tests/freeswitch.rs`, `tests/backup.rs`: tests.
- `web/src/lib/dialing.ts`, `web/src/lib/components/routing/DialingProfiles.svelte`, `web/src/routes/routing/+page.svelte`, `web/src/lib/components/ExtensionForm.svelte`, `web/src/lib/components/CallBlockStatus.svelte`, `web/src/lib/calls/blocking.ts`, `web/src/lib/api.ts`, `web/src/lib/i18n/{en,de}.ts`, `web/tests/dialing.test.ts`: UI and tests.
- `docs/en/call-routing.md`, `docs/de/anrufsteuerung.md`, `docs/en/users-and-roles.md`, `docs/de/benutzer-und-rollen.md`: user documentation.

## Decisions

- Most specific rule wins (number, longest prefix, country, base mode);
  deny wins a tie. Rule order has no effect, so the UI sorts rules.
- Emergency numbers are never evaluated. Service numbers are evaluated as
  `+<country code><digits>`.
- No profile on users. Fax requires that each enabled extension of the
  owner allows the number; an owner without extension uses the default.
- Upgrade: a tenant with an extension or a trunk gets **Unrestricted** as
  default. A new installation gets **International** (premium blocks).
  A tenant without a default profile has no limit.
- Blind transfer: FreeSWITCH gives no reliable transferor. The dialplan
  adds the per-leg variable `api_on_answer_talkops_xfer=uuid_setvar ${uuid}
  talkops_xfer_extension_id <ext>` to each device leg and exports
  `nolocal:talkops_xfer_extension_id` from the caller. Queue calls use
  `cc_agent`. Without a mark: the default profile.
- A directory variable or `dial-var-*` cannot carry the mark: FreeSWITCH
  `var_check` refuses values with `${` on new channels (checked in the
  FreeSWITCH 1.10.12 source: `switch_ivr_originate.c`, `mod_dptools.c`).
- A blocked forward rings the extension instead of failing the call.
- Profile assignment has its own endpoint; `PUT /extensions/{id}` does not
  change it, so `extensions.manage` alone cannot change dialing rights.
- Premium list sources: Bundesnetzagentur (DE), RTR KEM-V 2009 (AT),
  BAKOM/OFCOM (CH), ITU-T E.164 list. Shared-cost `+49180` is not blocked.

## Checks

- Passed: `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with a local PostgreSQL 16,
  including the new `tests/dialing.rs` (10 tests), `tests/freeswitch.rs`
  (25 tests), `tests/backup.rs` and the core unit tests. The first run
  found two test problems that this change caused; both are fixed and the
  affected targets passed again (`talkops-core --lib`, `tests/webrtc.rs`):
  `tests/webrtc.rs` dialed `+4912345678`, which the new default blocks
  (012 range), so the test now dials `+4930123456`.
- Failed (environment): `tests/fax.rs::read_only_fax_directory_rejects_upload_without_queueing`.
  The container runs as root (uid 0), so a `0o555` directory stays
  writable. The test does not touch this change; CI runs as a normal user.
- Passed: `npm run lint`, `npm run check`, `npm test` (61 tests), `npm run build` in `web/`.
- Not run: browser tests (`tests/browser/*.mjs`), SIPp E2E (`tests/e2e/run.sh`)
  and the Docker stack. They need Playwright browsers and Docker. No SIPp
  REFER case was added.
- STE review: written with the rules in `docs/documentation-style.md`. The
  official ASD-STE100 standard was not available; full review pending.

## Dependencies and open work

- Migration ID: `20261110000001`. Check for duplicates with other open PRs.
- Open: a SIPp E2E case for a blocked call and a REFER case for the
  blind-transfer mark (needs the Docker stack).
- Open: hardware check of the `api_on_answer` mark with a real blind
  transfer (Yealink, softphone).

## Next action

Release manager: review and integrate the PR; add the release notes.

## References

- ADR 0045, ADR 0044 (geolocation), ADR 0029 (roles).
- `docs/ai-memory/20261023-claude-call-map.md` (geo data).
