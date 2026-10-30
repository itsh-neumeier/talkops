# TalkOps 1.8.18 release

## Task state

- Task ID: 20261029-claude-release-1.8.18
- Date: 2026-10-29
- Tool: Claude Code (release manager)
- Status: Completed
- Branch: codex/release-1.8.18
- Base commit: 45e2b25 (main, v1.8.17)
- Related commits or PR: PR102, PR103 (merge b7e46ad), tag v1.8.18

## Goal

Publish PR102 (Cisco Enterprise registration user agent fix) as stable
TalkOps 1.8.18. The user requested the release.

## Result

PR102 is merged into the release branch with its history. Release metadata is
updated. The 1.8.17 summary is marked Completed. No main merge, tag or
publication yet.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR102 | Claude Code | 21cfc69 | Integrated |
| PR101, PR100, PR93, PR91, PR61 | Claude Code | - | Not in scope |

- Version 1.8.18: bug fix without a breaking API change.
- Images are amd64 only (ADR 0050).

## Checks

- Passed: clean merge. No new migrations. PR102 had green CI (22 passed,
  2 skipped) at the frozen head.
- Passed: version strings changed at the places of the 1.8.17 release commit.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Hardware test of resync, restart and last contact on the CP-7861 with 1.8.18.

## Next action

None. Published by the Cisco session; release and images checked on 2026-10-30.

## References

- docs/release-workflow.md
- docs/ai-memory/20261029-claude-cisco-registration-agent.md
- docs/ai-memory/20261029-claude-release-1.8.17.md
