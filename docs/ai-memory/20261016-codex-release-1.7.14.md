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

All six ready source heads and PR 35 are merged into main. Version 1.7.14
is published. Registry verification passed; the additional tag E2E is pending. No contribution is omitted. The four promised UI/API tasks
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
  Linux CI and E2E passed on the final integration head.
- Manual integration review used source changes, API/UI contracts and tests.
  CodeRabbit CLI is unavailable. FreeSWITCH source/workflow are unchanged.
- Superseded candidate ea9de61: PR CI 38858462646 found an unused local
  `std::io::Write` import after the backup merge. Remove it. All final-head
  gates passed after the correction. Earlier runs are superseded.
- STE review: short-sentence/manual clarity review. Full official standard
  unavailable; full language review pending, separate from functional checks.

## Dependencies and open work

- Passed gates: final candidate CI/E2E; exact-main CI/E2E before tag; Release;
  GitHub release; three amd64/arm64 manifests, OCI labels and moving tags.
- Live SMB storage and Portainer were unavailable. Tests use protocol services,
  a fake Portainer API and real TLS certificate pin checks. Extra Git-stack
  files and backup failure emails are outside this contribution.
- No deployed UI is available here. CI CLI versions and all six published
  OCI version/revision labels were verified.
- VTO-to-VTH missing picture and percent-encoded CDR spool import remain open
  separate work. Preserve rejected spool files.

## Next action

Wait for the additional tag E2E. Archive the final summary and update PR 35.

## References

- `docs/release-workflow.md`, `docs/release-manager-prompt.md`, `CHANGELOG.md`
- Source summaries: `20261015-claude-call-log-filters.md`,
  `20261015-claude-roles-permissions.md`, `20261015-claude-transcription-jobs.md`,
  `20261015-claude-motion-polish.md`, `20261015-claude-backup-targets.md`,
  `20261015-claude-backup-deployment-capture.md` in this directory.

## Main merge checkpoint

- Final candidate: 057e2bea587d2c598872edc16612bb7b1680c30d.
- PR CI 38858886905 and E2E 38858886884 passed. Push CI 38858882436
  and E2E 38858882201 attempt 2 passed. All 12 final-head checks are green.
- Push E2E attempt 1 failed at the unchanged Piper duration assertion. Logs
  did not print durations. No render error was logged. The cause is unknown.
  It did not recur in the independent PR run or the full rerun. No assertion
  was changed or waived. The ignored real-whisper test was not run in CI.
- Both new integration tests passed. rclone WebDAV/FTP/SFTP/S3 transfers and
  real self-signed TLS pin tests passed. Server and worker report 1.7.14.
- Merged at 2026-10-16T06:26:15Z. Main commit:
  0f128ad210028b386081db0bdae90c210ff84523.
  The main tree equals the candidate tree. All six heads are ancestors.
- Source PRs 29-34 and integration PR 35 are merged. No open PR remains.
- Exact-main CI 38861196732 and E2E 38861196707 passed before the tag push.
- This checkpoint is local documentation only. It does not change main or
  restart its checks. The final summary will use an immutable audit reference.
## Tag checkpoint

- Exact-main CI 38861196732 and E2E 38861196707 passed on the unchanged
  main commit. The latest-main run checks passed immediately before tagging.
- Scope recheck: no open PR. All source heads are included. Versions match.
- Annotated tag v1.7.14 was pushed once. Tag object:
  dab752803d18c14ac54335f7df3c2209aede3bf8.
  Peeled target: 0f128ad210028b386081db0bdae90c210ff84523.
- Release workflow and artifact verification passed. No published tag was moved.

## Publication verification

- Release 38862917854 passed all six native builds, three manifest jobs and
  GitHub release creation. Additional tag CI 38862918000 and FreeSWITCH
  38862917817 passed. Tag E2E 38862917820 is still running.
- GitHub release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.14
- Published at 2026-10-16T06:51:21Z. It is the latest stable release, not a
  draft or prerelease. English release notes list all six source contributions.
- Registry inspection passed for talkops-server, talkops-media-worker and
  talkops-freeswitch. Each versioned manifest contains linux/amd64 and
  linux/arm64. All six OCI configurations report version v1.7.14 and revision
  0f128ad210028b386081db0bdae90c210ff84523. For every image, tags 1.7 and
  latest have the same platform digests as 1.7.14.

| Image | Architecture | Published platform digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:6dcfbbf0e5c5918326ce9168ef7fd29f7030b6fbfab5cd0b70b290989485ce47 |
| talkops-server | arm64 | sha256:55c26aafe7330612a605df207ba20763f8ad2b6a9c238be21a1446566dd0296e |
| talkops-media-worker | amd64 | sha256:91ee4b42fc426d3d9737b6a478f0bfdbb2fe1e8bcb66e39515da7f5d724ff4d8 |
| talkops-media-worker | arm64 | sha256:d0f841f2bc8deb87fcce9993e11dd2a812c4dc6993d3af23ce9be48ecc162393 |
| talkops-freeswitch | amd64 | sha256:66c38d105cb53e5af68cc0d3d068e20225c3c27e72f57494e193b371d54621a5 |
| talkops-freeswitch | arm64 | sha256:687f71e9d96a04e49d9a677ecaafdec674eb3204d25595d952bcdce5efefa5be |
## Branch cleanup

- Checked all six remote source heads and the integration head unchanged.
  Confirmed ancestry in the release commit. Deleted the seven integrated
  remote branches in one atomic push with expected-head guards. No history
  was rewritten. GitHub now has only main, and no open PR remains.
- Foreign local branches, worktrees and unpublished primary documentation
  remain. Only this task's local release branch can be removed after archiving.