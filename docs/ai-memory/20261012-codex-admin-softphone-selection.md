# Admin softphone selection and first feature scope

## Task state

- Task ID: 20261012-codex-admin-softphone-selection
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: Completed implementation. PR readiness follows its current checks.
- Branch: codex/admin-softphone-selection
- Base commit: 2fbe3195517b509d2afd3d57247c05a98437f064
- Source commit: ea13328c38c63f82dfb180e37bbae7572b70407a
- PR: https://github.com/itsh-neumeier/talkops/pull/3

## Goal

Let administrators select a softphone identity without a fixed extension
assignment. Let users select only their assigned extensions. Add the first
recommended feature scope after user review.

## Result

The user approved implementation after the proposal. The first scope includes
identity selection, separate browser accounts, audio devices and tests,
ringtone, optional notifications, contacts, local favorites, recent calls,
callback and a connection that survives navigation within TalkOps.
Transfer and multiple active calls remain a separate proposed second scope.

## Changes

- `crates/talkops-api/src/routes/webrtc.rs`: Shared selection rules, safe account
  access, available identities and recent calls for the selected extension.
- `crates/talkops-core/src/extensions.rs`: Browser accounts per user and
  extension, transaction locks, audit entries and current SIP permission checks.
- `crates/talkops-api/src/routes/extensions.rs`: Protect browser credentials
  and password resets from another extension user's access.
- `migrations/20261026000002_browser_account_owners.sql`: Add browser ownership
  and preserve an existing owner's first browser account.
- `web/src/lib/components/Softphone.svelte`: Persistent softphone and controls.
- `web/src/lib/components/SoftphoneAudio.svelte`: Device selection and tests.
- `web/src/lib/components/DeviceList.svelte`: Hide other users' browser secrets.
- `web/src/routes/+layout.svelte`: Keep the component alive during navigation.
- `web/src/lib/softphone.ts`: Dial targets, callback direction and local tones.
- API types and both language catalogs: New fields and translated UI strings.
- HTTP, Node and browser tests: Permissions, migration, concurrency, caller
  numbers, audio playback, incoming notifications, navigation and reconnection.
- English and German softphone guides and ADR 0020: Behavior and compatibility.

## Decisions

- Target `ccr-df1c8083-0ce1he` was verified from remote HEAD and fetched.
  Preserve the original checkout and other worktrees.
- Admins select any enabled extension in their tenant. Other roles can select
  only their enabled assignments. Admins must select explicitly.
- Selection includes incoming calls and extension settings. It does not change
  extension ownership or provide an arbitrary caller number override.
- Use a separate browser SIP identity per user and extension. Do not store SIP
  secrets in browser storage. Credential responses use `Cache-Control: no-store`.
- Block identity changes during calls. Unregister the previous identity first.
  Logout and tab closure stop the connection. Browser suspension can stop calls.
- Use blob audio URLs within the existing CSP. Do not relax the CSP.
- Store audio preferences and contact favorites locally per user.
- PR #2 already uses migration ID `20261026000001`. This task uses
  `20261026000002` to avoid that conflict. The migration does not depend on PR #2.
- The initial review requirement is satisfied by the user's approval. Commit,
  push and PR creation are authorized. The release chat owns merges and releases.

## Checks

- Passed: Remote and working-tree inspection. Initial checkout was clean.
- Passed: `npm ci`, web type check, Node tests and an initial production build.
- Passed: Web lint after local CRLF normalization. Unchanged source content is
  not part of the patch. Final lint, type check, Node tests and build passed.
- Passed: Browser fixture scenarios for admin, one/multiple user extensions and
  an operator with no extension. Includes successful blob audio playback,
  incoming ringtone, notifications, favorites and automatic SIP reconnection.
- Passed: Visual inspection. Corrected the audio CSP failure found in the first
  screenshot. Final visual inspection passed.
- Passed: MSVC `cargo check -p talkops-core -p talkops-api --all-targets --locked`
  before the final browser-owner field and access-control changes.
- Passed: MSVC `cargo test -p talkops-api --lib routes::webrtc::tests --locked`:
  3 tests. Other non-database API library tests also passed in an earlier run.
- Failed local environment checks: GNU build lacked gcc. Workspace MSVC tests
  include Unix-only worker tests. The older MSVC Clippy (Rust 1.94.1) reports
  an existing `identity.rs` warning. A retry exhausted local build disk space;
  `cargo clean` removed only this worktree's generated target directory.
- Initial remote results: Formatting, full Clippy, web checks, Compose and image
  smoke tests passed. Six of seven WebRTC integration tests passed. The caller
  number test expected E.164 instead of the provider format. The real browser
  call kept live audio after navigation, but its hangup selector targeted the
  hidden full view. Corrected both test assumptions in commit `5371a68`.
- Passed remotely on `5371a68`: Full workspace formatting, Clippy and tests;
  all seven WebRTC integration tests; web lint, check, Node tests and build;
  Compose validation; both image builds and smoke tests; SIPp and browser E2E.
  The live browser call kept audio after navigation and used the compact
  hangup control. CI run `38256722301` and E2E run `38256718255` succeeded.
- Final UI text: Clarified the incoming missed-call filter and let its heading
  wrap on narrow screens. Local lint, type check and Node tests passed after
  this change. The PR checks provide validation for the final revision.
- Not run locally: Full database and FreeSWITCH tests. PostgreSQL, Linux Docker
  and SIPp are not available. Linux CI and E2E are required before PR readiness.
- Review: Manual diff review found and fixed shared credential access through
  device endpoints. CodeRabbit CLI was not installed; no CodeRabbit run occurred.
- STE review: Clear English review only. Full official standard unavailable;
  full language review is pending.

## Dependencies and open work

- Migration ID: `20261026000002`.
- No required dependency on another PR. PR #2 also changes the API types and
  language catalogs; the release chat must preserve both sets of additions.
- Existing owner credentials stay unchanged. An old admin API client must send
  `extension_id`; regular users retain the empty-request default.
- Worktree: `C:/Users/timo/.codex/worktrees/admin-softphone-selection/talkops`.
  App attachment registration failed, but Git recognizes the separate worktree.
- Proposed release notes: Administrators can select a softphone extension
  without a fixed assignment. Users can select their assigned extensions.
  The browser softphone stays connected during TalkOps navigation and adds
  audio device tests, ringtone, notifications, contacts, favorites and callback.

## Next action

The release chat must review and integrate PR #3 after its final checks pass.
No version, tag, target merge or publication was created in this task.

## References

- `docs/release-workflow.md`
- `docs/adr/0020-softphone-identities-and-lifecycle.md`
- `docs/ai-memory/20261012-codex-shared-ai-workflow.md`
- https://github.com/itsh-neumeier/talkops/pull/2
