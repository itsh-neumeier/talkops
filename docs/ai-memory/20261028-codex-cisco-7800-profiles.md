# Cisco 7800 MPP provisioning profiles

## Task state

- Task ID: 20261028-codex-cisco-7800-profiles
- Date: 2026-10-28, Europe/Berlin
- Tool: Codex
- Status: Completed implementation; hardware tests and remote CI pending
- Branch: codex/cisco-7800-profiles
- Initial checkout: 60aeda917bbe0c2d03911d9155f596d2af9c5ec0
- Target base: 4b44cd68580908fb54407898aa01b0dcc4f80954 (origin/main)
- Related PR: https://github.com/itsh-neumeier/talkops/pull/89 (draft)
- Implementation commits: `5772d8c`, `dd7344d`

## Goal

Add supported Cisco 7811, 7821, 7841 and 7861 provisioning variants. Preserve
pinned FusionPBX files, secret handling and authenticated device directories.
Commit and submit a draft PR. Do not merge, publish or change release versions.

## Result

Four selectable MPP/3PCC profiles are implemented with capacities 1, 2, 4 and
16. The documentation baseline is MPP 11.3(1). Enterprise firmware remains
unsupported and has no selectable profile. UI text explains this boundary.
No hardware verification is claimed.

The user supplied PID `CP-7861-K9= V09` and Active Load
`sip78xx.14-2-1-0001-14`. Cisco documentation identifies this as Enterprise
14.2(1). The current phone cannot test the MPP profiles. The user performed a
factory reset during identification. No device write or upgrade was performed
by this task.

## Changes

- `profiles/cisco7800.json`, `templates/talkops/cisco7800/` and
  `presets/phones/cisco7800.yaml`: authored manifest, XML and model catalog.
- Build embedding and vendor profile lookup: combine authored profiles without
  changing generated FusionPBX files. Reject duplicate IDs.
- Account bindings: use assigned credentials, registrar port and UDP context.
  Clear all unused hardware slots, including slots above a reduced account count.
- Manufacturer fields: keep identity and account bindings managed. Dictionary
  scripts are encrypted secrets, masked in previews and excluded from copies.
- API source library: keep authored templates outside the MPL upstream listing.
- DE/EN model hints, setup instructions, firmware boundary and device test steps.
- ADR 0048 and Cisco research notes record source evidence and scope.
- Dedicated rendering and API tests cover all four models and device isolation.

## Decisions

- Worktree: `D:/GitHub/talkops/.worktrees/cisco-7800`.
- Preserve primary checkout changes in `tools/coral/train_citrinet_de.py`,
  untracked `build/` and the earlier Cisco status report.
- The Cisco implementation was rebased onto the verified origin/main. The two
  inherited Coral-only commits are excluded. The implementation commit is
  `5772d8c`. It has no dependency on those Coral changes.
- No Cisco 8800 template reuse, TFTP server, firmware conversion or migration.
- No custom keys, BLF or contacts. Only documented account-to-line assignments.
- NTP, voicemail, English and German selection are supported. German requires
  an external firmware-matched dictionary. Time zones are explicitly bounded to
  UTC, London and the documented Central European zones.

## Checks

- Passed: source blob integrity and both existing generators with `--check`.
- Passed: `cargo test -p talkops-provisioning --locked`: 16 unit, 2 Cisco,
  6 profile and 4 vendor tests. All combined profile manifests render.
- Passed: `cargo test -p talkops-api --test multi_vendor --test phone_clone
  --test phones --test provisioning_profiles --locked`: 23 database tests.
- Passed: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Passed: `cargo fmt --all --check`.
- Passed: web lint, Svelte check (zero errors/warnings), 91 tests and build.
- Passed: existing provisioning and manufacturer browser flows at
  320/768/1280 widths with light/dark themes.
- Passed: Cisco DE/EN firmware and untested hints in the model selector and
  device settings. The check is part of CI. An intermediate build overlapped
  branch rebasing and omitted the selector hint. A consistent rebuild and the
  final browser checks passed.
- Passed: final targeted Cisco fixture rerun, including malformed XML,
  undeclared references, account limits and firmware-profile rejection.
- Windows Wiki test: 12 passed; symlink test unavailable due Windows permissions.
- Linux Wiki tests: all 13 passed. The CLI check passed on the host. The Linux
  CLI could not resolve the Windows worktree `.git` link; this is an environment limit.
- Initial web lint reported CRLF checkout lines. Normalize working files to LF
  without content changes. The repeated web lint passed.
- STE review: repository clarity rules applied. Full official standard unavailable;
  full language review pending.

## Dependencies and open work

- Real MPP device tests remain pending. Available hardware has Enterprise firmware.
- Branch pushed and draft PR 89 created. Remote CI is pending.
- No merge, release publication or version change occurred.
- No full workspace runtime test, LDAP/backup service suite, Docker image build
  or SIPp call test was run locally. These are outside the changed runtime paths.

## Next action

Review the draft PR and its exact-head CI results. Run the documented device
test on verified MPP hardware before changing `device_validation`. The supplied
Enterprise phone requires a separate supported infrastructure task; do not
convert or upgrade it as part of this contribution.

## References

- [Research](../research/cisco-7800.md)
- [ADR 0048](../adr/0048-authored-provisioning-profiles.md)
- `docs/ai-memory/20261017-codex-provisioning-all-vendors-proposal.md`
- Earlier untracked status report in the primary checkout:
  `docs/ai-memory/20261028-codex-cisco-7800-status.md`
