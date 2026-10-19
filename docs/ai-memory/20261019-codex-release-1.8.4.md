# TalkOps 1.8.4 release

## Task state

- Task ID: 20261019-codex-release-1.8.4
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.4, now removed from origin
- Base commit: 7beb63d5b8e3917b7621588413afcec2a8893456
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/54

## Goal

Integrate every open contribution and publish the user-selected version 1.8.4.

## Result

TalkOps 1.8.4 is the latest stable release. The only open contribution was
Codex PR53, exact head dac7d108f08a9452b98a3b197b521ff2233026ad. Its handoff
was complete and all source checks passed. No Claude Code contribution was
assigned. No open PR or promised handoff was omitted. Source was absent from
main. PR54 and PR53 are merged with source history preserved. No conflict or
integration code correction was required. The completed 1.8.3 audit is retained.

## Changes

- PR53: SIP fax send/receive; PDF/image preview and separate send; saved header
  and footer; own/shared/global sender numbers; shared inboxes and role checks.
- Add attachment-only IMAPS mail input with exact sender whitelist, number-only
  subject and rejection notices. Body/HTML/quotes/inline signatures are excluded.
- Add status reports, cancellation, explicit resend, retention and backup/restore.
  Uncertain delivery never retries automatically. Fax stays disabled until configured.
- Add migrations 20261106000001 and 20261106000002, shared fax volume and image
  conversion packages. Add MIME/document and G.711/T.38 round-trip workflow tests.
- Release metadata: Cargo.toml, six Cargo.lock workspace entries, web/package.json,
  npm lock root versions and CHANGELOG.md. Package tools made version-only edits.
- This file: final source scope, gates, artifact digests and publication evidence.

## Decisions

- Use the dedicated release worktree. Preserve foreign files, branches and worktrees.
- Keep the user-selected version 1.8.4. The API addition is opt-in.
- All 48 migration IDs are unique and ordered. Previous 46 SQL files are unchanged.
- No Rust/npm dependency version upgrade. Existing workspace Tokio is now used by core.
- Preserve closed, unmerged PR36 and its branch; it is outside the open scope.
- Tag only after final candidate and exact-main CI, E2E and FreeSWITCH passed.
  One new annotated tag push used the existing Release workflow. No tag was moved.

## Checks

- Passed locally: cargo fmt; clean web install, lint, check (zero errors/warnings),
  35 tests and build; four Compose variants; diff and release metadata checks.
- Passed locally: migration ordering/immutability, 2,257 matching EN/DE keys and
  placeholders, 15 browser suites, 39 unique ADR files and valid index links.
  Original provisioning hashes and both reproducible generators passed.
- Manual integration review: tenant/role checks, sender identity revalidation,
  uncertain-result retry rules, bounded adapters, verified mailbox connection,
  attachment filtering, fax storage/backup and API/UI contracts.
- Source CI39161227697, E2E39161227628 and FreeSWITCH39161227565 passed on source head.
- Final candidate 4716681c5aaf3b581a046d7dfa9fa8b71dbca937 passed all 16 checks:
  PR CI39206930642, E2E39206930647, FreeSWITCH39206930576; duplicate push
  CI39206925728, E2E39206925734, FreeSWITCH39206925645 also passed.
- Exact-main CI39208652398, E2E39208652451 and FreeSWITCH39208652404 passed before tag.
  Full Linux Rust: 401 passed, one existing ignored Whisper/model test. Web: 35 tests
  and all 15 browser suites. Ten Python MIME, IMAPS and document fixtures passed.
  Database/LDAP, backup/restore, Compose and server/worker image smoke checks passed.
  G.711 and required negotiated T.38 multi-page fax transfers passed; received PDFs
  checked. Existing SIPp and browser softphone scenarios also passed.
- Exact-main smoke tests report talkops 1.8.4 and talkops-media-worker 1.8.4.
- Release39210410306, tag E2E39210410301 and FreeSWITCH39210410212 passed.
- Additional tag CI39210410441 passed on the exact release commit. No gate failed; no rerun or waiver.
- CodeRabbit CLI unavailable. No CodeRabbit review ran.
- Not run locally: full Linux services, Python conversion fixtures, Docker/SIPp/fax
  stack. No Linux Docker daemon is available. The complete checks used GitHub CI.
- STE review: repository clarity rules applied. Full official standard unavailable;
  full language review remains pending, separate from functional release gates.

## Publication

- Final main/release commit: 488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d.
- Main tree equals the tested candidate. Source ancestry, latest main gates,
  unused tag, empty open scope and versions were checked immediately before push.
- PR54 merged 2026-10-19T06:13:12Z; PR53 auto-merged through it one second later.
- Annotated tag v1.8.4 object: a1d8d158746063a502dcec0ae9adba225d66c0be.
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.4
- Published 2026-10-19T06:38:59Z. Latest stable, not draft or prerelease.
- The Release workflow built six images and published three indexes. Each index
  has linux/amd64 and linux/arm64. All six OCI configurations report v1.8.4 and
  the exact release revision. Moving 1.8/latest platform digests match 1.8.4.
  English release notes include the source, upgrade steps and deployment limits.

## Artifact evidence

| Image | Architecture | Published platform digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:1856d688849a6e7eba2161c6df5ade3ef1354700e56996af781fb9b1c798e1d2 |
| talkops-server | arm64 | sha256:067be65e767b2fa032e53148ac734bad07aeed333dd20ba5f05f4d7f3a5c6a83 |
| talkops-media-worker | amd64 | sha256:5d14b1ee130213c735bc93ebd7da6d9e5da808fa577c44a5b38f11d11a818b0d |
| talkops-media-worker | arm64 | sha256:7d24814df86770dd413dc15acd2d492edbe9aa5735d11bed46e3b4df6ed92713 |
| talkops-freeswitch | amd64 | sha256:7bb918fe523130d3d63a072086fd97eadfbe42c4f2d8c79b122d4d8756e33269 |
| talkops-freeswitch | arm64 | sha256:95d1536df753f81bd66663fdbc8a15480d45071fe82d02ce276715e207309849 |

## Cleanup

The unchanged integrated PR53 and owned release branches were removed in one
atomic push after publication, ancestry and artifact checks. Expected-head guards
prevented deletion of changed branches. No history was rewritten. Remote branches
left: main and closed, unmerged PR36's claude/vigilant-tesla-ef0027. Foreign local
worktrees and branches remain intact. Save the final record through audit/release-1.8.4.
Do not push the removed batch branch again. Only this record differs from release main.

## Dependencies and open work

No missing handoff or software release blocker remains. Back up before upgrading.
Apply 20261106000001 then 20261106000002 before the new binary. Do not downgrade
once fax routes are assigned. Deploy updated Compose with the shared fax volume
on server, worker and FreeSWITCH. Keep images together and reload the UI.
Server adds Python; worker adds Python, Pillow, ReportLab, Poppler and fonts.
Configure fax permissions, numbers and dedicated destinations. Retention defaults
to 30 days for each direction. Referenced users/numbers cannot be hard-deleted;
remove fax references and reassign destination ownership first.
Mail input needs outgoing SMTP, verified IMAPS and mail-server enforcement of
authenticated sender identities. A From header alone does not authenticate a sender.
Real provider interoperability and deployment mail delivery remain operator acceptance
checks. Existing hardware limitations from 1.8.3 remain unchanged.

## Next action

None. Release, artifact verification and branch cleanup are complete.

## References

- [Source PR53](https://github.com/itsh-neumeier/talkops/pull/53).
- [Integration PR54](https://github.com/itsh-neumeier/talkops/pull/54).
- docs/ai-memory/20261018-codex-fax-sip-proposal.md.
- docs/ai-memory/20261017-codex-release-1.8.3.md.
- docs/adr/0040-fax-over-sip-and-mail-input.md; docs/release-workflow.md.

## Final workflow evidence

- [Candidate CI](https://github.com/itsh-neumeier/talkops/actions/runs/39206930642),
  [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39206930647),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/39206930576).
- [Exact-main CI](https://github.com/itsh-neumeier/talkops/actions/runs/39208652398),
  [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39208652451),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/39208652404).
- [Release](https://github.com/itsh-neumeier/talkops/actions/runs/39210410306),
  [tag CI](https://github.com/itsh-neumeier/talkops/actions/runs/39210410441),
  [tag E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39210410301),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/39210410212).
