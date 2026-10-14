# TalkOps 1.7.10 release

## Task state

- Task ID: 20261014-codex-release-1.7.10
- Date: 2026-10-14, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.7.10
- Base: bfc46429f0889ceec79e72f45b49c6ec79dc9f26 (remote main, 1.7.9)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/20

## Goal and scope

Publish all open PRs as 1.7.10. The only source PR is ready #19 from Claude Code:
Dahua VTO/VTH setup, per-device SIP password formats and door station vendors.
Exact source head: 8ad591945ef17bea48f5edf3fe27a85fe86c0434.
Branch: claude/busy-fermat-sy3r41. No Codex feature PR or planned contribution
is missing. No dependency outside this batch. The user selected patch 1.7.10;
API changes are additive and defaults remain compatible.

## Result and changes

Preserve the source history and the completed 1.7.9 audit through merge commits.
The source merge was conflict free. Codex adds one necessary validation:
check the resulting password format when a device changes its kind to Browser.
Test both an omitted and explicit compact format; reject both without changing
the stored device or password. Keep the source rule for existing browser devices.

Set Cargo workspace and web root versions to 1.7.10 through Cargo/npm tools.
Dependency locks stay unchanged apart from workspace/root versions.
Add combined English release and upgrade notes. No production deployment.

## Checks

- Passed: exact-head source PR CI 38472348133 and E2E 38472348145.
  Push CI 38472343115 and E2E 38472343116 also passed.
  The handoff says 692f9b0; actual head only adds the task-summary PR link.
- Passed: 36 unique migration IDs; new 20261030000001_device_sip_password_format
  and 20261030000002_door_station_vendor follow the released 20261029000001.
  Released SQL was not edited.
- Passed: 1,338 matching DE/EN keys and unchanged external dependency locks.
- Passed: local Rust format and whitespace after owned-checkout LF normalization.
- Passed: local npm ci/lint/check (zero errors/warnings), six Node tests and build.
- Passed: final candidate b1582421e5a451afce0602cd610f40c9beaad991 CI
  (38520138029, 38520132966) and E2E (38520138011, 38520132928).
- Pending: resulting main CI/E2E,
  Release workflow, three amd64/arm64 manifests, labels and moving 1.7/latest tags.
- Full Linux DB/LDAP/stack checks use CI: the local Docker daemon is unavailable.
  New source tests cover passwords, roles, encryption, audit, FS directory and
  vendor defaults. Source UI checks cover desktop and 390 px.
- Real Dahua VTO/VTH hardware and deployed UI observation are unavailable.
  The 15-character workaround for VTH5422H V4.800.0.2 is not confirmed.
- Manual integration review. CodeRabbit CLI unavailable.
- Verified the cited primary Yeastar guide: it specifies 8-15 characters and
  the room field mapping. It does not identify the tested VTH model/firmware.
- Full official STE standard unavailable; full language review remains pending.

## Dependencies and open work

Existing passwords stay unchanged. Editing a format applies it to the next
password reset only. New door devices preselect compact15 in the web UI;
clients without the field keep standard. Update device credentials after a reset.
Existing stations get Dahua through the migration. See ADR 0026 and the EN/DE guide.

The known raw percent-encoded CDR spool import defect remains separate work.
No assigned PR fixes it. Preserve cdr-spool/rejected; do not claim recovery.
The primary main checkout has unpublished docs commit 397fc83 and three untracked
Dahua memory files. They have no source PR; preserve them and all foreign worktrees.
No development-chat message or automation was requested or created.

## Next action

PR #20 merged as 3cad0180d34fedaa554339afaa67a9e56fed0ab4. The merged tree
equals the candidate; source history is included. #19 is merged, no open PR
remains. Target CI 38521860605 and E2E 38521860655 are queued.
Verify resulting main CI/E2E before pushing unused v1.7.10
once. Verify publication and all registry artifacts. Delete only fully integrated
unchanged remote branches and the owned release branch. Save the final audit.

## References

- [Source PR](https://github.com/itsh-neumeier/talkops/pull/19)
- [Source summary](20261013-claude-dahua-vth-setup.md)
- [Previous release](20261013-codex-release-1.7.9.md)
- [Vendor decision](../adr/0026-door-station-vendors.md)
- [Dahua guide](../en/door-station-dahua.md)
