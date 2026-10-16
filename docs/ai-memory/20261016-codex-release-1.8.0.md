# TalkOps 1.8.0 release

## Task state

- Task ID: 20261016-codex-release-1.8.0
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.7.15
- Base commit: 0f128ad210028b386081db0bdae90c210ff84523
- Related commits or PR: integration head 5bd0d65; [integration PR 45](https://github.com/itsh-neumeier/talkops/pull/45)

## Goal

Integrate all open PRs and publish version 1.8.0, as the user now requests. Verify the
combined candidate, exact main and all published artifacts. Preserve foreign
work. Record the breaking backup API and the fixed version in the release notes.

## Result

All eight ready source heads are integrated with merge commits. None was in
the base. No open contribution is omitted or pending. All features are from
Claude Code; Codex supplies integration, validation and release work.
Publication has not started. The final 1.7.14 audit history is preserved too.

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
- Pending: final candidate CI/E2E, exact-main CI/E2E, release and artifacts.
  Initial combined candidate bf3b55c: PR CI 38937833861/E2E 38937833472,
  push CI 38937820115/E2E 38937819970 are running. Web/browser and Compose
  checks passed; Clippy passed and Rust tests are running.
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

Pass checks on the final 1.8.0 candidate, merge PR 45 and pass exact-main
CI/E2E. Recheck scope and tag, then publish and verify all artifacts.

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

- User replies select 1.8.0 twice. Update all Cargo/web version metadata and
  CHANGELOG.md. The source scope is unchanged: PRs37-44, all eight exact heads.
- Preliminary 1.7.15 candidate eaad6df7bac6107d4ee44fa8b2381c7b7d713cde had green
  CI on both events: PR38939657493, push38939650450. E2E runs38939657587 and
  38939651093 are superseded by final 1.8.0 checks. No prior tag was created.
- Use fresh final-version CI/E2E and exact-main checks before publication.
Version metadata validated against eaad6df7bac6107d4ee44fa8b2381c7b7d713cde:
only Cargo workspace and npm root version fields change to 1.8.0.
