# TalkOps 1.8.2 release

## Task state

- Task ID: 20261017-codex-release-1.8.2
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.2
- Base commit: 86772ec0c31990f7a29a639ffb90db6ebef217e2
- Related PR: https://github.com/itsh-neumeier/talkops/pull/50

## Goal

Integrate all open contributions and publish the user-selected version 1.8.2.
Verify the final candidate, exact main and published artifacts. Preserve foreign
work and all published tags.

## Result

PR49 and integration PR50 are merged. The entire agreed contribution is from
Codex; no Claude Code contribution was assigned. Exact source head
 e74a06c51c5fc84dd61d0903d6396ddc09660906 is preserved and was not in the base.
No agreed contribution is omitted or blocked. Exact-main checks passed. The
new tag is pushed; publication and artifact verification are in progress.

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
- Passed locally: cargo fmt, npm ci/lint/check, 34 web tests and production
  build; zero Svelte errors and warnings; four Compose configurations.
- Passed: 46 unique ordered migrations, previous 45 unchanged; 2,094 matching
  EN/DE keys/placeholders without duplicates; all 13 browser suites; 37 unique
  ADR files and 38 resolving index links; exact source ancestry; version-only
  release metadata against the integrated source baseline.
- Passed: all 433 reference Git blob IDs and bytes at the pinned upstream head.
- Passed: all 12 final-candidate checks, including 375 Rust tests, one existing
  ignored test, web/browser suites, Compose and version1.8.2 image smoke tests.
- Passed: exact-main CI39006646325 and E2E39006646300 before tagging.
  Main E2E passed on its first attempt. Publication and artifact checks remain pending.
- Local provisioning test build was not started: C: has less than 4 GiB free.
  Do not remove foreign outputs. Linux CI supplied the full profile tests.
- The first local version checker treated an unrelated dependency version
  1.8.1 as a root version. Corrected the checker to compare workspace/npm roots
  only. Actual locks have no unrelated upgrade; no candidate change resulted.
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

Wait for Release39007742037 and additional tag checks. Verify the GitHub
release, three versioned two-platform manifests, all six OCI version/revision
labels and moving 1.8/latest tags. Then archive the final record and clean
only unchanged integrated branches.

## References

- docs/release-workflow.md; docs/release-manager-prompt.md; ADR0038.
- 20261016-codex-provisioning-multivendor-plan.md.
- 20261016-codex-release-1.8.1.md (completed previous release).

## Candidate and main checkpoint

- Final candidate: bb79930c23d01b252de55c139b93dd0564961f3d.
  All 12 checks passed. PR CI39005493440/E2E39005493573 and push
  CI39005490181/E2E39005490056. Both E2E runs passed on their first attempts.
- CI logs confirm HTTP override preservation, actual SIP-secret redaction,
  template draft/activation/restore, tenant isolation and request retention,
  T41P keys and reset, all active model fixtures and source integrity.
  Browser coverage includes 320/768/1280 px, both themes, contact/mobile
  selection, save/resync ordering, preview and failed activation preservation.
- PR50 merged at 2026-10-17T05:32:37Z. Main commit:
  c4765b0495adf004f0b2b0f5099cbd098f384f86.
  Its tree equals the tested candidate tree. The agreed source is an ancestor.
- Source PR49 auto-closed as merged at 05:32:39Z. No other open contribution
  was present. Main CI39006646325 and E2E39006646300 were started.
- No test was weakened and no gate waived. No release tag exists yet.
- Do not push the batch branch after merge. Local checkpoint commits change
  only this task record. Archive the final record under audit/release-1.8.2
  after publication, without changing main or the released code.

## Tag checkpoint

- Exact-main CI39006646325 and E2E39006646300 passed on the release commit.
  Rechecked latest-main run IDs, current remote main, empty open-PR scope,
  exact source ancestry, versions and unused tag immediately before publication.
- Annotated v1.8.2 was pushed once at 2026-10-17T05:50:41Z. Tag object:
  3e5834d117026e29b2099098af47024fe0c25abc. Peeled target:
  c4765b0495adf004f0b2b0f5099cbd098f384f86. No published tag moved.
- Publication: Release39007742037. Additional tag CI39007742064,
  E2E39007742124 and FreeSWITCH39007742129 are in progress.
