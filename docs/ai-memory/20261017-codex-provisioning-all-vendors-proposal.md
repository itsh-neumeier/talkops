# All-vendor provisioning, manufacturer fields and phone copies

## Task state

- Task ID: 20261017-codex-provisioning-all-vendors-proposal
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: Completed implementation; PR records final CI and readiness
- Branch: codex/provisioning-all-vendors-proposal
- Base commit: c4765b0495adf004f0b2b0f5099cbd098f384f86
- Related PR: https://github.com/itsh-neumeier/talkops/pull/51
- Implementation commits: 72f0831 (engine), 972919d (API, core and UI),
  08a256c (delivery references, contacts and secret PIN handling), and
  e2d9463 (contact number types).
  Git history records later documentation.

## Goal

Implement all usable FusionPBX provisioning templates. Add categorized model
settings, language and time controls, and same-model phone copies. Preserve
independent device identities and credentials. Commit, push and maintain the
PR. The release chat owns merges, versions, tags and publication.

The user approved implementation with "nein passt so mach das". The user then
required language settings and comprehensive categorized manufacturer fields.
The user explicitly accepted Spectralink as a missing-template entry.

## Result

The implementation contains 386 adapted profiles: 385 usable source profiles
and the explicit GXP1630 selector variant. The complete pinned tree contains
1,037 files and 31 vendor/app directories. Spectralink has only an empty file.
Grandstream Wave has an empty template. Both entries remain visible and blocked.
Source-template coverage and device verification remain separate states.

## Changes

- Provisioning crate: shared account context, constrained Smarty conversion,
  native manifests, bounded rendering, XML validation, vendor function catalog,
  common locale mappings and scalar field schemas.
- Sources: 1,037 original provisioning blobs and two original app/protocol blobs
  at 20143aec4366afc2755f336d3058243479d49d44. TalkOps executes no upstream PHP.
- Adapters: correct credential bindings, selectors, malformed XML, account
  aliases, Fanvil X7C key codes and auxiliary file URLs. Coalesce T4x boot files.
  CP965 reuses pinned Yealink directory files. Sangoma uses the explicit Htek
  P4401 directory-format alias. Clear missing optional Swissvoice directory
  and Snom firmware defaults. Originals remain unchanged.
- Model catalog: vendor-prefixed IDs, extra device families, source account
  bounds and configurable baseline counts. Existing Yealink IDs stay stable.
- Core/API: encrypted and masked manufacturer overrides, scoped artifact and
  QR exports, parameter schemas, transaction-aware phone and account creation,
  atomic same-model copies and audit records.
- UI: categorized source fields, search and pages, source comments, managed
  fields, saved secrets, native key functions, account count, locale controls,
  files and copy preview. Primitive schema dependencies preserve input focus.
- Documentation: ADR 0039, English/German guides and source adaptation notes.
- Tests: all-profile fixtures, field parsers, clone and manufacturer database
  integration tests, and production browser flows.

## Decisions

- Exclusive worktree:
  C:/Users/timo/.codex/worktrees/provisioning-all-vendors-proposal/talkops.
  Preserve the primary checkout, old PR 49 worktree and foreign Dahua changes.
- Remote default target main was reverified at the base commit. PR 49 is merged.
- Preserve original hashes and MPL attribution. Independently authored Groundwire
  web contacts export uses the TalkOps project license.
- Override allowed value ranges only. Escape XML and reject unsafe text values.
  Encrypt secret fields and URLs with credentials through the existing store.
- Copies exclude identity, runtime, source labels, secrets and device-specific
  values. Generate independent SIP credentials. Defer keys until an account is
  assigned. Validate before opening the transaction, then lock and recheck the
  source. Create the phone, accounts and audit entries in the same transaction.
- Native functions use source catalog UUIDs and vendor/group constraints.
  Unsupported semantic mappings fail explicitly. No native resync/reboot is
  exposed without a remote command adapter.
- Field coverage means settings present in the source templates. It does not
  claim every undocumented or later firmware feature. Keep device_validation
  at not_tested. Manufacturer fields expose source-specific locale formats.
- Spectralink stays blocked as explicitly requested. Wave has no usable source.

## Checks

- Passed: node tests/provisioning/reference.mjs. All 1,037 original blobs,
  previous 433 Yealink blobs and two app/protocol blobs match their hashes.
- Passed: native and function generators with --check.
- Passed: cargo test -p talkops-provisioning --locked (16 unit, 6 profile and
  3 native tests). Final --lib --test vendors run passed 16 unit and 4 native
  tests after PIN classification. All 386 adapted profiles pass account,
  structure, file-reference, contact-number and field-schema fixtures.
- Passed: cargo test -p talkops-api --test multi_vendor --test phone_clone
  --test phones --test provisioning_profiles --locked: 22 database tests.
  Includes native functions, encrypted lock PINs, clone credential separation,
  deferred keys, tenant scope, role checks and atomic rollback.
- Passed: web lint, check (zero errors/warnings), 35 unit tests and build.
  Both final browser flows passed after the last UI build: manufacturer and
  existing Yealink provisioning; 320/768/1280 widths and light/dark themes.
- Passed: cargo clippy -p talkops-provisioning -p talkops-core -p talkops-api
  --all-targets --locked -- -D warnings after the final corrections.
- Fixed during checks: clone pool starvation, schema focus resets, old preview
  manifests, source artifact aliases and missing auxiliary files. Contact
  fixtures distinguish an empty category from a missing entire contact export.
- CodeRabbit CLI is unavailable. The skill was read; the user was informed.
  Manual review covered transactions, tenant/role checks, file paths, secrets,
  model changes, source mappings and field reactivity. No CodeRabbit run occurred.
- Not run locally: full Linux workspace/services, Docker, Compose, SIPp and
  softphone E2E. Windows checks use the MSVC toolchain. PR 51 records the final
  pushed head and its CI/E2E results. Source-only CI on 576f8b3 is not evidence
  for the implementation.
- STE review: repository clarity rules applied. Full official standard unavailable;
  full language review pending.

## Dependencies and open work

- Required PR dependencies: None. PR 49 is already in the base.
- New migration IDs: None. Existing JSON-storage baseline: 20261105000001.
- Dependencies: existing locked md-5 0.10.6, roxmltree 0.21.1, chrono and
  chrono-tz. MiniJinja enables loop controls. No release version changes.
- Real device/firmware checks remain pending. Registration, LEDs, language
  dictionaries and firmware/media downloads require device-specific checks.
  Some old Cisco devices need separate TFTP delivery. Cloud Softphone requires
  the user's Cloud ID and portal configuration. Firmware files are not bundled.
- PR 51 records final CI and readiness. Keep it a draft if checks fail.
- No further code task is known. Release integration and hardware checks remain
  separate work. Do not change device_validation without actual device evidence.

## Next action

Use PR 51 for review and release integration after its exact-head checks pass.
The release chat owns merging, version selection, tags and publication.
No migration is added. Do not message other chats without user authorization.

Local runtime:
- DATABASE_URL=postgres://postgres@127.0.0.1:55439/postgres
- CARGO_TARGET_DIR=A:/aislop/codex/talkops/all-vendor-tests/target
- CARGO_PROFILE_DEV_DEBUG=0; CARGO_PROFILE_TEST_DEBUG=0
- Toolchain: cargo +stable-x86_64-pc-windows-msvc
- Logs: C:/Users/timo/.codex/tmp/talkops-all-vendors-tests
- Browser: temporary Playwright and installed Microsoft Edge
- Preview: own web directory, port 5184; restart after a build

## References

- docs/provisioning-all-vendors-proposal.md
- docs/research/fusionpbx-adaptation-notes.md
- docs/adr/0039-source-derived-multi-vendor-provisioning.md
- docs/en/manufacturer-provisioning.md
- docs/de/hersteller-provisionierung.md
- docs/ai-memory/20261017-codex-release-1.8.2.md
- docs/release-workflow.md
