# TalkOps 1.8.0 release

## Task state

- Task ID: 20261016-codex-release-1.8.0
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.7.15 (merged and remote deleted)
- Final record: immutable annotated tag audit/release-1.8.0
- Base commit: 0f128ad210028b386081db0bdae90c210ff84523
- Related commits or PR: release main 7d7990b99e9f02c126ad3983da633476a22a9708; [integration PR 45](https://github.com/itsh-neumeier/talkops/pull/45)

## Goal

Integrate all open PRs and publish version 1.8.0, as the user now requests. Verify the
combined candidate, exact main and all published artifacts. Preserve foreign
work. Record the breaking backup API and the fixed version in the release notes.

## Result

All eight ready source heads and PR45 are merged into main. None was in
the base. No agreed contribution is omitted or pending. Later PRs 46 and 47
arrived after the release tag and belong to a future release. All features are from
Claude Code; Codex supplies integration, validation and release work.
Version 1.8.0 is published. Final candidate, exact-main and tag CI/E2E passed.
Release, FreeSWITCH and registry verification passed. The final 1.7.14 audit
history is preserved too. No publication work remains for 1.8.0.

| PR | Exact source head | Dependency | Migration ID |
| --- | --- | --- | --- |
| [37](https://github.com/itsh-neumeier/talkops/pull/37) | 0ea197d70726c82a0c1ea779b0db1dff27eb0b18 | None | None |
| [38](https://github.com/itsh-neumeier/talkops/pull/38) | 6d36a12874cd9486989ebf2f6812a27c10c28da4 | None | None |
| [39](https://github.com/itsh-neumeier/talkops/pull/39) | c8a4d5b7e94784e8b6478453f7be0038c0116784 | None | None |
| [40](https://github.com/itsh-neumeier/talkops/pull/40) | da03f9fc7bd9325399acccb967a224029be44082 | None | 20261103000001 |
| [41](https://github.com/itsh-neumeier/talkops/pull/41) | 138335c4e1571207f623523ae46d5acd9e687585 | 37 | 20261104000001 |
| [42](https://github.com/itsh-neumeier/talkops/pull/42) | da6787013614eb57161b58c7586fb43c8be308e1 | 39 | None |
| [43](https://github.com/itsh-neumeier/talkops/pull/43) | 53bf32a02ef7932b797adcc04e7586d374e267b1 | 41 and 37 | 20261104000002 |
| [44](https://github.com/itsh-neumeier/talkops/pull/44) | 84ec89199c14ac9622d1d35e31062947b67bd24f | 40 | 20261103000002 |

## Changes

- Settings System sub-tabs and separate Backup and Storage sections.
- Resolved parties and own numbers in dashboard recent calls.
- Shared phonebook/call-list filters and sorted transcription history.
- HTML/text mail design, branding and a structured template editor.
- Shared storage targets, automatic migration of existing backup connections.
- Audit filters, downloads, incremental export, checksums and retention.
- Operations mail notifications with rules, delay/recovery, digest and rate limit.
- Resolve merge conflicts by keeping all settings sections and permission
  types, all browser routes and all ADR index entries. Keep both background
  workers and the existing backup permission checks.

## Decisions

- Target branch is main; merge commits are permitted. No active ruleset or
  main protection was reported. Sources remain unchanged.
- The user explicitly selected 1.8.0 in both version replies. This supersedes
  the original 1.7.15 request and its preliminary metadata. No 1.7.15 tag or
  release was created. Use 1.8.0 exactly; keep the backup API break in upgrade
  notes. Do not describe the API break as backward compatible or claim that a
  minor version meets strict SemVer for a public API break.
- Keep codex/release-1.7.15 as the original batch branch so PR45 and its review
  record stay together. The code, release title, task summary, tag and image
  labels use 1.8.0. Remove only this owned batch branch after release audit.
- Closed PR 36 (Cactus Whistle evaluation) is outside the open-PR scope. Its
  unmerged branch claude/vigilant-tesla-ef0027 at 37f8a18aa2971c1a570821d150dcf0941fae12eb
  must remain. ADR 0033 is reserved there; do not reuse it.
- Preserve primary checkout 397fc83 and its three untracked Dahua summaries.
  Do not import uncommitted changes or use foreign chat history.
- minijinja 2.24 with fuel is an intentional new direct dependency in core.
  It was already in Cargo.lock; source PR 40 adds the core dependency edge.
  No unrelated lockfile upgrade is authorized.

## Checks

- Passed at inventory: all eight exact source heads have 12 green CI/E2E
  checks each. Save the source run IDs in the integration PR.
- Passed after merges: cargo fmt; npm ci, lint, check, 25 tests and build.
- Passed: all eight exact heads are ancestors; 45 ordered unique migrations,
  previous 41 unchanged; 36 unique ADR files (0033 reserved), 61 local ADR links;
  1,929 matching EN/DE keys and placeholders; all ten browser suites retained.
- Passed: all 12 final-candidate checks, exact-main CI/E2E before tagging,
  Release and all additional tag checks. See immutable run IDs below.
  Earlier pre-version runs were superseded and are not the release evidence.
- Not run locally: full Rust/DB/LDAP and Linux stack tests. Windows GNU lacks
  dlltool.exe; Docker Linux daemon is unavailable. Use the workflow CI gates.
- CodeRabbit CLI is unavailable. Manual integration review uses actual diffs,
  contracts and tests. Do not claim a CodeRabbit review ran.
- STE review: manual clarity review. Full official standard unavailable;
  full language review pending separately from functional checks.

## Dependencies and open work

- Four new migrations, in ID order: mail templates, notifications, shared
  storage, audit export. Back up before upgrade. Existing connections keep
  their IDs, names, history and encrypted secrets through migration.
- New notifications are default-on for some events when SMTP and admin email
  are configured. New permissions: storage.manage, audit.manage,
  notifications.manage. Storage permission is granted to existing backup
  managers by migration; the other new sensitive permissions default to admin.
- Notification producers still planned: trunk.outbound_failures,
  freeswitch.xml_curl_failed, freeswitch.profile_down,
  freeswitch.channels_high, services.update_available.
- Real mail-client delivery, live gateway transitions, SMB and Portainer
  hardware tests are unavailable here. Use automated workflow evidence and
  report these limits. Preserve separate Dahua picture/CDR spool follow-ups.

## Next action

None for 1.8.0. The release and verification are complete. Preserve this final
record under audit/release-1.8.0 and link it from merged PR45. New PRs46 and47
need a separate release assignment; do not change v1.8.0.

## References

- docs/release-workflow.md; docs/release-manager-prompt.md; ADRs 0034-0037.
- Source task summaries 20261016-claude-settings-system-structure,
  dashboard-recent-calls, phonebook-sort-filter, mail-design-editor,
  storage-targets, transcription-history-filter, audit-export and notifications
  in this directory.

## PR checkpoint

- Stacked PRs41-44 now target main with unchanged heads. PR45 contains the
  complete scope and exact source checks. The original dependency chains stay.

## Version 1.8.0 checkpoint

- User replies select 1.8.0 twice. All Cargo/web version metadata and
  CHANGELOG.md were updated. The source scope is unchanged: PRs37-44, all eight exact heads.
- Preliminary 1.7.15 candidate eaad6df7bac6107d4ee44fa8b2381c7b7d713cde had green
  CI on both events: PR38939657493, push38939650450. E2E runs38939657587 and
  38939651093 are superseded by final 1.8.0 checks. No prior tag was created.
- Fresh final-version CI/E2E and exact-main checks passed before publication.
Version metadata validated against eaad6df7bac6107d4ee44fa8b2381c7b7d713cde:
only Cargo workspace and npm root version fields change to 1.8.0.

## Main merge checkpoint

- Final candidate d2e76cafa68daeb96c54b8f9ad022e5bd55f6ee9 passed all 12 checks.
  PR CI38941246995 and E2E38941247001; push CI38941241180 and E2E38941241187.
  The full SIPp/browser stack passed on both events. CLI smoke checks report
  talkops 1.8.0 and talkops-media-worker 1.8.0. Svelte: 0 errors/warnings.
- Existing backup migration, delegated backup/Portainer permissions, target
  staging capture and transcription-party tests passed. All ten browser
  suites passed. No test was weakened and no gate was waived.
- Body-to-final source differences: PR-link-only summary changes, except
  PR38's order-independent door-event test fix. All exact heads were checked.
- Rechecked all eight source heads and main unchanged before merge.
- PR45 merged at 2026-10-16T17:56:12Z. Release main commit:
  7d7990b99e9f02c126ad3983da633476a22a9708.
  Its tree equals the tested candidate; all eight exact heads are ancestors.
- Source PRs37-44 and integration PR45 are merged. No open PR remained at
  the merge and pre-tag checkpoints. Later arrivals are listed below.
- Exact-main CI38943496152 and E2E38943496202 passed before the tag push.
- This checkpoint is committed locally only. Do not push the batch branch
  after merge. Final audit documentation stays separate from released code.
## Tag checkpoint

- Exact-main CI38943496152 and E2E38943496202 passed. The latest-main run IDs
  were checked immediately before tagging. Remote main stayed at the release
  commit. No open PR remained; all eight source heads are ancestors.
- Annotated v1.8.0 was pushed once to 7d7990b99e9f02c126ad3983da633476a22a9708.
  Tag object: b29171493399297c658589c32adda9238ff9e97e.
- Code and web versions match 1.8.0. The tag was unused; no published tag moved.
- Release workflow, GitHub release and registry verification passed.

## Publication and registry verification

- Release38945722759 passed all six native builds, three manifest jobs and
  GitHub release creation. Tag CI38945722845 and FreeSWITCH38945722748 passed.
  Additional tag E2E38945722771 passed at 2026-10-16T18:24:49Z.
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.0
- Published at 2026-10-16T18:21:15Z. It is the latest stable release, not a
  draft or prerelease. English release notes cover all eight contributions.
- All three versioned manifests contain linux/amd64 and linux/arm64. All six
  OCI configurations report v1.8.0 and revision
  7d7990b99e9f02c126ad3983da633476a22a9708.
  Moving tags 1.8 and latest match the versioned platform digests.
- Server and worker CLI versions were checked in CI. No deployed UI or Linux
  runtime is available locally; do not claim a manual production UI test.

| Image | Architecture | Published platform digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:75d950647d73ee4022c1caab5446c5450b8c29a4022c9b191badeb8b5dd3db02 |
| talkops-server | arm64 | sha256:ff0155223e35110317a0e5c126a6b7e411c1c4b34a77a881c102f1d6e3aa5d8c |
| talkops-media-worker | amd64 | sha256:46d5c9b49218832decfadb01cca523093c8cd89e33351008087562bd5c28b371 |
| talkops-media-worker | arm64 | sha256:008fbb04a96f61b45be6e99f247c31de7387e63dfe5d7a30f904c2f3d76b845c |
| talkops-freeswitch | amd64 | sha256:4c1b12610462c6d7afc3c97d225cfef63c8aa59ecae5e9705028f6dd6251a90b |
| talkops-freeswitch | arm64 | sha256:9c72567eb26d5e66bf92778c1eb39283649cd903f7aabc1825d4cf5e6da16740 |
## Branch cleanup and later arrivals

- All eight remote source heads and the batch head were unchanged and were
  ancestors of the release commit. Deleted those nine integrated branches
  atomically with expected-head guards. No history was rewritten.
- Keep the unmerged closed-PR36 branch claude/vigilant-tesla-ef0027 at
  37f8a18aa2971c1a570821d150dcf0941fae12eb. It is not an ancestor of the release.
- New PR46, Codex country flags/loading, was created at 2026-10-16T18:18:01Z.
  Head f81e140b62abec0e709bd737f3d6d7d07fd1e4f2; branch codex/call-flags-skeleton;
  base main at the release commit. It was created after the tag push
  (Release run created at 18:13:49Z) and was a draft with pending checks.
  Its handoff asks for the next release. It is not part of 1.8.0. Keep its
  branch; do not change the published tag or silently add a later contribution.
- New PR47, Codex graphical Yealink provisioning keys, was created at
  2026-10-16T18:26:16Z. Head dce0e31b27975f1305982c0cf96b9294e6d3d6b9;
  branch codex/yealink-t41p-dss. It was a draft at final inventory. It also
  arrived after the tag and is outside 1.8.0. Keep its branch.
- Final GitHub branch inventory: main, claude/vigilant-tesla-ef0027,
  codex/call-flags-skeleton and codex/yealink-t41p-dss. Open PRs46 and47
  are later arrivals. No agreed 1.8.0 source was omitted; the pre-tag
  open-PR check was empty.
- Foreign local worktrees, branches and unpublished primary documentation
  remain. Only the owned batch branch can be removed after final audit.
## Final verification links

- [Final candidate CI](https://github.com/itsh-neumeier/talkops/actions/runs/38941246995)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38941247001).
- [Exact-main CI](https://github.com/itsh-neumeier/talkops/actions/runs/38943496152)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38943496202).
- [Release](https://github.com/itsh-neumeier/talkops/actions/runs/38945722759),
  [tag CI](https://github.com/itsh-neumeier/talkops/actions/runs/38945722845),
  [tag E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38945722771),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/38945722748).
- Every listed run passed on its expected immutable head. Main, published tag
  and all image revisions still match 7d7990b99e9f02c126ad3983da633476a22a9708.
- The final audit descendant changes only this task record. It is not a new
  release candidate and does not change main or the published release tag.
