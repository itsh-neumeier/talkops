# TalkOps 1.7.7 release

## Task state

- Task ID: 20261012-codex-release-1.7.7
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: In progress. Combined candidate prepared; CI and publication pending.
- Branch: codex/release-1.7.7
- Base commit: 4503fb3b96167aa77320cc936597fc6147b392c3
- Integration PR: Pending

## Goal

Publish all complete contributions as 1.7.7. Clean up integrated branches after
verification. The user selected publication of ready PRs while preserving draft
branches. The user then asked to include PR #8 after another readiness check.

## Result

Merged ready PRs #3 through #8 with their source histories. No feature branch
was rewritten. Preserved the final documentation audit from release 1.7.6.
Prepared version 1.7.7 and English notes. No target merge or publication yet.

## Sources

| PR | Contributor / change | Verified head |
| --- | --- | --- |
| #3 | Codex: persistent softphone and identity selection | `ec0560b69eb78d5f781a748c9ac7149443156a74` |
| #4 | Codex: encrypted backup and guided recovery | `1b02ed7071b214e6e9f6c916cbe1d396474716be` |
| #5 | Codex: explicit internal phonebook dialing | `a35169dc6f3ba31010756dbdbb85b3d316dca9f1` |
| #6 | Claude Code: task memory backfill | `6ce43b6a323c4ce40634455aac23d1994f5edb17` |
| #7 | Codex: active call identities and local live previews | `8b9081149bd651dbf05dfda786ff0ad2e3dbc249` |
| #8 | Claude Code: persistent CDR spool and importer | `d2462b5cd12d51e4ce5ea9c7448984d5493aa6a4` |

PR #8's summary says implementation completed. Its only stated draft condition
was CI success. All exact-head CI/E2E/FreeSWITCH checks passed. The release
manager verified this condition and the handoff. PR #9 is unfinished and excluded
by the user's scope decision. Its branch and work remain preserved.

## Changes and decisions

- Keep both recovery and active-call browser suites when resolving the CI merge.
- Keep ADRs 0020, 0021 and 0022. No released migration was changed.
- PR #4 adds age, fs2 and tempfile. No extra dependency upgrade is intended.
- PR #8 adds a shared CDR spool. Integration adds it to backup storage,
  archive validation and restore. CLI/server use the same configured spool path.
- Extend the encrypted backup round trip to preserve pending and rejected
  records and remove post-backup spool data. Test CLI storage path selection.
- Use the explicitly requested 1.7.7 version. The pickup-code and admin API
  compatibility changes are stated in the changelog and upgrade notes.
- Cleanup applies only to integrated remote branches. Preserve draft and any
  newly advanced branch. Keep foreign worktrees and uncommitted files intact.
- The remaining primary branch will use the requested `main` name after release
  verification. GitHub branch rename must preserve pending PR targets.

## Checks

- Passed: Source CI/E2E on all six recorded heads. PR #8 also passed FreeSWITCH.
- Passed: Rust format, whitespace, merge ancestry and unique migration IDs.
- Passed: 31 migrations; new IDs `20261026000002` and `20261026000003` follow
  the released `20261026000001`. Released migrations remain unchanged.
- Passed: DE/EN catalogs have the same 1,129 unique keys.
- Local web validation: In progress. Initial lint failed on Windows CRLF in
  unchanged files. LF normalization corrected lint without Git content changes.
- Full combined Rust/database, images, Compose and E2E validation uses CI.
- Review: Manual integration review. CodeRabbit CLI is unavailable.
- STE review: Repository writing rules used. Full official standard unavailable;
  full language review is pending.

## Dependencies and open work

- Resync provisioned phones. `**<number>` makes an internal call; pickup is
  `*8*<extension>`. Update manual pickup/BLF keys and saved pickup speed dials.
- Admin clients must send extension_id to POST /api/v1/me/webrtc.
- Deploy all three images and updated Compose together for the CDR volume.
- Copy undelivered CDRs out of old FreeSWITCH containers before replacement.
- Complete recovery archives need a separate password and deployment capture.
- Local previews need the base model and add worker CPU/memory load.
- PR #8's diagnostic feedback about unavailable ESL and unresolved overview
  rows remains follow-up work. The production cause of lost CDRs is unconfirmed.
- No background automation was created.

## Next action

Open the integration PR. Wait for all relevant final candidate checks, merge,
and verify the resulting target's CI/E2E/FreeSWITCH checks before tagging.
Publish v1.7.7 and verify all three amd64/arm64 manifests and moving tags.
Then perform safe branch cleanup and save the final result in the PR.

## References

- [Release workflow](../release-workflow.md)
- [Release 1.7.6 summary](20261012-codex-release-1.7.6.md)
- Source PRs: https://github.com/itsh-neumeier/talkops/pulls
