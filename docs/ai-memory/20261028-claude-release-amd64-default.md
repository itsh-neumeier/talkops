# Release images amd64 by default, arm64 on request

## Task state

- Task ID: 20261028-claude-release-amd64-default
- Date: 2026-10-28
- Tool: Claude Code
- Status: Completed
- Branch: claude/release-amd64-default
- Base commit: 145f3bc2699f2dbb84f2a9ea1b21dcafd6365597 (main, v1.8.15)
- Related commits or PR: Pending

## Goal

The project owner decided that future releases build amd64 images only.
arm64 images are built only when the owner explicitly requests them.

## Result

The release workflow selects the architectures in the version job. It builds
amd64 by default. The repository variable `RELEASE_ARM64=true` (tag push) or
the `arm64` dispatch input adds arm64. ADR 0050 supersedes the architecture
part of ADR 0007. Release docs, the release-manager prompt, README and
EN/DE installation requirements state the new rule.

## Changes

- `.github/workflows/release.yml`: architecture selection step and output,
  matrix `arch` from that output, runner chosen by expression. The matrix
  `include` holds only image keys, because an include entry for an unselected
  architecture would create an extra job.
- `docs/adr/0050-release-architectures.md`: new ADR. ADR 0007 and ADR 0002
  point to it. The ADR index gets the entry and loses the blank lines that
  broke the table after PR 89.
- `docs/release-workflow.md`, `docs/release-manager-prompt.md`: verify amd64;
  verify arm64 only on an explicit request; set and remove the variable.
- `README.md`, `docs/en/installation.md`, `docs/de/installation.md`: amd64
  host requirement and the effect on `latest` and `1.8` for arm64 hosts.
- `docs/ai-memory/20261028-claude-release-1.8.15.md`: record the completed
  1.8.15 publication.

## Decisions

- Keep arm64 available on request instead of removing it, so a later arm64
  release needs no workflow change.
- A tag push cannot carry inputs, so a repository variable controls arm64.
- CI keeps its arm64 Coral image check. The owner asked to change releases only.

## Checks

- Passed: YAML parse of the workflow; matrix and outputs resolve as intended.
- Passed: actionlint 1.7.7 reports no new issue. Two SC2046 warnings in the
  unchanged manifest step also exist on main.
- Not run: the release workflow itself. The next tag push is its first run.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- PR 91 adds an ADR 0048 and must renumber; ADR 0050 is now also taken.
- arm64 hosts that use `latest` or `1.8` get no image after the next default
  release.

## Next action

Review and merge the PR before the next release.

## References

- docs/adr/0007-deployment-und-netzwerk.md
- docs/release-workflow.md
