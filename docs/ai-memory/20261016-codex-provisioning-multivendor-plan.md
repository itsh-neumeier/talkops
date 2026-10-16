# Provisioning profiles and complete Yealink template library

## Task state

- Task ID: 20261016-codex-provisioning-multivendor-plan
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: In progress. Approved implementation; final checks running.
- Branch: codex/provisioning-multivendor-plan
- Base commit: 86772ec0c31990f7a29a639ffb90db6ebef217e2
- PR: https://github.com/itsh-neumeier/talkops/pull/49
- Proposal commits: 7610aa5, 160c205
- Initial implementation commit: 317dd57
- Hardware mapping and regression follow-up: 4ce322e

## Goal

Complete T41P key provisioning. Import the full FusionPBX Yealink source library.
Prepare model and vendor driven forms. Support explicit phone transport settings
and diagnose failed HTTPS resync downloads. Submit a tested PR for the release chat.

## Result

The user reviewed the proposal and approved implementation. Added programmable
keys, expansion module profiles, ordered call-state softkeys, transport overrides,
a redacted draft preview, a source library and versioned native templates.
The API regression and new database tests pass. Production UI checks pass.
Linux CI passed Rust checks, web checks and both image builds at 317dd57.
Final follow-up checks and E2E remain in progress. No production phone, DHCP or release setting changed.

## Changes

- `crates/talkops-provisioning/reference/`: All 433 original Git blobs from 70
  upstream directories at be0d0e3220e6b3666775fa46db6f1accc60c79c6. Includes
  provenance, MPL-1.1 license and a reproducible source verification check.
- `crates/talkops-provisioning/profiles/yealink.json`: Explicit key capabilities
  for the existing 21 desk models. Unknown vendors receive no Yealink fallback.
- `crates/talkops-provisioning/src/{adapter,keys,library}.rs`: Vendor dispatch,
  semantic assignments, endpoint validation and bounded template evaluation.
- `crates/talkops-core/src/phone_provisioning.rs`: Tenant options, immutable
  template revisions, active pointers and bounded configuration request records.
- `crates/talkops-api/src/routes/provisioning_admin.rs`: Privileged, audited
  transport and template operations. Source inspection does not execute Smarty.
- `crates/talkops-api/src/routes/{phones,provisioning}.rs`: Boot and softkey files,
  per-device endpoints, draft previews, request results and old-client compatibility.
- `web/src/lib/components/PhoneProvisioningEditor.svelte`: Capability driven
  programmable and expansion keys, contact numbers and call-state layouts.
- `web/src/lib/components/ProvisioningTransport.svelte`: Server settings and
  recent download results, separate from the SIP NOTIFY result.
- `web/src/routes/phones/templates/+page.svelte`: Source viewer, native template
  cloning, immutable drafts, validation, activation and revision restore.
- `migrations/20261105000001_provisioning_profiles.sql`: Phone overrides, tenant
  settings, template revisions and diagnostic records.
- `docs/adr/0038-vendor-provisioning-profiles.md`, `docs/en/provisioning.md` and
  `docs/de/provisionierung.md`: Design, operation, diagnosis and upgrade notes.
- `.github/workflows/ci.yml`: Source integrity and provisioning browser checks.

## Decisions

- The user approved the reviewed proposal with "Passt so gerne übernehmen".
  Commit, push and PR publication are authorized. The release chat owns merging,
  versions, tags and publication.
- Use only the task worktree at
  C:/Users/timo/.codex/worktrees/provisioning-multivendor-plan/talkops.
  The original checkout's local commit and untracked Dahua summaries are preserved.
- The remote default is main. Its head still matches the base commit.
- T41P firmware is 36.83.0.120. Its programmable IDs are 1–10, 13, 17 and 18.
  Copied source blocks do not establish hardware capabilities. The T41P profile
  exposes no expansion module. All new mappings need real-device verification.
- Keep raw Smarty reference files separate from native MiniJinja templates.
  Source folders do not imply 70 supported active models. Adapted models remain
  the existing catalog; additional vendor renderers are extension points.
- Use modern codes by default and an explicit legacy code profile.
- Reset sends effective profile defaults. Omission alone would retain old values.
  A custom mute key needs features.keep_mute.enable=0; reset restores 1. Direct
  pickup of a numeric extension emits *8*<extension>. Explicit codes stay intact.
  Modern XML Group type 22 selects the remote phonebook index; source type 47 is
  an explicit legacy mapping. Source-only redial and line-local-directory entries
  are not offered as modern functions. T57W omits unsupported account switching.
- Templates have tenant/model scope, immutable revisions, strict variables,
  output/evaluation/recursion limits and no filesystem or network loader.
- Both URLs used /provisioning/. The initial chat spelling was only a typo.
- AX86R HTTPS works via DHCP option 66. T41P HTTPS resync fails; LAN HTTP resync
  works immediately. The root cause remains unconfirmed. DHCP 66 delivers an
  address and does not prove a TFTP transfer. No global TLS checks are disabled.
- Device endpoint overrides apply to common values repeated in the MAC file,
  assets and callbacks. This prevents a web HTTPS request from replacing a saved
  phone HTTP exception. The server default remains available to other phones.

## Checks

- Passed: Original source research and comparison of all 433 Git blobs.
- Passed: Initial 10 existing phone API tests and 3 new PostgreSQL integration
  tests for overrides, preview redaction, resync, templates and tenant isolation.
- Passed: Web lint, Svelte check (0 errors, 0 warnings), 34 web unit tests and build.
- Passed: All 13 provisioning unit tests and 5 renderer/profile integration tests,
  including all active native models,
  unsupported slots, numeric profiles, reset, source lookup and bounded rendering.
  Also checked all keys in a full six-module setup, mute hardware gating, direct
  pickup codes and restoration of the profile defaults.
- Passed: Expanded preview checks with a real assigned SIP account: secret redaction,
  unsaved labels and rejection of unrelated account IDs. Preview does not write labels.
- Passed: Existing browser regressions for all 21 desk models and general layouts.
- Passed: New provisioning browser flows at 320/768/1280 px. Stored light/dark theme
  is asserted after navigation. Inspected phone and desktop screenshots.
- Passed: Clippy with warnings denied for API, core and provisioning, all targets.
- Passed at 317dd57 in Linux CI: full workspace fmt, clippy and tests; web checks;
  server and worker image build/smoke tests; Compose validation.
- In progress: Final follow-up checks and E2E.
- Local full workspace Clippy fails on Windows in unchanged media-worker code:
  realtime::Streams::is_empty is unused in the Windows test target. The Linux
  workspace CI Clippy check passes. Local Docker tests cannot run because the
  Docker Desktop Linux daemon is not running. CI supplies the Linux checks.
- Earlier failures: C: filled during a debug test build. Cleaned only task package
  outputs with cargo clean, then moved test outputs to the larger A: volume.
  Browser checks found a Svelte proxy clone error; snapshot cloning fixes it.
- Not run: Actual T41P/AX86R hardware tests and packet capture. No device access.
- Review: CodeRabbit CLI is absent. Use direct code review; do not claim a CLI run.
- STE review: Applied repository clarity rules. The full standard was unavailable.
  Full language review remains pending.

## Dependencies and open work

- Required PR dependencies: None. Includes the already merged PR 47 behavior.
- Migration ID: 20261105000001. Apply before starting the new API.
- Cargo: Enable existing MiniJinja loader/fuel features. Add direct serde_json and
  url use to the provisioning crate. No dependency versions or release versions
  changed. Cargo.lock has one new dependency edge.
- Keep existing secrets and assignments. Clients that omit provisioning preserve
  saved overrides. Templates become effective after activation, assignment and a
  phone download; activation alone does not send a resync.
- HTTPS root cause needs a phone-level TLS/DNS/auth comparison. TLS failures before
  HTTP leave no TalkOps request entry. The HTTP exception is a LAN workaround.

## Next action

Finish final checks, fix findings, update this summary and the PR, then hand off
the verified feature branch to the release chat. Do not merge or publish a release.

## References

- docs/provisioning-multivendor-proposal.md
- docs/research/yealink-template-inventory.md
- docs/research/yealink-template-inventory.json
- docs/ai-memory/20261016-codex-yealink-t41p-dss.md
- docs/ai-memory/20261012-codex-yealink-internal-phonebook.md
- docs/release-workflow.md
