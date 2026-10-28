# TalkOps 1.8.14 release

## Task state

- Task ID: 20261028-codex-release-1.8.14
- Date: 2026-10-28, Europe/Berlin
- Tool: Codex
- Status: In progress; candidate validation
- Branch: codex/release-1.8.14
- Base commit: 4b44cd68580908fb54407898aa01b0dcc4f80954
- Related PR: Pending

## Goal

Integrate all ready open contributions and publish stable TalkOps 1.8.14.
Preserve source histories, unmerged work and every foreign worktree.

## Result

Merged the verified 1.8.13 audit and Codex PR88. Updated release metadata,
English changelog and current EN/DE installation examples. No main merge,
tag or publication yet.

## Decisions and scope

| Source | Contributor | Frozen head | Status |
| --- | --- | --- | --- |
| PR88 | Codex | f4d352223c9ec3f2f88924a11a18aaf02f319433 | Integrated; source 22 required checks passed |
| PR89 | Codex | 00fe4e5ac6b13ec9a43e3674371f627b59841cb7 | Draft Cisco profiles; excluded and preserved |
| PR61 | Claude Code | 16c6c45a61655a9ff215c350ac47d499d15dae9f | Draft Wi-Fi proposal; excluded and preserved |

- No missing agreed ready contribution. Neither draft is approved for this batch.
  Recheck readiness before publication. No new feature work in this release task.
- Target main and permitted merge method verified. No protection or rulesets
  at intake. Latest stable was 1.8.13; v1.8.14 was unused.
- PR88 has no open dependency, migration, API or application dependency change.
- Include only the host installer and handbooks. No model archive, weights or
  release asset is included. The helper needs the separately supplied archive.
  An application update alone does not install a model. Source handoff marks
  distribution clearance and production recognition quality as pending.
- Coral stays experimental, opt-in and disabled by default. No hardware,
  production installation, model upload or legal clearance is claimed.
- Shell helpers use LF through the source .gitattributes change.

## Checks

- PR88: 22 required CI/E2E checks passed; two branch-only trial contexts skipped.
  PR CI40693471894/E2E40693471868; push CI40693464038/E2E40693464327.
- Local installer: Git Bash syntax check and invalid-archive checksum rejection
  passed. Checksum failure occurs before Docker operations.
- Source handoff records isolated Docker install/reinstall, backup, model digest,
  non-root access, restart and mock health checks. No physical recognition test.
- Manual integration review: exact archive digest before writes, named-volume
  and model-directory checks, symbolic-path rejection, two-file extraction,
  existing-model backup and Coral-only restart. EN/DE instructions agree.
- Static check: only eight workspace package versions and npm root versions
  changed from the combined source baseline. No unintended dependency upgrade.
  All 59 released migration files unchanged. 2,829 matching EN/DE keys/placeholders.
- Local docs: 13 tests passed; Wiki 75 pages/690 local links passed. Rust fmt passed.
- The initial temporary metadata checker still matched the old lock version.
  Corrected that checker from 1.8.12 to 1.8.13; strict comparison then passed.
  The Cargo diff contained only the eight intended package version changes.
- Full combined Linux Rust/database/LDAP, image/runtime, Compose and SIPp/browser
  proof will come from final candidate CI/E2E, then exact-main CI/E2E.
- CodeRabbit CLI unavailable at the previous check; manual review used. No
  CodeRabbit result claimed. Full STE standard unavailable; language review pending.

## Dependencies and open work

The separate model archive and a running Coral service are required to use the
installer. Custom mounts need manual installation. Keep draft89 and draft61.
Do not include model weights or perform a production installation in this task.

## Next action

Validate final candidate, merge, validate exact main, push v1.8.14 once, then
verify publication, all four images and moving tags. Archive the final audit.

## References

- https://github.com/itsh-neumeier/talkops/pull/88
- https://github.com/itsh-neumeier/talkops/pull/89
- https://github.com/itsh-neumeier/talkops/pull/61
- docs/ai-memory/20261028-codex-coral-missing-model.md
- docs/ai-memory/20261027-codex-release-1.8.12.1.md
- docs/release-workflow.md
