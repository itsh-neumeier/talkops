# TalkOps 1.8.2 release

## Task state

- Task ID: 20261017-codex-release-1.8.2
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.2
- Base commit: 86772ec0c31990f7a29a639ffb90db6ebef217e2
- Related commits or PR: Pending

## Goal

Integrate all open contributions and publish the user-selected version 1.8.2.
Verify the final candidate, exact main and published artifacts. Preserve foreign
work and all published tags.

## Result

The only open contribution is ready PR49 from Codex. No Claude Code contribution
is assigned. Its exact head e74a06c51c5fc84dd61d0903d6396ddc09660906 is integrated
with its source history and was not in the base. There is no other promised or
blocked contribution in the repository handoff. Publication is pending validation.

| PR | Source branch | Exact source head | Dependency | Migration ID |
| --- | --- | --- | --- | --- |
| [49](https://github.com/itsh-neumeier/talkops/pull/49) | codex/provisioning-multivendor-plan | e74a06c51c5fc84dd61d0903d6396ddc09660906 | None; PR47 is already in base | 20261105000001 |

## Changes

- PR49: Model-based programmable and expansion keys, ordered call-state keys,
  explicit provisioning transport, redacted draft preview, download diagnostics,
  immutable native template revisions and vendor dispatch extension points.
- Include the 433 original FusionPBX files from 70 source folders at upstream
  be0d0e3220e6b3666775fa46db6f1accc60c79c6. They are reference data, not executed
  Smarty. Notices and reference license are included in the server image.
- Preserve the final 1.8.1 audit history from audit/release-1.8.1.
- Set Cargo and web versions to 1.8.2. Add English release notes and the
  missing ADR0038 index entry. No production integration fix was needed.

## Decisions

- Use the explicitly requested version 1.8.2. Target main is the verified
  remote default. Merge commits are allowed; no active ruleset or main branch
  protection was reported. Preserve source history without rewriting it.
- Phone input provisioning is optional; omitted fields retain saved settings.
  Existing credentials, accounts and line-key storage remain compatible.
- The 70 source folders do not expand the active supported model catalog.
  Additional vendor adapters and real-device tests are separate follow-ups.
- Preserve closed, unmerged PR36 branch claude/vigilant-tesla-ef0027 at
  37f8a18aa2971c1a570821d150dcf0941fae12eb and reserve ADR0033.
- Preserve primary checkout 397fc83 and its three untracked Dahua summaries.
  Do not use foreign chats or import uncommitted work from other worktrees.
- Source Cargo changes enable existing MiniJinja loader/fuel and direct
  serde_json/url use. No package version changes. Release lock changes must
  be version-only against the integrated source baseline.

## Checks

- Passed at inventory: all 12 checks on exact source head e74a06c. PR CI38973944625
  and E2E38973943991; push CI38973939305 and E2E38973939288.
- Pending: combined local checks, final candidate CI/E2E, exact-main CI/E2E,
  publication workflow, GitHub release and all three two-platform manifests.
- Full Linux workspace/DB/LDAP and Docker/SIPp checks use CI. No local Linux
  Docker daemon is available. The source handoff reports an unchanged Windows
  media-worker test warning in full workspace Clippy; Linux Clippy passed.
- CodeRabbit CLI is unavailable. Manual integration review checks actual API,
  UI, migration, tenant scope, permissions, template limits and preview contracts.
- STE review: Manual clarity review. The full official standard is unavailable;
  full language review remains separate from functional checks.

## Dependencies and open work

- Back up the database, then apply 20261105000001_provisioning_profiles before
  the new API starts. All 45 released migrations must remain unchanged.
- A phone HTTP exception needs an initial reachable device/DHCP address and
  credentials. No automatic downgrade or global TLS validation change is added.
- T41P firmware 36.83.0.120 HTTPS behavior still needs a phone-level TLS, DNS,
  time and authentication comparison. A successful NOTIFY is separate from
  a successful download. No phone access is available here.
- Key/profile/template changes require a configuration download. Template
  activation alone does not resync. Run the documented device acceptance steps
  after deployment, including T41P keys/reset and AX86R HTTPS regression.
- Existing 1.8.0 backup API, notification and Dahua follow-ups remain separate.

## Next action

Publish the integration PR. Run combined checks and wait for final-head CI/E2E.
Merge, verify exact main, then push one unused v1.8.2 tag and verify publication.

## References

- docs/release-workflow.md; docs/release-manager-prompt.md; ADR0038.
- 20261016-codex-provisioning-multivendor-plan.md.
- 20261016-codex-release-1.8.1.md (completed previous release).
