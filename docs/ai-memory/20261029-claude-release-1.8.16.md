# TalkOps 1.8.16 release

## Task state

- Task ID: 20261029-claude-release-1.8.16
- Date: 2026-10-29
- Tool: Claude Code (release manager)
- Status: In progress
- Branch: codex/release-1.8.16
- Base commit: 145f3bc2699f2dbb84f2a9ea1b21dcafd6365597 (main, v1.8.15)
- Related commits or PR: PR94, PR95, PR96, integration PR pending

## Goal

Publish PR94, PR95 and PR96 as stable TalkOps 1.8.16. The user requested the
release after PR96.

## Result

The three branches are merged into the release branch with their history.
Release metadata is updated. No main merge, tag or publication yet.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR94 | Claude Code | c60673f | Integrated |
| PR95 | Claude Code | ee4c7a3 | Integrated |
| PR96 | Claude Code | 732e765 | Integrated |
| PR93, PR91, PR61 | Claude Code | - | Not in scope |

- Version 1.8.16: UI change, template fix and release build change without a
  breaking API change.
- With PR94 the tag build makes amd64 images only. The user asked for this.
  The release has no arm64 images.
- PR95 was planned as "PR only". The user now asked for a release. The
  rendered template is not retested on hardware; the user can retest with the
  1.8.16 image.

## Checks

- Passed: clean merges. No new migrations. PR94, PR95 and PR96 each had green
  CI (22 passed, 2 skipped) at the frozen heads.
- Passed: version strings changed at the places of the 1.8.15 release commit.
  Cargo.lock changes only the eight talkops workspace packages.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Retest the CP-7861 with the 1.8.16 server image and the normal sidecar.
- Calls with Cisco Enterprise phones are not tested.

## Next action

Open the integration PR, wait for CI, merge, check main, push v1.8.16 once and
verify the release and images.

## References

- docs/release-workflow.md
- docs/adr/0050-release-architectures.md
- docs/ai-memory/20261028-claude-release-amd64-default.md
- docs/ai-memory/20261029-claude-cisco-enterprise-tested-config.md
- docs/ai-memory/20261029-claude-vendor-names-badges.md
