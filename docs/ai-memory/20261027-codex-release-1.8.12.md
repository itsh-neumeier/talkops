# TalkOps 1.8.12 release

## Task state

- Task ID: 20261027-codex-release-1.8.12
- Date: 2026-10-27, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.12 (merged and removed)
- Base commit: f8bdfb37cf3f7705d7fbbaf63d6bd6148caba400
- Related PR: https://github.com/itsh-neumeier/talkops/pull/85

## Goal and scope

Publish all agreed ready contributions as user-selected 1.8.12. Preserve source
history, previous release audit, foreign worktrees and unfinished contributions.

| PR | Contributor | Exact source head | Dependency and result |
| --- | --- | --- | --- |
| 81 | Claude Code, Codex base corrections | 5a1a53131005f6b28f0619ef0f776da670b3b8de | Coral service; included |
| 82 | Codex | ab7d13724f3e6b34616a409c6bb412176dee34e7 | Coral UI/diagnostics; depends on 81; included |
| 84 | Codex | 93ffc93bb39caf25781ed04cab5b096c5d8836a5 | Map previews/emergency mail; independent; included |

No selected source head was already in main. Draft PR61 remains excluded at
16c6c45a61655a9ff215c350ac47d499d15dae9f (review-only Wi-Fi proposal).
No missing agreed ready handoff. None of the three contributions is silently omitted.

## Decisions and integration

- PR81/82 are not drafts, but their handoffs only approve review/opt-in trial.
  The release manager asked about stable scope. The user explicitly selected
  all three PRs, with Coral marked experimental. This supersedes the prior
  draft exclusion. Physical hardware/readability/quality acceptance is still
  pending. Automated checks and publication do not pass that gate.
- User selected 1.8.12 for this additive scope. No published API is removed.
  New optional Coral service/profile stays off; local Whisper stays the default.
- Target main verified through GitHub API; merge commits allowed. No rulesets
  or branch protection at inspection. v1.8.12 was unused; latest was 1.8.11.
- Merge prior audit/release-1.8.11, then 81, 82, 84 without rewriting sources.
  Resolve only the ADR index conflict by retaining both 0046 and 0047.
- Mark Coral experimental in EN/DE connection badges/hints and setup guides.
- Supply TALKOPS_BUILD_REVISION from github.sha in CI and Release image builds.
  Source Dockerfiles otherwise default diagnostic revisions to unknown.
- Run existing Coral library/mock smoke on both native Release architectures,
  before manifest publication. No hardware result is implied.
- Version Cargo workspace, eight workspace lock entries and npm roots through
  package tools. No additional dependency upgrades beyond the source PRs.
  Offline Cargo metadata update initially failed on the uncached edgefirst-tflite
  index entry; online workspace-only update succeeded and version-only checks passed.

## Checks

- Passed locally: cargo fmt; locale formatting; 86 web unit tests; 13 docs tests;
  Wiki 75 pages/688 links; provisioning blob hashes and both derivative checks.
- Five Compose variants with coral profile passed: base, dev, backup, targets,
  isolated trial. Normal profile remains opt-in. No live credentials used.
- Static: 59 unique ordered migrations; all 56 released SQL files unchanged.
  2,825 matching EN/DE keys/placeholders; 25 unique browser registrations;
  46 unique ADR files and 47 valid index links. Four Release image manifests.
- Metadata check proves only eight package versions and npm roots changed
  from the combined source baseline; no unintended dependency upgrades.
- Manual review: diagnostics permissions/tenant scope/explicit text consent,
  redaction and bounded capture; loopback Coral contract and local fallback;
  map grouping/legacy aliases; emergency event before routing lookup, bounded
  best-effort insert, UUID deduplication and unchanged ordinary notification flow.
- Final CI passed full Rust/DB/LDAP, clean npm/lint/check/build, image/runtime
  smoke, all 25 browser suites and SIPp/fax/softphone. No local Linux Docker daemon.
  C: had about 290 MB free. Automatic approval review blocked removal of this
  worktree's ignored target directory with "blocked by policy". The complete
  command was rejected before execution; nothing was deleted. No bypass/retry.
  Keep foreign build caches and worktrees untouched; use CI for heavy builds.
- CodeRabbit CLI unavailable; manual integration review used. Repository clarity
  rules applied; full STE standard unavailable, full language review pending.

## Migrations, compatibility and open work

Apply in order: 20261112000001_coral_transcription,
20261112000002_coral_stable_preview, 20261113000001_emergency_dial_alert.
Back up first. Update server/worker and enabled Coral together. Old-binary rollback
needs the matching pre-upgrade DB backup. No model weights shipped; model license
and hardware/readability acceptance remain pending with the user-owned trial.
No production host, USB/PCIe devices or live Wiki were changed.

Emergency alerts are opt-in and report dialing attempts, including failed routes.
SMTP is asynchronous/best effort; no retry or exactly-once guarantee. Ordinary
notification rules and old country-map filter links retain compatibility.

Keep draft61 and unmerged branches (claude/nifty-babbage-ymchlw,
claude/charming-bell-fh6q3y, claude/vigilant-tesla-ef0027). Original primary
checkout stays at 397fc83 with three untracked Dahua summaries.

## Next action

None for this release. User-owned Coral hardware/readability/quality and model-license
acceptance, draft PR61 and live Wiki publication remain separate work.
The task-only final record is archived under audit/release-1.8.12.

## References

- docs/ai-memory/20261024-claude-coral-stt.md
- docs/ai-memory/20261027-codex-coral-base-sync.md
- docs/ai-memory/20261026-codex-coral-production-readiness.md
- docs/ai-memory/20261026-codex-coral-stable-preview-feature.md
- docs/ai-memory/20261027-codex-map-emergency-proposal.md
- docs/ai-memory/20261026-codex-release-1.8.11.md
- docs/adr/0046-coral-edge-tpu-speech-recognition.md
- docs/adr/0047-call-map-device-markers-and-emergency-alerts.md
- docs/research/coral-production-trial.md
- docs/release-workflow.md

## Candidate correction

Initial candidate 5680cd75a1c789f1d5a439deb3fe6654a970eafe is superseded.
PR CI40508739179 web job122204728574 failed at coral-transcription.mjs:73.
The release manager changed the badge to Coral (experimentell) but did not
update the existing exact-text assertion. Match the exact experimental badge;
keep all connection, runtime, fallback and permission assertions unchanged.
No product behavior changed by this correction. New complete CI/E2E passed.
Clean local npm ci, lint, check (zero errors/warnings) and build also passed.
All source PR checks have now passed; PR84 E2E finished successfully.
## Native architecture gate

Add a native Coral arm64 row to the existing CI image matrix. The source trial
images only proved amd64. This new fourth image must build and pass its existing
library/mock smoke on arm64 before any release tag is pushed. Existing server,
worker and Coral amd64 checks stay. All image jobs pass their explicit platform
and exact build revision; per-image/per-architecture cache scopes stay separate.
The normal Release build also checks its actual Coral digest on both platforms.
No physical TPU, quality or deployment claim. Candidate e7fb60c is superseded
only by this stronger CI/release validation contract and task record.
## Final integration and publication

- Final candidate: b6c6f6b225bf6712e91138617dbdf718a1673b29. All 22 required checks passed; two branch-only
  trial image contexts skipped as designed. PR CI40510106255/E2E40510106360;
  push CI40510095619/E2E40510095618. The superseded badge expectation failure
  was corrected and is recorded above. No assertion removed or gate waived.
- PR82 retargeted to main with its head unchanged. PR85 and all three agreed
  source PRs merged. Exact-main target: 3603ed56b0073a1dd23646622cf4fb0284644ab7. Its tree equals the candidate;
  all recorded source heads are ancestors. Only unchanged draft PR61 remained.
- Exact-main CI40514888617 and E2E40514888443 passed before tag push.
  Rust 558 passed, two model/runtime tests ignored. Web 86 tests/25 browser suites.
  Docs 13 tests/75 pages/688 links; Coral tools 25 tests. Native Coral library/mock
  smoke passed on amd64/arm64. Server, worker and Coral displayed 1.8.12.
  DB/LDAP, recovery, image/storage smoke, received G.711/T.38 fax PDFs and
  SIPp/browser softphone passed. No physical TPU or text-quality result implied.
- Annotated v1.8.12 pushed once on this exact target. Tag object: 221b72a2113129530f14b1e59594fb504682fff1.
  No published tag moved. Release workflow 40518594652 passed, including the
  actual Coral digest's native library/mock smoke on both architectures.
- Stable GitHub release published at 2026-10-27T14:19:05Z. Additional tag CI 40518594859,
  E2E 40518595237 and FreeSWITCH 40518594647 passed. Build revisions supplied from github.sha.
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.12

## Upgrade

Back up first. Apply 20261112000001_coral_transcription,
20261112000002_coral_stable_preview and 20261113000001_emergency_dial_alert
in order. Update server/worker and enabled Coral together, then reload the UI.
Old-binary rollback needs the matching pre-upgrade DB backup. Coral is off by
default and remains experimental; hardware/quality and model-license acceptance
are separate user-owned work. No models are shipped.

## Registry verification

Docker buildx imagetools verified FOUR versioned manifests and eight platform
configs. Every OCI version label equals v1.8.12; revision labels equal the target.
Tags 1.8/latest have the same platform digests as 1.8.12.
Coral remains explicitly experimental, opt-in and without model weights.

| Image | Platform | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:d2e2b02dd21c04dbc951601581f41cd1e4ba2cb6d2073fb20da16696755541fd |
| talkops-server | arm64 | sha256:8f8a35558e9caf926642ebe9d212fd408e183be910ed1173becb7ef4133bbaf4 |
| talkops-media-worker | amd64 | sha256:b5284872b3baf588048586d4ae0075a2d29a1efec39c4aa5a4382f0313bbc4db |
| talkops-media-worker | arm64 | sha256:ed270b09b0d9575fd498a4c9094c3fad730248ba534a04f3cba0a0ea04e2cdc9 |
| talkops-freeswitch | amd64 | sha256:160aaac3e5c3dd2ef912d96ee04e6e00fa0ebb3cec0c361cb2634914a5e34e60 |
| talkops-freeswitch | arm64 | sha256:5994ffb19cfd3ffe0d13f881618f0c9a736e995e735cfc04dd5d133da9570c28 |
| talkops-coral-stt | amd64 | sha256:9496336c6f1c9be2a99bbd2df410c5bed99744767ac047491f430148b73855fe |
| talkops-coral-stt | arm64 | sha256:ea936a84cf758a46395300bf7348dc1e39fe9655429bce98b1db6c2a1ea74bf0 |

## Cleanup and audit

Removed the three unchanged merged source branches and codex/release-1.8.12
with atomic expected-head guards. Preserve draft61, claude/nifty-babbage-ymchlw,
claude/charming-bell-fh6q3y and closed unmerged PR36 branch
claude/vigilant-tesla-ef0027. Foreign worktrees and primary files untouched.
Only this task record differs from the released commit in the separate audit.
Source PR82 trial-guide and PR84 task-record links use immutable source commits, so
they remain available after branch removal. Do not push the merged release branch
again. No production service or device changed.
