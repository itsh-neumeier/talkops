# TalkOps 1.8.20 release

## Task state

- Task ID: 20261030-claude-release-1.8.20
- Date: 2026-10-30
- Tool: Claude Code (release manager)
- Status: In progress
- Branch: codex/release-1.8.20
- Base commit: 3bbdb31 (main, v1.8.19)
- Related commits or PR: PR107, integration PR pending

## Goal

Publish the Cisco TFTP export-before-resync fix as stable TalkOps 1.8.20.
The user requested the release.

## Result

PR107 is merged into the release branch with its history. Release metadata is
updated. The 1.8.19 summary is marked Completed.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR107 | Claude Code | see PR | Integrated |

- Version 1.8.20: bug fix without a breaking API change.
- Images are amd64 only (ADR 0050). CI runs on GitHub-hosted runners
  (`CI_RUNNER` is not set).

## Checks

- Passed: clean merge. No new migrations.
- Passed: version strings changed at the places of the 1.8.19 release commit.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Cisco German locale: the user downloads the Cisco locale package; upload and
  TFTP export are separate work.

## Next action

Open the integration PR, wait for CI, merge, check main, push v1.8.20 once and
verify the release and images.

## References

- docs/release-workflow.md
- docs/ai-memory/20261030-claude-cisco-tftp-refresh.md
