# TalkOps 1.8.19 release

## Task state

- Task ID: 20261030-claude-release-1.8.19
- Date: 2026-10-30
- Tool: Claude Code (release manager)
- Status: Completed
- Branch: codex/release-1.8.19
- Base commit: 2fdf037 (main)
- Related commits or PR: PR104, PR105, PR106 (merge 3bbdb31), tag v1.8.19

## Goal

Publish the Cisco Enterprise call manager layout fix as stable TalkOps 1.8.19.
The user requested the release.

## Result

PR104 and PR105 are merged into the release branch with their history.
Release metadata is updated. The 1.8.18 summary is marked Completed.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR105 | Claude Code | 142f7ad | Integrated |
| PR104 | Claude Code | a47400f | Integrated |

- Version 1.8.19: bug fix without a breaking API change.
- Images are amd64 only (ADR 0050).
- The user approved removal of the repository variable `CI_RUNNER`. The
  self-hosted runner now runs three replicas on one Docker daemon. Fixed
  service ports (Postgres 5432) and loopback health checks fail there. CI runs
  on GitHub-hosted runners until the runner setup is fixed.

## Checks

- Passed: clean merge. No new migrations.
- Passed: version strings changed at the places of the 1.8.18 release commit.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Hardware check: the CP-7861 registers with the exported file from 1.8.19.
- Self-hosted runner: one replica with `network_mode: host`, or CI changes
  for parallel jobs on one Docker daemon. Then set `CI_RUNNER=talkops` again.

## Next action

None. Published; images checked. The CP-7861 registered on 1.8.19 on 2026-10-30.

## References

- docs/release-workflow.md
- docs/ai-memory/20261030-claude-cisco-ccm-layout.md
- docs/ai-memory/20261029-claude-ci-self-hosted-runner.md
