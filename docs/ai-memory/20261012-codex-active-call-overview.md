# Active call overview and live transcript preview

## Task state

- Task ID: 20261012-codex-active-call-overview
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: Completed implementation. PR readiness is recorded on PR 7.
- Branch: codex/active-call-overview
- Base commit: 4503fb3b96167aa77320cc936597fc6147b392c3
- Related commits or PR: [PR 7](https://github.com/itsh-neumeier/talkops/pull/7),
  application fixes 37f446c and d1e20f5

## Goal

Resolve the parties in active calls against extensions and all phonebooks.
Show two lines with status and talk time on the right. Add an optional live
transcript popup. The user approved the proposal before implementation and
specified square brackets for all phone numbers.

## Result

Implemented and published the changes on the feature branch in PR 7.
Names use the existing phonebook label, including round brackets for a suffix.
Numbers use square brackets. The popup transcribes recent audio locally during
an already recorded call. It shows provisional text, speakers, speech times,
and a typing animation. It supports reduced motion.

The exclusive task worktree is
`C:/Users/timo/.codex/worktrees/active-call-overview/talkops`.
App registration failed; Git works with an exact safe.directory exception.
The original checkout and other worktrees were preserved.

## Changes

- `crates/talkops-api/src/active_calls.rs`: Resolve routing metadata, both bridge
  legs, extensions, device logins, trunk numbers and contact format variants.
- `crates/talkops-api/src/routes/live_transcripts.rs`: Authenticate, authorize
  and audit preview access. Stop subscriptions when the viewer closes them.
- `crates/talkops-core/src/live_transcripts.rs`: Bound jobs and text, isolate
  viewers and tenants, expire subscriptions, and remove old preview jobs.
- `crates/talkops-media-worker/src/live.rs`: Read growing stereo WAV snapshots
  and run local recognition in a separate worker lane.
- `web/src/lib/components/`: Show two-line calls, status, time and transcript
  dialogs. Stop the old preview when a transfer changes the remote party.
- `tests/browser/active-calls.mjs`: Check display, brackets, animation, access
  states, subscription cleanup, transfers and a 320-pixel viewport.
- `tests/e2e/run.sh`: Check resolved call parties and real Piper speech in an
  active SIPp recording. Preserve the existing SIPp and softphone checks.
- `docs/adr/0022-active-call-identities-and-live-previews.md`: Record the design.
- `docs/en/active-calls.md` and `docs/de/laufende-gespraeche.md`: Document use.

## Decisions

- Resolve direction and ownership from tenant routing variables. A bridge
  caller ID can still identify the original caller. Resolve the target from
  the peer destination or the routing target instead.
- Match all contact sections and all three number fields. Use tenant dialing
  settings for national, international and provider formats. Resolve internal
  identities first; do not match an external number to an extension by suffix.
- Do not select an arbitrary contact when matches are ambiguous.
- Use the existing phonebook name formatter for labels such as
  `Familie Vollmeier (Maria)`. Put every phone number in square brackets.
- Limit previews to four per tenant, one pending job per viewer/call,
  eight seconds per snapshot and 200 segments. Work expires after 30 seconds
  without requests. Remove stale data and completed jobs after five minutes
  during requests or through hourly retention.
- Keep final transcripts separate. Do not enable recording through the popup.
  Animate only new segments and keep screen-reader text complete.

## Checks

- Passed locally: Four active-call unit tests, three core preview tests,
  active-call and existing dashboard API tests, and two growing-WAV tests.
- Passed locally: Web npm ci/lint/check/build, browser fixtures with Edge,
  Rust formatting, Git whitespace and shell syntax checks.
- Passed locally: Targeted clippy with `-D warnings -A clippy::nonminimal_bool`.
  The exception covers an existing identity.rs warning with MSVC Rust 1.94.1.
  Linux CI uses the full workspace command without that exception.
- Passed remotely: Full CI at 64580c4, including Rust, web, Compose and images.
- Passed remotely: Real SIPp/browser E2E at 37f446c, including Piper speech in
  the live popup. PR run 38291604177 and push run 38291596419 both succeeded.
- Failed then corrected: E2E exposed the bridge caller-ID target error.
  Added a regression test and repeated real E2E successfully.
- Failed then corrected: Clippy at 37f446c requested one collapsed condition.
  d1e20f5 applies that change; local clippy passes.
- Failed then corrected: Linux mobile header overflow and an asynchronous
  fixture assertion. The full browser checks then passed without weaker tests.
- Final revision: Consult PR 7 for its CI/E2E results and readiness. Keep it as
  a draft if any required check fails. The PR records the verified source head.
- Manual review: Checked routing, tenant boundaries, content access, job
  bounds, cleanup, growing audio input, subprocess cancellation and i18n.
- Not run: CodeRabbit CLI, which is unavailable. Manual review was completed.
  Linux Docker/SIPp checks ran in GitHub Actions instead of on Windows.
- STE review: Used repository rules. The full official standard was unavailable;
  full language review remains pending.

## Dependencies and open work

- Migration: 20261026000003_live_transcript_previews. Update server and worker
  together. No new application package or service dependency.
- The local base model needs about 148 MB and downloads on first use if absent.
  Preview recognition adds worker CPU/memory use. Text and times are provisional;
  snapshot boundaries can split words. The popup begins at recent audio.
- No hard dependency on another PR. PRs 3, 4 and 5 overlap API, translations,
  CI or phonebook code. Preserve their behavior during integration. This task
  respects PR 5's explicit internal **101 format. Migration IDs do not collide.
- Target merges, release versions, tags and publication belong to the release
  chat. No release metadata was changed and no other chat was messaged.

## Next action

The release chat reviews PR 7, integrates the agreed scope and runs combined
release checks. The PR contains migration details and proposed release notes.

## References

- [PR 7](https://github.com/itsh-neumeier/talkops/pull/7)
- [Successful real E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38291604177)
- [Earlier full CI](https://github.com/itsh-neumeier/talkops/actions/runs/38289743847)
- ../release-workflow.md
- ../adr/0022-active-call-identities-and-live-previews.md
