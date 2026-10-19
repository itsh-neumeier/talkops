# TalkOps 1.8.4 release

## Task state

- Task ID: 20261019-codex-release-1.8.4
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.4
- Base commit: 7beb63d5b8e3917b7621588413afcec2a8893456
- Integration PR: Pending

## Goal

Integrate every open PR and publish the user-selected version 1.8.4.

## Result

The inventory has one open, ready contribution: Codex PR53, exact head
dac7d108f08a9452b98a3b197b521ff2233026ad. Its handoff is complete and all
15 source checks passed. Source CI39161227697, E2E39161227628 and
FreeSWITCH39161227565 passed on that head. No other open contribution or
assigned Claude Code handoff is pending. The source was absent from main.
Merge commits preserve source history and the completed 1.8.3 audit.
There were no conflicts or integration code corrections. Publication is pending.

## Changes

- PR53: SIP fax send/receive, preview-before-send, sender policies, shared
  inboxes, attachment-only mail input, reports, retention and backup/recovery.
- Cargo.toml, Cargo.lock, npm root versions and CHANGELOG.md: release metadata.
- This file: integration and publication evidence.

## Decisions

- Use the dedicated release checkout. Preserve all foreign work and worktrees.
- Keep the user-selected version 1.8.4. The feature is disabled until configured.
- Add migrations 20261106000001 and 20261106000002 in that order. Keep all
  46 previously released migrations unchanged. All 48 IDs are unique.
- No dependency version upgrades. Package tools change only release versions.
- Keep closed, unmerged PR36 and its branch outside the open contribution scope.
- Wait for final candidate and exact-main CI, FreeSWITCH and E2E before tagging.

## Checks

- Passed locally: cargo fmt; web clean install, lint, check with zero errors
  and warnings, 35 tests and production build; four Compose configurations.
- Passed locally: version-only lock verification, migration IDs and order,
  2,257 matching EN/DE keys/placeholders, 15 browser suites, 39 unique ADRs
  and valid index links; original provisioning hashes and both generators.
- Manual integration review: tenant/role checks, sender identity revalidation,
  uncertain-result retry limits, bounded adapters, mailbox certificate checks,
  attachment filtering, fax volume and backup/recovery paths, API/UI contracts.
- CodeRabbit CLI unavailable. No CodeRabbit review ran.
- Not run locally: full Linux Rust/database/LDAP, Python conversion fixtures,
  images, SIPp/browser and G.711/T.38 fax round trips. No local Linux Docker
  daemon is available. Use the exact-head CI/E2E checks for these requirements.
- STE review: repository clarity rules applied. Full standard unavailable;
  full language review remains pending.

## Dependencies and open work

- No source PR dependency or missing handoff.
- Back up and apply both migrations before starting the new binary. Do not
  downgrade after configuring fax destinations. Deploy the updated Compose
  file: server, worker and FreeSWITCH share the new fax volume.
- Server adds Python. Worker adds Python, Pillow, ReportLab, Poppler and fonts.
- Fax needs assigned permissions, sender numbers and dedicated receive routes.
  Retention defaults to 30 days in both directions.
- Mail input needs outgoing SMTP and verified IMAPS. The mail server must bind
  authenticated users to sender addresses. A From header alone is insufficient.
- Referenced users/numbers cannot be hard-deleted. Remove fax references and
  reassign destination ownership first. Disabled users lose sending permission.
- Real provider and deployment mail-server acceptance remain deployment tasks.

## Next action

Run the final candidate checks, merge, verify exact main, publish once and
verify GitHub release plus all six image architectures and moving tags.

## References

- https://github.com/itsh-neumeier/talkops/pull/53
- docs/ai-memory/20261018-codex-fax-sip-proposal.md
- docs/ai-memory/20261017-codex-release-1.8.3.md
- docs/adr/0040-fax-over-sip-and-mail-input.md
- docs/release-workflow.md

## Merged checkpoint

- Integration PR: https://github.com/itsh-neumeier/talkops/pull/54
- Final candidate: 4716681c5aaf3b581a046d7dfa9fa8b71dbca937.
- All 16 candidate checks passed: PR CI39206930642, E2E39206930647 and
  FreeSWITCH39206930576; duplicate push CI39206925728, E2E39206925734 and
  FreeSWITCH39206925645 also passed. Rust: 401 passed, one existing ignored.
- Fax audio and required T.38 transfers were confirmed; received PDFs checked.
  All existing SIPp/browser checks passed. Python fixtures passed.
- PR54 merged 2026-10-19T06:13:12Z; source PR53 merged through it one second later.
- Released-candidate main: 488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d.
  Its tree equals the tested candidate. Source ancestry was checked.
- Exact-main CI39208652398, E2E39208652451 and FreeSWITCH39208652404 are running.
  No tag exists yet. Publication remains gated on these exact-head results.
- Do not push the batch branch again. Save final records through the audit ref.
