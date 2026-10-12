# Yealink internal phonebook dialing

## Task state

- Task ID: 20261012-codex-yealink-internal-phonebook
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: Completed. Application checks passed. Release integration is separate.
- Branch: codex/yealink-internal-phonebook
- Base commit: 4503fb3b96167aa77320cc936597fc6147b392c3
- Original application commit: 397c06cdb8e7171d3616e6c913ab7e7a2ef464a1
- Related PR: https://github.com/itsh-neumeier/talkops/pull/5

## Goal

Use **101 for internal calls from Yealink phonebooks. Prevent an internal
number from becoming +499543101. The user reviewed the proposal and selected
**101. Commit, push and PR creation are authorized. The release chat owns
integration, release versions, tags and publication.

## Result

Implemented explicit internal dialing, contact storage and CSV import.
The internal directory emits **101. Call pickup now uses *8*101. Updated
Yealink BLF provisioning, dial-now rules, UI hints and English/German guides.
Added regression tests and a second SIPp call with an explicit internal number.
No database migration or package dependency was added.
Committed and pushed the application change. Created PR #5. All application
CI and E2E checks passed. A final guard also prevents explicit internal calls
from using the separate SIP-address route. All 22 local routing tests passed
after that guard. The PR records current check results and readiness for later
revisions. No target merge or release publication was performed.

## Changes

- `crates/talkops-core/src/numbering.rs`: Parse explicit internal numbers.
  Update the pickup feature prefix. Keep stored extension numbers unchanged.
- `crates/talkops-core/src/phones.rs`, `contact_import.rs`: Accept and preserve
  valid ** numbers in contacts and import. Reject malformed star numbers.
- `crates/talkops-api/src/routes/provisioning.rs`: Emit ** extension numbers.
- `crates/talkops-api/src/fsxml/dialplan.rs`: Resolve ** calls and blind
  transfers in the caller's tenant. Reject unknown targets without a trunk
  fallback. Prevent explicit internal targets from using a direct SIP peer.
  Move call pickup to *8*.
- `crates/talkops-provisioning/`: Update BLF pickup codes and dial-now rules.
  Explicit internal rules work with an area code and avoid feature collisions.
- `crates/talkops-api/tests/freeswitch.rs`, `tests/phones.rs`: Test routing,
  transfer, disabled/foreign targets, search, contact CRUD and section import.
- `tests/e2e/run.sh`: Call 210 and **210. Verify XML and the BLF pickup code.
- `web/src/lib/i18n/`: Update English/German contact and feature-code hints.
- `docs/en/`, `docs/de/`, `docs/architecture.md`: Explain dialing and upgrade.

## Decisions

- The user explicitly selected **101 after the review identified its old
  pickup meaning. Use *8*101 for pickup to avoid existing feature prefixes.
- Existing bare numbers and the single-star alias remain available.
- Explicit calls support 2-8 digit internal numbers, including legacy numbers.
- A contact must use **101 for explicit internal dialing. Do not infer an
  extension by removing +499543 from a stored external number.
- Before this change, bare 101 becomes +499543101 when no internal target
  matches and the area code is 9543. Existing internal targets resolve first.
  Tests reproduce that path. The user's device-side transformation is unverified.
- The original checkout and other task worktrees remain unchanged. The task
  worktree starts from the verified remote target, ccr-df1c8083-0ce1he.

## Checks

- Passed: All 88 relevant local tests: 48 core tests, 11 provisioning tests,
  22 FreeSWITCH API tests and 7 phone API tests, with isolated PostgreSQL 17.
- Passed: Web npm ci, lint, check and build. No type errors or warnings.
- Passed: Rust formatting, Git whitespace check and bash syntax check.
- Passed: English/German catalog parity; 1,030 unique keys in each catalog.
- Passed: Final local routing rerun after the SIP-address guard, 22 tests.
  The regression includes a request URI host and an unknown ** target.
- Passed on application commit 397c06c: Linux CI, including full workspace
  fmt, clippy without exclusions, tests, web checks, Compose, image builds
  and smoke tests. PR run 38282000282; push run 38281907317.
- Passed on application commit 397c06c: SIPp and browser E2E, including real
  calls to 210 and **210, phonebook and BLF checks. PR run 38282000293;
  push run 38281907331. Browser call reported live audio in both directions.
- Manual review: Checked prefix routing, pickup/BLF, tenant scope, stored
  values, search, transfers, dial-now ambiguity and upgrade instructions.
- Failed then corrected: Windows CRLF templates failed existing LF assertions.
  Normalized embedded templates locally without unrelated Git content changes.
  Final reruns passed. Restored unrelated files. Web lint also passed after
  local LF normalization. No broad formatting changes are committed.
- Local clippy limitation: MSVC Rust 1.94.1 reports a pre-existing
  nonminimal_bool lint in unchanged identity.rs. Target CI with current stable
  passed. Local API/core/provisioning clippy passed with -D warnings and
  -A clippy::nonminimal_bool. Full Linux clippy passed without exclusions.
- Not run locally: Linux Docker builds and SIPp/browser E2E; daemon unavailable.
- Not run: CodeRabbit CLI review; CLI not installed. Manual review completed.
- STE review: Applied repository writing rules. The full official standard
  was unavailable; full language review is pending.

## Dependencies and open work

- Required PR dependencies: None. No migration IDs. No new packages.
- PR #3 and PR #4 also edit translation files. Preserve both sets of changes.
- Compatibility change: ** now calls the target. Pickup uses *8*. Resync all
  provisioned phones and reload their remote phonebooks after the update.
  Update manually configured pickup/BLF keys and saved pickup speed dials.
- Do not change release versions or publish from this branch. The release
  manager must account for the pickup-code compatibility change.
- Device/firmware validation is pending on the user's handset.
- The Git worktree exists but the app could not attach it as a managed worktree.
- The isolated local PostgreSQL test service was stopped. Worktree is preserved.
- No feature work remains. The PR is the release-manager handoff.

## Next action

The release chat selects the release version and integrates PR #5. Account
for the pickup-code compatibility change. Resync phones after installation.

## References

- ../release-workflow.md
- ../en/yealink.md
- ../en/first-steps.md
- https://github.com/itsh-neumeier/talkops/pull/3
- https://github.com/itsh-neumeier/talkops/pull/4
- https://github.com/itsh-neumeier/talkops/actions/runs/38282000282
- https://github.com/itsh-neumeier/talkops/actions/runs/38282000293
