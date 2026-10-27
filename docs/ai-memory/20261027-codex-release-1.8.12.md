# TalkOps 1.8.12 release

## Task state

- Task ID: 20261027-codex-release-1.8.12
- Date: 2026-10-27, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.12
- Base commit: f8bdfb37cf3f7705d7fbbaf63d6bd6148caba400
- Related PR: Pending

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
- Full Rust/DB/LDAP, clean npm/lint/check/build, image/runtime smoke, all 25
  browser suites and SIPp/fax/softphone use final CI. No local Linux Docker daemon.
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

Publish integration PR, wait for final checks, merge, verify exact-main CI/E2E,
then push unused v1.8.12 once. Verify Release, stable GitHub release, all four
amd64/arm64 manifests, version/revision labels, 1.8/latest and Coral mock runtime.
Save final audit and remove only unchanged fully merged batch branches.

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
No product behavior changed by this correction. New complete CI/E2E required.
Clean local npm ci, lint, check (zero errors/warnings) and build also passed.
All source PR checks have now passed; PR84 E2E finished successfully.