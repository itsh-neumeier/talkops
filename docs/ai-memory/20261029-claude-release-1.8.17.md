# TalkOps 1.8.17 release

## Task state

- Task ID: 20261029-claude-release-1.8.17
- Date: 2026-10-29
- Tool: Claude Code (release manager)
- Status: Completed
- Branch: codex/release-1.8.17
- Base commit: 9e9adbf (main, v1.8.16)
- Related commits or PR: PR98, integration PR pending

## Goal

Publish PR98 (Cisco Enterprise resync, contact status, ringtones, directory
and UI fixes) as stable TalkOps 1.8.17. The user requested the release.

## Result

PR98 is merged into the release branch with its history. Release metadata is
updated. The 1.8.16 summary is marked Completed. PR99 merged as 45e2b25.
Tag v1.8.17 is published (GitHub release 2026-10-29T20:08:26Z). The user ran
1.8.17 on the CP-7861: **Resync** reported "not registered". PR102 fixes the
cause in 1.8.18.
All four images (amd64) have the 1.8.17, 1.8 and latest tags on one digest.
Server and worker report 1.8.17.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR98 | Claude Code | aba5e5d | Integrated |
| PR93, PR91, PR61 | Claude Code | - | Not in scope |

- Version 1.8.17: new optional functions without a breaking API change.
- Images are amd64 only (ADR 0050).
- PR98 changes the size of the `cisco-tftp` tmpfs volume in
  `docker-compose.cisco-tftp.yml` from 16m to 64m. Users with the sidecar must
  change their compose file and recreate the volume. The changelog says this.
- The user asked for the release before the hardware test of PR98.

## Checks

- Passed: clean merge. No new migrations. PR98 had green CI (22 passed,
  2 skipped) at the frozen head.
- Passed: version strings changed at the places of the 1.8.16 release commit.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Hardware test of resync, restart, ringtones, directory and last contact on
  the CP-7861.

## Next action

Open the integration PR, wait for CI, merge, check main, push v1.8.17 once and
verify the release and images.

## References

- docs/release-workflow.md
- docs/ai-memory/20261029-claude-cisco-enterprise-features.md
- docs/ai-memory/20261029-claude-release-1.8.16.md
