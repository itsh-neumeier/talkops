# TalkOps 1.8.22 release

## Task state

- Task ID: 20261030-claude-release-1.8.22
- Date: 2026-10-30
- Tool: Claude Code (release manager)
- Status: In progress
- Branch: codex/release-1.8.22
- Base commit: main, v1.8.21
- Related commits or PR: PR111, integration PR pending

## Goal

Publish the Cisco locale directory fix as stable TalkOps 1.8.22. The user
requested the release.

## Result

PR111 is merged into the release branch with its history. Release metadata is
updated. The 1.8.21 summary is marked Completed.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR111 | Claude Code | see PR | Integrated |

- Version 1.8.22: bug fix without a breaking API change and without a
  migration.
- Images are amd64 only (ADR 0050). CI runs on GitHub-hosted runners.

## Checks

- Passed: clean merge. No new migrations.
- Passed: version strings changed at the places of the 1.8.21 release commit.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Device test on the CP-7861: resync and check the German menus.

## Next action

Open the integration PR, wait for CI, merge, check main, push v1.8.22 once and
verify the release and images.

## References

- docs/release-workflow.md
- docs/ai-memory/20261030-claude-cisco-locale.md
