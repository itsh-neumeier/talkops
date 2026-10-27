# Coral base synchronization with TalkOps 1.8.11

## Task state

- Date: 2026-10-27
- Tool: Codex
- Status: Base synchronized; CI and user-owned production trial remain external
- Branch: codex/coral-base-readiness, published to claude/lucid-wright-lhtgr1
- Base commit: 3d11790618ed99c70fc5b7864c5be363e8a969d7
- Related PR: https://github.com/itsh-neumeier/talkops/pull/81

## Result

Merged main at f8bdfb3 without conflict. The merge commit is d92a5e0.
Aligned the Coral Cargo.lock package metadata with the inherited 1.8.11
workspace version. No new release version or tag was created.

The separate feature PR #82 contains the UI complete-word setting,
time-limited diagnostic capture, credential/text redaction, a JSON support
report and isolated trial-image publication. It depends on this base.
Its combined local checks passed workspace Clippy, targeted SQL/API/worker
tests, frontend checks, browser tests, Compose and documentation checks.
GitHub checks determine readiness of the published base commit itself.

The user will test the real Rust service on the production host. The
hardware/quality gate remains pending. No production service was changed.
Ready for review does not mean readability or production quality is approved.

## Next action

Review #81 and #82 together. Use the trial guide and report export from #82
for the user-owned production test. Do not mark the quality gate passed
without the test result. No merge, release or deployment was performed.

STE review: repository writing rules used; full standard unavailable,
full language review pending.

## CI overload-test correction

Feature push CI 40461728658 failed the pre-existing overload test. The
four HTTP requests did not saturate the queue before the fixed 300 ms mock
delay elapsed. The assertion depended on runner speed. Replace this timing
assumption with a gated inference: hold one active job, wait for one queued
job, then assert that the HTTP request returns 503 with Retry-After. Release
the gate and drain both jobs. Runtime queue behavior is unchanged.
The combined feature branch passed Clippy and all seven service tests with
a two-CPU quota after this correction. Apply the same focused test fix to
the base PR so both branches use the deterministic overload assertion.
