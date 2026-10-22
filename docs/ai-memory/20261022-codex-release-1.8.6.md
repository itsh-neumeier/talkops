# TalkOps 1.8.6 release

## Task state

- Task ID: 20261022-codex-release-1.8.6
- Date: 2026-10-22, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.6
- Base commit: b1fa28350dc1830e222a27ee770641edb1935ccc
- Integration PR: Pending

## Goal

Integrate every open PR and publish the user-selected version 1.8.6.

## Result

Both ready contributions are integrated with full source history:
- Codex PR58: 60ff55f0326adc501d5b3c028c7748dfe8c09ce8; bilingual handbook and Wiki build.
- Claude Code PR59: 26cd5d0fc9c121512b2c59e8bb69d7f4be3c87ca; Wi-Fi survey stages 1-3.

Both final heads have successful CI/E2E (16 checks for PR58, 14 for PR59).
Neither was in main. No other contribution or handoff is pending or omitted.
The previous completed release audit is preserved. Publication is pending.

## Changes

- PR58: English/German README, handbook sources, checked Wiki generator and CI job.
- PR59: encrypted UniFi connection, GET-only client, survey lifecycle/media/radio
  samples, analysis, permissions, live/results/compare UI and range audio playback.
- Integration: retain new README layout and add survey links in both languages.
  Add the survey guide pair/navigation, all three UI routes and updated counts
  to the handbook manifest/indexes and coverage test. The combined source now
  has 35 topic pairs and 75 generated pages. No feature code was changed.
- Release metadata: Cargo workspace, seven workspace lock entries, npm roots,
  changelog and this task record. No dependency version upgrade.

## Decisions

- Use the dedicated release worktree. Preserve foreign files/worktrees and closed PR36.
- Keep requested 1.8.6 for all open contributions. PR59 suggests a minor release;
  the user's current explicit target selects 1.8.6 for this additive feature.
- Migrations 20261107000001 and 20261107000002 are unique and follow the unchanged
  49 released migrations. Apply before the new binary. Feature code *39 is reserved.
- UniFi writes and handset roaming provisioning are source follow-up stage 4,
  outside this ready PR. Hardware acceptance is separate and remains pending.
- PR58's live Wiki is a separately approved publication from source 9010545.
  Read-only remote check still found Wiki master 7fd1c4108386fd4cf3180756f78c86774e42dd2a.
  Do not overwrite that snapshot as part of application release integration.
  New repository sources generate 75 pages; future Wiki publication is separate.

## Checks

- Passed locally: cargo fmt; web lint/check (zero errors/warnings), 40 tests/build.
  Reused installed locked web dependencies; final clean install runs in CI.
- Passed locally: 13 documentation tests and Wiki --check (75 pages, 654 links),
  four Compose variants, provisioning hashes and both generator --check commands.
- Passed locally: version-only Cargo/npm lock edits; seven workspace packages;
  51 unique ordered migrations, previous 49 unchanged; 2,470 matching EN/DE keys
  and placeholders; 16 existing browser suites; 40 unique ADRs and valid index.
- Corrected integration check: original docs test fixed its old 34-topic count.
  Extend it to 35 topics/75 pages and map all new UI routes. All 13 tests then passed.
- Manual integration review: handbook routes/navigation and source links,
  role/tenant survey access, encrypted key, TLS/pin flow and GET-only client,
  bounded requests, range audio, migration order and API/UI/worker job contracts.
- Source58 final green runs: 39540887902/39540887914/39540890953/39540891003.
  Source59 final green runs: 39754211867/39754211924/39754219660/39754219785.
- CodeRabbit CLI unavailable. No CodeRabbit review ran.
- Not run locally: full Linux Rust/database/LDAP, containers, SIPp/fax/browser
  stack. Local Linux Docker unavailable; C: has less than 1 GiB free. Use CI.
- STE review: repository clarity rules applied. Full standard unavailable;
  full language review remains pending.

## Dependencies and open work

No missing PR or code dependency. New talkops-unifi crate uses already locked
packages. Keep server and worker together; back up and apply both migrations.
Survey data/recordings default to 30-day retention. Downlink measurements depend
on handset RTCP; MOS is an estimate. Test real Wi-Fi handset calls and a real
UniFi console, record Network version and classic stat/sta API-key compatibility.
No real hardware acceptance, live deployment or new Wiki publication is claimed.

## Next action

Run final candidate CI/E2E, merge, verify exact main, publish once and verify
GitHub release, three image manifests for amd64/arm64 and moving stable tags.

## References

- https://github.com/itsh-neumeier/talkops/pull/58
- https://github.com/itsh-neumeier/talkops/pull/59
- docs/ai-memory/20261020-codex-readme-wiki-proposal.md
- docs/ai-memory/20261021-codex-wiki-publication.md
- docs/ai-memory/20261022-claude-wifi-voice-survey.md
- docs/ai-memory/20261019-codex-release-1.8.5.md
- docs/adr/0041-wifi-voice-survey-and-unifi-correlation.md; docs/release-workflow.md

## Candidate documentation correction

Final review found installation/update examples still selected 1.8.5. Update
the active examples in both READMEs and paired installation, Portainer, updates
and reference guides to 1.8.6, including both config tag and image version.
Keep historical fax upgrade notes unchanged. All 13 documentation tests and
Wiki --check passed again (75 pages/654 links). The candidate must be checked
again on its new head. Earlier candidate runs are superseded, not final gates.
