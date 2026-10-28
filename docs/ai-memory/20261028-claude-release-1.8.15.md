# TalkOps 1.8.15 release

## Task state

- Task ID: 20261028-claude-release-1.8.15
- Date: 2026-10-28
- Tool: Claude Code (release manager; Codex reached its usage limit)
- Status: In progress
- Branch: codex/release-1.8.15
- Base commit: 977528ab223c449af4f863619d69dcaa49eaca43 (main, v1.8.14)
- Related commits or PR: https://github.com/itsh-neumeier/talkops/pull/89, integration PR pending

## Goal

Complete Codex draft PR89 (Cisco 7800 profiles, including the CP-7861) and
publish it as stable TalkOps 1.8.15. The user requested this scope.

## Result

PR89 failed CI in the new TFTP protocol test. Root cause and fix are recorded
in `docs/ai-memory/20261028-claude-cisco-tftp-privileges.md`. The fix is on
the PR89 branch. The PR89 branch is merged into the release branch with its
history preserved. Release metadata is updated. No main merge, tag or
publication yet.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR89 | Codex + Claude Code fix | 1125e59 | Integrated |
| PR91 | Claude Code | 039284c | Excluded: failing checks at intake; adds a second ADR 0048 |
| PR61 | Claude Code | 16c6c45 | Draft Wi-Fi proposal; excluded and preserved |

- The user chose the capability fix for the TFTP sidecar over removing TFTP.
- PR91 also adds `docs/adr/0048-*`. Its ADR number must change before it is
  integrated, because PR89 now takes ADR 0048 and 0049.
- Version 1.8.15: new optional features without a breaking change. The
  repository uses the patch number for all 1.8 releases so far.

## Checks

- Passed: clean merge of PR89 into main 977528a. No new migrations.
  ADR numbers 0048 and 0049 are free on main.
- Passed: version strings changed only at the places of the 1.8.14 release
  commit. Cargo.lock changes only the eight talkops workspace packages.
- Not run locally: cargo and npm are not installed on this host. CI on the
  integration PR is the validation contract.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- All Cisco profiles remain hardware-untested.

## Next action

Open the integration PR, wait for CI and E2E, merge, validate exact main, push
v1.8.15 once and verify the release, images and moving tags.

## References

- docs/release-workflow.md
- docs/ai-memory/20261028-codex-cisco-7800-profiles.md
- docs/ai-memory/20261028-codex-cisco-7800-enterprise.md
- docs/ai-memory/20261028-claude-cisco-tftp-privileges.md
- docs/ai-memory/20261028-codex-release-1.8.14.md
