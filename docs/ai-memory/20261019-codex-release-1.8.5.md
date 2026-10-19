# TalkOps 1.8.5 release

## Task state

- Task ID: 20261019-codex-release-1.8.5
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.5
- Base commit: 488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d
- Integration PR: Pending

## Goal

Integrate every open contribution and publish version 1.8.5 as requested.

## Result

Both ready Codex PRs are integrated with source history preserved:
- PR55: a535797c12a26c977111d9f3dc5a174e433d66d6, fax storage repair.
- PR56: 823a59faf346a3dbbe8a5b8f4a72216ccda30f07, blocked reasons and local whitelist.

Each source has 14 successful exact-head checks. PR56's handoff referenced
runtime checks; its final documentation head also has successful CI/E2E.
Neither source was in main. No other open PR, assigned Claude Code contribution
or promised handoff is missing. Keep the final 1.8.4 audit in the merged history.

## Changes

- PR55: actual writable storage probe, safe 503/507 errors, retained attachments,
  administrator storage check, documented volume repairs and image smoke tests.
- PR56: stored blocking reason, exact tenant whitelist before local/PhoneBlock
  rules, separate permission, atomic audit, call views, filters and statistics.
- Release metadata: Cargo workspace/lock versions, npm root versions and changelog.
- This file: integration and publication evidence.

## Decisions

- Resolve the DE/EN translation conflict by retaining both independent additions.
  Static checks compare every changed source translation against the merged result.
  Both CI additions remain present. No other integration correction was needed.
- Migration 20261106000003 follows the previous 48 unchanged SQL migrations.
  All 49 IDs are unique. Historical calls keep null reasons; do not infer them.
- Non-admin roles receive no automatic blocking.allowlist grant. Whitelisting
  preserves original call decisions and does not bypass other routing rules.
- No dependency version upgrade. Package tools made version-only lock edits.
- Preserve foreign files/worktrees and closed, unmerged PR36. Never move a tag.
- Gate tag creation on final candidate and exact-main CI/E2E success.

## Checks

- Passed locally: fmt; clean web install/lint/check (zero errors/warnings),
  38 tests/build; four Compose variants; migrations and version-only locks.
- Passed locally: 2,283 matching EN/DE keys/placeholders; both source translation
  additions preserved; 16 browser suites, 39 unique ADRs and valid index links.
  Original provisioning hashes and both generator checks passed.
- Manual integration review: safe fax errors, storage probe cleanup, volume
  repair scope, tenant/role checks, whitelist precedence and atomic audit,
  CDR/spool historical markers, filters/statistics and API/UI contracts.
- Source55: CI39233215945 and E2E39233215937 passed. Source56 final head has
  green runs39249343901/39249343915/39249349840/39249349914.
- CodeRabbit CLI unavailable. No CodeRabbit review ran.
- Not run locally: full Linux Rust/database/LDAP, image smoke/storage tests,
  Python conversion and fax/SIPp/browser stack. Local Linux Docker unavailable;
  use final candidate and main CI/E2E for complete evidence.
- STE review: repository clarity rules applied. Full standard unavailable;
  full language review remains pending.

## Dependencies and open work

No PR code dependency or missing handoff. Back up and apply 20261106000003
before starting the new binary. Server, worker and FreeSWITCH must share the
same writable fax volume/path with UID/GID 10001 access. Update old deployed
Compose/Portainer mounts while preserving documents and read-only root filesystems.
An image update alone does not repair old mounts. The administrator check tests
server access only. Retest provider send/receive and the intended local spam
exception after deployment. No production repair or live acceptance is claimed.

## Next action

Run final candidate checks, merge, verify exact main, publish and verify release
plus all three manifests for amd64/arm64 and stable moving tags.

## References

- https://github.com/itsh-neumeier/talkops/pull/55
- https://github.com/itsh-neumeier/talkops/pull/56
- docs/ai-memory/20261019-codex-fax-send-receive-fix.md
- docs/ai-memory/20261019-codex-spam-blocklist-visibility.md
- docs/ai-memory/20261019-codex-release-1.8.4.md
- docs/release-workflow.md

## Merged checkpoint

- Integration PR: https://github.com/itsh-neumeier/talkops/pull/57
- Final candidate: 9f1a29ed88e73b1e6dfe65fe7e308f4b337aa7b1.
- All 14 final candidate checks passed: PR CI39252972813/E2E39252972792 and
  push CI39252965249/E2E39252965273. Rust: 413 passed, one existing ignored.
- Both fax media round trips passed. The blocked-call historical reason and
  exact-whitelist next-call scenario ran; all SIPp/browser E2E checks passed.
- PR57 merged 2026-10-19T12:48:29Z. PR55/PR56 became merged through it two seconds later.
- Main: b1fa28350dc1830e222a27ee770641edb1935ccc. Its tree equals the candidate;
  both source heads are ancestors. Source heads and old main were unchanged.
- Exact-main CI39254950679 and E2E39254950487 are running. No tag exists yet.
- FreeSWITCH sources/workflow did not change; no separate main FreeSWITCH run applies.
- Do not push the batch branch again. Archive later task-only changes with the audit ref.

## Publication started

Exact-main CI39254950679 and E2E39254950487 passed. Fresh guards verified
unchanged main, empty open scope, both source ancestors, identical candidate
and main trees, matching package versions and latest successful main gates.
New annotated v1.8.5 was pushed once on b1fa28350dc1830e222a27ee770641edb1935ccc.
Tag object: 0963dfb4500904f26c40be88814c9a7e0cfa0667.
Release39256855610 is running. Extra tag CI39256855605, E2E39256855675 and
FreeSWITCH39256855604 are running. Artifact verification remains pending.
