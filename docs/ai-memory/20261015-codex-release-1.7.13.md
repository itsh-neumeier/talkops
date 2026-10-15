# TalkOps 1.7.13 release

## Task state

- Task ID: 20261015-codex-release-1.7.13
- Date: 2026-10-15, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: `codex/release-1.7.13`
- Base commit: `6276e7d5e1b3bbe2a97e0fe237758b3f3a0e44b4` (remote main, 1.7.12)
- Integration PR: [#28](https://github.com/itsh-neumeier/talkops/pull/28)

## Goal

Complete and integrate all open PRs for 1.7.13. Check the final candidate and
resulting main commit. Publish and verify the release. Keep only `main` on
GitHub after safe cleanup of unchanged, fully integrated source branches.

## Result

Both open source PRs are ready and integrated with merge commits. Neither
source head was already in the base. No contribution is omitted or pending.
No separate Codex feature PR is assigned. Publication has not started.

| Source | Contributor | Exact head | Dependencies | Migration IDs |
| --- | --- | --- | --- | --- |
| [#26](https://github.com/itsh-neumeier/talkops/pull/26), shared phonebook numbers | Claude Code | `83574493d75eb23945658b88218ca4367489f77a` | None | None |
| [#27](https://github.com/itsh-neumeier/talkops/pull/27), device media and door ringing | Claude Code | `652a9e7f3f7475e2c218302545f6722820e494a9` | None | `20261031000001` |

PR #27's body names the older head `088d744`. Its final commit only links
the PR in its summary. The selected actual head has all green checks.

## Changes

- PR #26: choose a contact name for a shared number, prefer the local
  extension's phonebooks, show all matches, and link to highlighted entries.
  Add API fields, EN/DE text, browser tests, user docs and ADR 0027.
- PR #27: device video modes, G.711 audio, caller codec inheritance,
  parallel door ringing, diagnostics RTP counters, migration, guides and tests.
- Integration: keep the phonebook ADR as 0027. Rename the concurrent media
  ADR to 0028. Update its code/doc links and the unreleased migration comment.
  Keep the original contributor summaries as source-history records.
- `crates/talkops-api/src/active_calls.rs`: add `video: None` to the new
  phonebook test device, to match the Device field added by PR #27.
- Release metadata: set Cargo workspace and web root versions to 1.7.13.
  Combine both contributions and upgrade notes in `CHANGELOG.md`.
- Merge `audit/release-1.7.12` to preserve the final previous release report.
  The owned checkout had a previous report copy identical to that remote
  archive. Save a temporary copy and clear only that published local change
  before switching branches. Import no foreign uncommitted work.

## Decisions

- Use the requested patch version 1.7.13. API fields are additive. Existing
  device defaults, SIP names and passwords stay. New settings are optional.
- The `ambiguous` API field stays. A selected phonebook name is not proof of
  identity. The matches card shows the other possible entries.
- Migration `20261031000001_device_video_and_g711` follows the existing
  migration IDs. No released migration changes. The PostgreSQL enum addition
  is not used as a new value in the same migration transaction.
- Door calls lose SIP early-media previews. On video calls, callers without
  a fixed audio profile inherit both codecs from the answering device.
- Do not claim a hardware fix for the VTO-to-VTH missing picture. The source
  handoff explicitly leaves its cause open and supplies settings and counters.
- Preserve the primary checkout's unpublished commit and three untracked
  Dahua summaries, all foreign local branches, and foreign worktrees.
- Target `main` is verified. Merge commits are permitted. No branch protection
  or ruleset is active. Do not force push or move existing release tags.

## Checks

- Passed: #26 PR CI 38735445206 and E2E 38735444110;
  push CI 38735404379 and E2E 38735404472, on the selected source head.
- Passed: #27 PR CI 38740919360 and E2E 38740918846;
  push CI 38740911428 and E2E 38740911576, on the selected source head.
- Passed locally: `cargo fmt --all --check`; `npm ci`, web lint, check
  (0 errors, 0 warnings), tests (6 passed), and production build.
- Passed: lock-file comparison finds no external dependency changes;
  DE/EN have the same 1,354 keys; all 37 migration IDs and 28 ADR numbers
  are unique; released migration contents are unchanged; ADR links resolve.
- Not run locally: Rust compilation and full DB/LDAP tests. The Windows GNU
  toolchain lacks `dlltool.exe`. Linux CI must run the complete workspace.
- Not run locally: Compose, container execution, SIPp and browser E2E. The
  Linux Docker daemon is unavailable. Use Linux CI and E2E as the evidence.
- FreeSWITCH source and workflow files are unchanged. E2E builds the image.
- Final candidate, main and publication checks: pending.
- STE review: manual review with `docs/documentation-style.md`. The official
  full standard is unavailable; full language review remains pending.
- CodeRabbit CLI is unavailable. Focused integration review used the actual
  diffs, API/UI contracts, migrations, ADRs, translations and source tests.

## Dependencies and open work

- Only the stated migration is new. No source dependency or package upgrade.
- The VTO-to-VTH picture cause remains open. Hardware tests must compare the
  main-VTO configuration, video modes, parallel ringing and RTP counters.
- A deployed UI version check is unavailable. Check source versions and CI
  server/worker version output before reporting release success.
- The percent-encoded FreeSWITCH CDR spool import defect remains separate
  work. Preserve `cdr-spool/rejected/` files for later recovery.

## Next action

Wait for final-head CI/E2E on PR #28. Merge it, verify the resulting main
commit, and wait for main CI/E2E
before the tag push. Verify all release images and moving tags after publication.

## References

- `docs/ai-memory/20261015-claude-shared-phonebook-numbers.md`
- `docs/ai-memory/20261015-claude-door-video-device-media.md`
- `docs/ai-memory/20261014-codex-release-1.7.12.md`
- `docs/adr/0027-shared-phonebook-numbers-in-active-calls.md`
- `docs/adr/0028-device-media-settings.md`
- `docs/release-workflow.md`
