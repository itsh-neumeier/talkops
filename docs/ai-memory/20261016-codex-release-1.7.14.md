# TalkOps 1.7.14 release

## Task state

- Task ID: 20261016-codex-release-1.7.14
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.7.14
- Base commit: 702507b39fb1cf80db4ce5a0f9dd3d339b9da68c
- Related commits or PR: Integration corrections 1bad7389a7f97f1dc477c840e200d0cc3e9d6692; [integration PR 35](https://github.com/itsh-neumeier/talkops/pull/35)

## Goal

Integrate all open contributions and publish the requested version 1.7.14.
Verify the final main commit before the tag push. Verify release artifacts.
Keep foreign local work. Remove only unchanged, integrated remote branches.

## Result

All six ready source heads are integrated with merge commits. Publication
has not started. No contribution is omitted. The four promised UI/API tasks
and both backup tasks are present. No separate Codex feature PR is assigned.

| PR | Contributor | Exact head | Dependencies | Migration |
| --- | --- | --- | --- | --- |
| [29](https://github.com/itsh-neumeier/talkops/pull/29) | Claude Code | 8bdea73278144a8ad877eb9f2b1b61509afa3f12 | None | None |
| [30](https://github.com/itsh-neumeier/talkops/pull/30) | Claude Code | 4034e8ad2d436409f16e4a5c917f6832a096b72f | Integrate call-log changes from 29 | 20261101000001 |
| [31](https://github.com/itsh-neumeier/talkops/pull/31) | Claude Code | 972246c4ac9ab968269d18d0be7b41b5530d13be | 29, 30 | 20261101000002 |
| [32](https://github.com/itsh-neumeier/talkops/pull/32) | Claude Code | 2da5a6a881b4b3e8aeb7e738c55af76b179cae89 | 31 and its stack | None |
| [33](https://github.com/itsh-neumeier/talkops/pull/33) | Claude Code | 88840f013ce11dac9b1522836aa88c3b2c231e79 | Joint backup release with 34 | 20261102000001 |
| [34](https://github.com/itsh-neumeier/talkops/pull/34) | Claude Code | ef553a60c2de2fcb0b457c504afa8a79dbf577c0 | Joint backup release with 33 | 20261102000002 |

## Changes

- Merge source histories in order 29 through 34. Preserve the final 1.7.13
  audit history from `audit/release-1.7.13` too.
- Keep all seven browser checks, all four Compose checks and ADRs 0029-0032.
- Resolve call-log conflicts with the new permission checks. Keep separate
  diagnostics and backup UI permissions. Combine backup/Portainer translations.
- Apply `backups.manage` to new backup-target and Portainer endpoints.
- Run Portainer refresh in `backup::create_in` so target staging also refreshes.
- Add new BackupConfig fields to recovery and Portainer test fixtures.
- Add tests for delegated backup permission, same-session grant/revoke, and
  fresh Portainer files in a real encrypted staging archive.
- Update Cargo and web root versions to 1.7.14 and add English release notes.

## Decisions

- The current user instruction sets 1.7.14. It supersedes source proposals for
  a later minor release. Existing access defaults remain. API additions are
  compatible. New transcription rights require an explicit non-admin grant.
- Server and worker must run the same version. Back up before four migrations.
- rclone v1.75.2 is an intentional checksum-pinned server runtime addition.
  Cargo and npm dependency versions do not change.
- Preserve unpublished primary-checkout documentation and its three untracked
  Dahua summaries. Do not import foreign worktree changes or use foreign chats.

## Checks

- Passed: all six exact source heads had green CI/E2E at inventory. Source
  workflow runs: 29 (38793840872, 38793840946, 38793847886, 38793847923);
  30 (38793847857, 38793847862, 38793855474, 38793855523);
  31 (38793852865, 38793852936, 38793860992, 38793861080);
  32 (38793857745, 38793857774, 38793867608, 38793867642);
  33 (38820253620, 38820253689, 38820258696, 38820258706);
  34 (38819769959, 38819770095, 38819776597, 38819776601).
- Passed locally: cargo fmt; npm ci, lint, Svelte check, 14 web tests, build.
- Passed: lockfiles change only workspace/root version metadata; 41 unique
  ordered migrations; existing 37 migrations unchanged; 32 unique ADR IDs;
  1,673 matching EN/DE keys and placeholders, no duplicate keys; all six source
  heads are ancestors; all seven browser suites remain in CI.
- Not run locally: full Rust/DB/LDAP tests (Windows GNU lacks dlltool.exe),
  Linux image/Compose/SIPp/browser stack (Docker Linux daemon unavailable).
  Required Linux CI and E2E are pending for the final integration head.
- Manual integration review used source changes, API/UI contracts and tests.
  CodeRabbit CLI is unavailable. FreeSWITCH source/workflow are unchanged.
- Superseded candidate ea9de61: PR CI 38858462646 found a duplicate local
  `std::io::Write` import after the backup merge. Keep the shared module import
  and remove the duplicate. Web browser and Compose checks passed on that head.
  Repeat all final-head gates after this correction.
- STE review: short-sentence/manual clarity review. Full official standard
  unavailable; full language review pending, separate from functional checks.

## Dependencies and open work

- Required gates: final candidate CI/E2E; merged exact-main CI/E2E; unused tag;
  Release workflow; GH release; three amd64/arm64 manifests and moving tags.
- Live SMB storage and Portainer were unavailable. Tests use protocol services,
  a fake Portainer API and real TLS certificate pin checks. Extra Git-stack
  files and backup failure emails are outside this contribution.
- No deployed UI is available here. Check CLI versions in image CI and OCI
  version/revision labels after publication.
- VTO-to-VTH missing picture and percent-encoded CDR spool import remain open
  separate work. Preserve rejected spool files.

## Next action

Create the integration PR. Pass all checks before merge. Verify exact main,
then push the new tag once. Verify artifacts before reporting success.

## References

- `docs/release-workflow.md`, `docs/release-manager-prompt.md`, `CHANGELOG.md`
- Source summaries: `20261015-claude-call-log-filters.md`,
  `20261015-claude-roles-permissions.md`, `20261015-claude-transcription-jobs.md`,
  `20261015-claude-motion-polish.md`, `20261015-claude-backup-targets.md`,
  `20261015-claude-backup-deployment-capture.md` in this directory.
