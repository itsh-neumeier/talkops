# TalkOps 1.8.23 release

## Task state

- Task ID: 20261030-claude-release-1.8.23
- Date: 2026-10-30
- Tool: Claude Code (release manager)
- Status: In progress
- Branch: codex/release-1.8.23
- Base commit: 5e13f5d (main, v1.8.22)
- Related commits or PR: PR113, PR114, PR115, integration PR pending

## Goal

Publish the Cisco directory menu fix, the provisioning file explorer and the
Cisco Enterprise ringtones, speed dials and locale select as stable TalkOps
1.8.23. The user requested one release after all three items were done.

## Result

PR113, PR114 and PR115 are merged into the release branch with their history.
The CHANGELOG conflicts are resolved into one 1.8.23 section. Release metadata
is updated. The 1.8.22 summary is marked Completed.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR113 | Claude Code | see PR | Integrated |
| PR114 | Claude Code | see PR | Integrated |
| PR115 | Claude Code | see PR | Integrated |

- Version 1.8.23: new features without a breaking API change.
- New migration `20261114000001_provisioning_files`. It runs at start.
- Images are amd64 only (ADR 0050). CI runs on GitHub-hosted runners.

## Checks

- Passed (rust:1-trixie + Postgres 17, merged branch): `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test -p talkops-provisioning`, `cargo test -p talkops-api --test
  phones --test multi_vendor --test provisioning_files`.
- Passed: version strings changed at the places of the 1.8.22 release commit.
- Pending: CI and E2E on the integration PR and on main.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Device test on the CP-7861: contacts menu, speed dials, ringtones after a
  WAV upload, Cisco locale select.
- Browser test of the phone page tabs and the file explorer.
- Network locale (`networkLocale`, `g3-tones.xml`) is not implemented.

## Next action

Open the integration PR, wait for CI, merge, check main, push v1.8.23 once and
verify the release and images.

## References

- docs/release-workflow.md
- docs/ai-memory/20261030-claude-provisioning-files.md
- docs/ai-memory/20261030-claude-cisco-ringtones-keys.md
- docs/adr/0052-admin-provisioning-files.md
