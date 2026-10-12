# Codec profiles per device (PR 1 of 3)

## Task state

- Task ID: 20261012-claude-codec-profiles
- Date: 2026-10-12
- Tool: Claude Code
- Status: In progress (PR 1 ready for review; PR 2 and PR 3 not started)
- Branch: `claude/eager-galileo-oj3e5d`
- Base commit: `4503fb3` (`chore(release): release TalkOps 1.7.6 (#1)`) on the
  remote default branch `ccr-df1c8083-0ce1he`
- Related commits or PR: see the PR of this branch

## Goal

The user wants a robust codec for provisioned Wi-Fi handsets (Yealink
AX86R) in bad Wi-Fi networks. TalkOps must transcode between the codec of
the handset and the codec of the other party.

The user approved a plan with three PRs:

1. Codec profiles per device (`standard`, `hd`, `robust`), `mod_ilbc` in the
   FreeSWITCH image, provisioning of the Yealink codecs. This task.
2. Call quality analysis: store packet loss, jitter and MOS per device leg
   from the CDR, show the quality per device, list devices with bad quality.
3. Automatic profile selection: configurable rules (tenant default, device
   override) select `hd` or `robust` from the recorded quality.

## Result

PR 1 is complete on the branch. Facts:

- The AX86R supports G.722, G.711, G.729, G.726, G.723.1 and iLBC. It does
  not support Opus (user screenshot of the phone web UI). The AX86R had only
  PCMU and PCMA enabled.
- `devices.codec_profile` stores the profile. `NULL` is automatic:
  Wi-Fi handsets use `hd`, all other devices use `standard`.
- Calls to a device: the directory params `dial-var-absolute_codec_string`
  and `dial-var-jitterbuffer_msec` set the device leg. `mod_dptools`
  `user_outgoing_channel` (FreeSWITCH v1.10.12) deletes the exported variable
  of the same name and adds the `dial-var-*` value.
- Calls from a device: the directory variable `talkops_codec_profile`
  identifies the profile. `with_caller_codecs` in `fsxml/dialplan.rs` sets
  the caller leg first and changes the plan's `absolute_codec_string`
  actions to `export nolocal:`.
- Provisioning writes `account.X.codec.<payload>.enable/.priority` for
  devices with `hd` or `robust`: G.722 (1), iLBC (2/3), PCMA (4), PCMU (5),
  all others disabled.

## Changes

- `migrations/20261027000001_device_codec_profile.sql`: enum
  `codec_profile` and column `devices.codec_profile`.
- `crates/talkops-core/src/extensions.rs`: `CodecProfile`, field on
  `Device`, `DeviceInput`, `DeviceAuth`; create and update store it.
- `crates/talkops-core/src/phones.rs`: `PhoneAccount` has `kind` and
  `codec_profile`.
- `crates/talkops-api/src/fsxml/codecs.rs`: profile to codec list and jitter
  buffer.
- `crates/talkops-api/src/fsxml/directory.rs`: `dial-var-*` params and the
  `talkops_codec_profile` variable.
- `crates/talkops-api/src/fsxml/dialplan.rs`: `with_caller_codecs`.
- `crates/talkops-api/src/routes/extensions.rs`: audit entry contains the
  profile.
- `crates/talkops-api/src/routes/provisioning.rs`,
  `crates/talkops-provisioning/src/yealink.rs`,
  `crates/talkops-provisioning/templates/yealink/mac.cfg`: codec settings.
- `docker/freeswitch/Dockerfile`, `build-modules.conf`,
  `conf/autoload_configs/modules.conf.xml`: `libilbc` v0.0.1 (commit
  `7cee47f`) and `mod_ilbc`.
- `.github/workflows/freeswitch.yml`: checks that `show codec` lists iLBC.
- `web/src/lib/api.ts`, `web/src/lib/components/DeviceList.svelte`,
  `web/src/lib/i18n/en.ts`, `web/src/lib/i18n/de.ts`: profile selection in
  the device dialog and a badge in the device list.
- Tests: `crates/talkops-api/tests/freeswitch.rs`
  (`codec_profiles_pin_the_device_leg`), `crates/talkops-api/tests/phones.rs`
  (`codec_profiles_set_the_phone_codecs`), `crates/talkops-core/tests/domain.rs`,
  unit tests in `codecs.rs`, `directory.rs`, `extensions.rs`, `yealink.rs`,
  SIPp `tests/sipp/uas_ilbc.xml`, `tests/sipp/call_ilbc.xml` and a new
  section in `tests/e2e/run.sh`.
- Documentation: `docs/adr/0023-device-codec-profiles.md`,
  `docs/en/codec-profiles.md`, `docs/de/codec-profile.md`, links in
  `README.md`, `docs/adr/README.md`, `docs/en/yealink.md`, `docs/de/yealink.md`.

## Decisions

- Use `dial-var-*` in the directory instead of changes to every bridge
  string. One place covers extensions, ring groups, queues and ESL calls.
- Wi-Fi handsets default to `hd`. `robust` is a manual selection until PR 3.
- The phone gets the same codec list for `hd` and `robust`. TalkOps selects
  the codec per call. A change between `hd` and `robust` needs no resync.
- Provision `ilbc`, `ilbc_15_2kpbs` and `ilbc_13_33kpbs`. The AX83H manual
  documents `ilbc`. The SIP-T2 manual V85 documents the two rate names
  (spelled `kpbs`). The AX86R UI shows two iLBC rates. Yealink documents
  that a phone ignores unknown payload names.
- No DSCP provisioning: Yealink sets RTP DSCP 46 by default
  (`network.qos.rtptos`), and a change restarts the phone.
- ADR number 0023: open branches use 0020 (`codex/admin-softphone-selection`),
  0021 (`codex/guided-backup-restore`) and 0022 (`codex/active-call-overview`).
- Migration ID `20261027000001`: open branches use `20261026000002` and
  `20261026000003`.

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with PostgreSQL 17, except
  `backup_and_restore`.
- Failed (environment): `backup_and_restore` needs PostgreSQL 17 client tools.
  The local `pg_dump` is version 16. CI installs version 17.
- Passed: `npm run lint`, `npm run check`, `npm run build` in `web/`.
- Passed: `libilbc` v0.0.1 builds with GCC 14.2 in `debian:trixie-slim`, and
  `pkg-config --modversion ilbc` gives `0.0.1`.
- FreeSWITCH with `mod_ilbc`: see the PR for the result of the local build.
- Not run: `docker build` of `docker/freeswitch` (the sandbox blocks the
  proxy access that the build needs). CI workflow `FreeSWITCH image` runs it.
- Not run: `tests/e2e/run.sh` (needs the full Docker stack and SIPp). CI
  workflow `E2E` runs it.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- Migration ID: `20261027000001`.
- Upgrade: existing Wi-Fi devices use `hd` after the upgrade. Calls to them
  offer G.722, G.711 and no Opus or video. After a resync, provisioned AX
  handsets enable G.722 and iLBC.
- PR 2 (quality analysis) and PR 3 (automatic rules) are open.
- `claude/modest-dirac-fhox1w` changes the CDR handling (spool). PR 2 must
  start after that branch merges or must merge it.

## Next action

Wait for the review of PR 1. Then start PR 2 from the updated target branch.

## References

- `docs/adr/0023-device-codec-profiles.md`
- `docs/release-workflow.md`
- FreeSWITCH v1.10.12 `src/mod/applications/mod_dptools/mod_dptools.c`
  (`user_outgoing_channel`, `dial-var-`), `src/switch_core_media.c`
  (`switch_core_media_prepare_codecs`, `jitterbuffer_msec`).
