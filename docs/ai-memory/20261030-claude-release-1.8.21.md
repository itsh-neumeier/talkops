# TalkOps 1.8.21 release

## Task state

- Task ID: 20261030-claude-release-1.8.21
- Date: 2026-10-30
- Tool: Claude Code (release manager)
- Status: In progress
- Branch: codex/release-1.8.21
- Base commit: 2bc991a (main, v1.8.20)
- Related commits or PR: PR109, integration PR pending

## Goal

Publish the Cisco Enterprise user locale upload as stable TalkOps 1.8.21.
The user requested the release.

## Result

PR109 is merged into the release branch with its history. Release metadata is
updated. The 1.8.20 summary is marked Completed.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR109 | Claude Code | a13e5a3 | Integrated |

- Version 1.8.21: new feature without a breaking API change and without a
  migration.
- Images are amd64 only (ADR 0050). CI runs on GitHub-hosted runners
  (`CI_RUNNER` is not set).

## Checks

- Passed: clean merge. No new migrations.
- Passed: version strings changed at the places of the 1.8.20 release commit.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Device test on the CP-7861: upload the de_DE package, set the device
  language to German, resync.
- Network locale (call progress tones) is separate work.

## Next action

Open the integration PR, wait for CI, merge, check main, push v1.8.21 once and
verify the release and images.

## References

- docs/release-workflow.md
- docs/ai-memory/20261030-claude-cisco-locale.md
