# All-vendor provisioning and phone copy proposal

## Task state

- Task ID: 20261017-codex-provisioning-all-vendors-proposal
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: In progress. User approved implementation on 2026-10-17.
- Branch: codex/provisioning-all-vendors-proposal
- Base commit: c4765b0495adf004f0b2b0f5099cbd098f384f86
- Related PR: https://github.com/itsh-neumeier/talkops/pull/51 (draft)
- Proposal commit: b2500f7

## Goal

Extend provisioning to all usable FusionPBX vendor templates. Add identical-phone
configuration copy with separate device identities. Present the design for user
review and feedback before runtime changes.

## Result

Prepared and published a proposal and pinned upstream inventory in draft PR 51.
The user approved it with "nein passt so mach das". Implementation has started.
Phone copy API, UI and database integration tests are being added locally.
The current code and final PR checks determine completed behavior.

Local implementation checkpoint:

- Imported all 1,037 original blobs without newline conversion. All old Yealink
  blob hashes remain unchanged. Generated native derivatives have MPL notices.
- Added a bounded MiniJinja renderer, exact artifact manifests, model-scoped
  authenticated paths, catalog records and template revision integration.
- Generated 386 source profiles. 385 contain usable configuration. Grandstream
  Wave contains only one newline. Spectralink contains only an empty placeholder.
  Both gaps are explicit in the source library and have no selectable device.
- Added parameter schemas from native configuration fixtures. Scalar fields have
  categories, source comments, file paths and copy/secret/managed metadata.
  XML value replacement preserves structure. The UI has search, category groups,
  lazy expansion, field paging and a filter for device overrides.
- Added source key groups and semantic line/BLF/speed-dial assignments. Button
  capacities without explicit model evidence are not claimed as verified.
- Added global and per-device language, IANA zone and NTP fields. Model-specific
  locale mapping is still under review. Unknown IANA offsets no longer use CET.
- Added atomic same-model copy with a source revision check, new MAC/app ID,
  separate SIP credentials, explicit extension assignments, permission checks,
  source-specific exclusions, transaction audit and deferred unassigned keys.
- Added app IDs, audited file/QR export and per-device configuration file lists.
- Manufacturer secrets are encrypted in JSON and redacted from phone responses.
  Copy preview excludes secret and device-specific fields.

## Changes

- `docs/provisioning-all-vendors-proposal.md`: Current gaps, complete vendor scope,
  adapter and routing design, copy rules, sequence, acceptance and test plan.
- `docs/research/fusionpbx-all-vendors-inventory.json`: Metadata from the complete
  upstream Git tree at 20143aec4366afc2755f336d3058243479d49d44.
- This file: Review boundary and continuation context.

## Decisions

- The user reviewed this proposal and explicitly approved its implementation.
  No further design approval is needed for ordinary technical decisions.
- Remote default HEAD and fetched origin/main both identify the base commit.
  PR 49 and integration PR 50 are merged. No open PR existed at task start.
- Use a new worktree at
  `C:/Users/timo/.codex/worktrees/provisioning-all-vendors-proposal/talkops`.
  Preserve the older provisioning worktree and the primary checkout's three
  untracked Dahua summaries. Do not import foreign uncommitted changes.
- Recommend same-model snapshot copy, a server allowlist, separate SIP devices,
  explicit extension selection and no automatic resync.
- All 31 source directories remain in scope. Spectralink has only empty.txt.
  Source import alone does not satisfy full vendor provisioning support.

## Checks

- Passed: Remote HEAD verification, fetched target and existing work inspection.
- Passed: Current adapter, catalog, routes, storage, UI, ADR and prior PR review.
- Passed: GitHub tree retrieval; response is not truncated (31 directories,
  1,037 files). Source directory presence is not a capability claim.
- Passed: Independent pinned subtree cross-check for all 31 vendor rows,
  1,037 files, directory lists and filename patterns. Spectralink empty.txt
  is the only file in that directory and has zero bytes.
- Passed: All local Markdown links in the proposal and summary resolve.
- Passed: `git diff --cached --check` after staging the three task files.
- Passed at local implementation checkpoints: provisioning unit tests (15), all
  385 usable source profiles with empty/single/full accounts, XML validation,
  model parameter schema generation, source key group fixtures and path tests.
- Passed: `node tests/provisioning/reference.mjs` verifies both the old 433 blobs
  and all 1,037 original blobs. Native regeneration `--check` also passed.
- Passed: web unit tests (35). An earlier web check passed with no warnings.
- Pending: rerun web check after the latest native key component type fix.
- Earlier API tests exposed a blank CFG scalar parser panic. It was fixed.
  Three clone tests then passed. A stale-source test changed an already-empty
  value; the fixture now changes the language. Its rerun remains pending.
- The app identity, authentication and export integration test passed. The
  parameter test exposed editable XML `group` metadata. This metadata is now
  excluded. Rerun the parameter and all API tests after this correction.
- Final formatting, clippy, full relevant Rust tests, browser tests and Linux
  CI/E2E remain pending. Do not use proposal-only CI as implementation evidence.
- A stronger, later check now verifies each SIP username and its distinct
  password in the complete output. It exposed about 90 profile failures.
  Causes include old `user.N` and flat `_N` aliases, indexed `lines` access,
  wrong source password bindings, generic model conditions and account bounds.
  Do not describe the current implementation as ready. Earlier successful
  render checks established syntax and XML structure, not registration.
- Boolean `filter_var` and string/number comparisons also needed corrections.
  The converter now handles boolean validation separately from IP validation.
  These latest semantic corrections need a full rerun.
- Latest web check passed without errors or warnings. Web tests (35), lint and
  production build passed before the latest account-count and route updates.
- STE review: Applied repository clarity rules. Full official standard was
  unavailable; full language review remains pending.

## Dependencies and open work

- Required PR dependencies: None. PR 49 is already in the base.
- Migration IDs in this PR: None. Existing schema baseline: 20261105000001.
- Runtime additions: `md-5` 0.10.6, `roxmltree` 0.21.1, and the existing chrono /
  chrono-tz versions in the provisioning crate. MiniJinja enables loop controls.
  No unrelated dependency versions or release metadata were changed.
- App worktree registration was unavailable for this projectless parent path.
  A separate ordinary Git worktree and feature branch were created successfully.
- Remaining implementation checks: source URL/firmware/phonebook references,
  app export requirements, model-specific locale formats, manufacturer field
  coverage, key semantics/reset, source account capacity evidence, structured
  field default/value validation, and compatibility of existing Yealink IDs.
- Added configurable account counts for source profiles without hardware
  evidence. Bound these by the source limit when one exists. Wire the count
  through assignment, clone, renderer, parameter schemas and UI. Test these
  paths before release. Parameter schema caching now has a memory limit.
- Original blobs remain complete. Some source files contain fixed sample SIP
  credentials. Replace these in native derivatives, as started for ATCOM AG198.
  Never deliver sample accounts as real provisioning defaults.
- FusionPBX `app/devices/app_config.php` contains the vendor function catalog.
  Use this pinned data to complete native key mappings. Its data must be parsed
  without PHP execution. The provisioning class also has legacy code mappings.
- The app config source and protocol helper source are not imported yet.
- Source-only checkpoint commits do not contain all local implementation work.
  The implementation remains local until its feature commits are pushed.
- Real hardware and firmware validation is unavailable locally. Source coverage
  does not prove complete hardware capability coverage. Keep this distinction
  visible in the UI, documentation and PR.

## Next action

Complete the approved design, update the summary and PR, and run the planned
checks. Local PostgreSQL tests use an isolated cluster at port 55439 in
`C:/Users/timo/.codex/tmp/talkops-all-vendors-tests/pgdata`.
The Windows GNU check could not find dlltool. The installed MSVC toolchain is
being used for local checks with output under `A:/aislop/codex/talkops/all-vendor-tests`.
Commands use `cargo +stable-x86_64-pc-windows-msvc`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, and `DATABASE_URL=postgres://postgres@127.0.0.1:55439/postgres`.
Logs are in `C:/Users/timo/.codex/tmp/talkops-all-vendors-tests`.
The sparse reference checkout is next to the task checkout at `fusionpbx-source`.
Keep the PR a draft while required implementation or checks remain open.

## References

- docs/provisioning-all-vendors-proposal.md
- docs/ai-memory/20261016-codex-provisioning-multivendor-plan.md
- docs/ai-memory/20261017-codex-release-1.8.2.md
- docs/adr/0038-vendor-provisioning-profiles.md
- docs/release-workflow.md
