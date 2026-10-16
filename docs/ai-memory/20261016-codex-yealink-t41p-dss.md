# Yealink desk phones and graphical key editor

## Task state

- Task ID: 20261016-codex-yealink-t41p-dss
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: Implementation complete and published for review. See the PR for
  the checks and readiness of its current head. Release integration is separate.
- Branch: codex/yealink-t41p-dss
- Base commit: 7d7990b99e9f02c126ad3983da633476a22a9708
- Application commit: 29c5a6f5fc2662712c4a641625898a9e7e7e181e
- Related PR: https://github.com/itsh-neumeier/talkops/pull/47

## Goal

Add T41P provisioning and every Yealink desk phone on the CPBX help site.
Add model graphics and selectable function keys with a phonebook dialog.
The user reviewed the interactive proposal and approved implementation on
2026-10-16. Commit, push and PR creation are authorized. The release chat
owns target merges, release versions, tags and publication.

## Result

Implemented 21 desk models: T31G, T33G, T41P, T41S, T42U, T43U, T46G,
T46S, T46U, T48G, T48S, T48U, T53, T53W, T54W, T57W, T58W, VP59,
T73W, T85W and T87W. Added original local device graphics, explicit key maps,
a graphical editor, a list view, a key dialog and Save & Resync.
No release metadata, package dependencies or database schema changed.

The task uses its own worktree at
C:/Users/timo/.codex/worktrees/yealink-t41p-dss/talkops.
The original checkout, local commit and three Dahua summaries remain intact.
The initial base was 0f128ad. Fast-forwarded this task branch to the verified
remote main at 7d7990b before implementation. Open PR #46 is an independent
call flags change; no dependency is required.

## Changes

- presets/phones/yealink.yaml: Add ten desk models and layout metadata for
  all 21 models. Correct T33G to 12 keys, T46U to 27 and VP59 to 27.
- crates/talkops-provisioning/src/catalog.rs: Parse and validate complete
  display maps. Expose the per-model limit for the Line function.
- crates/talkops-provisioning/templates/yealink/mac.cfg: Pin the shared
  bottom-right page control for older paged models to match the editor.
- crates/talkops-api/src/routes/phones.rs: Add typed layout metadata. Keep
  unchanged legacy overrides on updates after model capacity corrections.
- crates/talkops-api/src/routes/provisioning.rs: Accept the documented
  13-digit common CFG filenames for T73W, T85W and T87W.
- web/src/lib/components/PhoneGraphic.svelte, PhoneKeyEditor.svelte and
  web/src/lib/phoneKeys.ts: Native model graphics, draft key editing,
  phonebook/number/account selection, validation and accessible dialogs.
- web/src/routes/phones/: Integrate graphics, key editing and save/resync.
- web/src/lib/api.ts and i18n/: Add metadata types and English/German text.
- Backend, web and browser tests; .github/workflows/ci.yml: Add regression
  coverage and run the new browser suite in CI.
- docs/en/yealink.md, docs/de/yealink.md and
  docs/deskphone-key-editor-proposal.md: User instructions and design sources.

## Decisions

- The user approved the expanded 21-model scope and the interactive design.
  Conference phones, DECT devices and expansion modules are outside this task.
- Keep the existing line_keys JSON storage. Copy a contact number and name;
  later contact edits do not automatically change a saved phone key.
- The picker offers global contacts and draft-assigned phonebooks. It offers
  work, mobile and other numbers separately. Internal speed dial uses **103;
  BLF keeps a bare monitoring target and the existing *8* pickup behavior.
- Physical phones have explicit page maps. T33G has buttons on the right.
  T73W/T85W retain all physical keys because their page control is separate.
- Touchscreen models use an explicitly labeled, scrollable DSS overview.
  Do not claim a pixel-exact copy of firmware-specific idle screens.
- Default removes an override. Unused writes an explicit empty key. Account
  defaults and AX Wi-Fi handset restrictions remain in place.
- Unsupported legacy overrides remain visible and can be retained unchanged
  while saving other settings. New or modified unsupported overrides fail.
  A user can remove an old override and assign its value to a supported key.
- Save & Resync saves first. A save failure sends no notification. Offline or
  failed resync is reported separately from the saved result.
- New models remain untested on hardware. Keep undocumented optional media
  capabilities disabled. No migration ID is needed.

## Checks

- Passed: cargo fmt --all --check (rerun before commit).
- Passed: cargo test -p talkops-provisioning --locked; 13 tests and doc tests.
- Passed: cargo clippy -p talkops-provisioning --all-targets --locked -- -D warnings.
- Passed: npm ci; npm run lint; npm run check (zero errors and warnings);
  npm test (28 tests); npm run build.
- Passed: node tests/browser/phone-keys.mjs against the production build.
  All 21 models, each last key, contact/mobile selection, persistence,
  internal/BLF targets, default reset, missing accounts, legacy overrides,
  save/resync failures, role restrictions, dialog bounds and light/dark
  layouts at 320, 768 and 1280 pixels.
- Passed: node tests/browser/layout.mjs; general responsive layout suite.
- CI/E2E: Follow the checks on the current head of PR 47. Initial application
  push runs: CI 38947234032, E2E 38947234000. Initial PR runs:
  CI 38947338227, E2E 38947338284. The PR records final results. Do not infer
  final readiness from an older run or this task summary.
- Not available locally: full workspace Clippy stopped because dlltool.exe
  is absent from the Windows GNU toolchain. The Docker Linux daemon is not
  running. Full PostgreSQL/LDAP/Linux stack validation must run in CI.
- Initial local provisioning tests failed on Windows checkout CRLF template
  endings. Normalized this task checkout to LF and reran successfully. Web
  lint had the same local checkout issue. No unrelated file content changed.
- Manual review: inspected actual API, catalog, renderer and editor diffs.
  CodeRabbit CLI is not installed; no CodeRabbit review result is claimed.
- STE review: Used repository clarity rules. The full official standard was
  unavailable. Full STE language review remains pending.

## Dependencies and open work

- Dependencies on other PRs: None.
- Database migration IDs: None. Package dependency changes: None.
- Upgrade: Resync phones to apply saved layouts. Review unsupported old T46U
  and VP59 overrides. Firmware behavior still needs real phone checks.
- Suggested release notes: Add provisioning and graphical function key setup
  for 21 Yealink desk phone models. Select speed dial targets from assigned
  phonebooks or internal extensions. Preserve legacy key overrides when model
  limits are corrected. Support common CFG requests from newer Yealink phones.
- Committed and pushed the implementation. PR 47 contains the handoff.
  The final task-summary revision is a separate documentation commit.

## Next action

The development chat must verify the final PR checks and update the PR
handoff before marking it ready. The release chat can then review and
integrate PR 47. Target merges and publication belong to the release chat.

## References

- docs/deskphone-key-editor-proposal.md (research sources and approved design)
- docs/ai-memory/20261012-codex-yealink-internal-phonebook.md
- docs/ai-memory/20261012-claude-history-1.7.1-1.7.5.md
- docs/ai-memory/20261012-claude-codec-profiles.md
- docs/release-workflow.md
