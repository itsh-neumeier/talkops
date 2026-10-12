# Active call overview proposal

## Task state

- Task ID: 20261012-codex-active-call-overview
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: In progress. Implementation complete. Final validation pending.
- Branch: codex/active-call-overview
- Base commit: 4503fb3b96167aa77320cc936597fc6147b392c3
- Related commits or PR: Pending

## Goal

Improve the dashboard display for active calls. Match both parties against
all phonebook entries. Compare national and international number formats.
Show two lines on the left. Keep call status and duration on the right.
Assess an optional live transcript popup with speaker labels, timestamps,
and a typing animation. The user requires proposal review before code changes.

## Result

Read the project guidance, release workflow, task memory, relevant ADRs,
and current dashboard, routing, phonebook, and transcription code.
Verified the remote target and fetched its current commit. The original
checkout was clean. Created a separate worktree and feature branch.
Prepared a local interactive preview with example data outside the repository.
The user approved the proposal and specified square brackets for all numbers.
Implemented the overview and an on-demand local live transcript preview.
Application commit 1562aa4 is pushed. Draft PR 7 is open:
https://github.com/itsh-neumeier/talkops/pull/7.

The task worktree is at
`C:/Users/timo/.codex/worktrees/active-call-overview/talkops`.
App worktree registration failed. Git can use this checkout after an exact
safe.directory exception. Do not create a second checkout for this task.

## Changes

- API: Resolve tenant calls from routing variables. Match all contact fields
  and sections. Return structured caller/called parties and answer timestamps.
- Core: Add bounded, temporary preview jobs with expiry, isolation, and cursor
  compare-and-set. Add migration 20261026000003_live_transcript_previews.
- Worker: Add a separate live lane. Read bounded growing WAV snapshots and
  transcribe locally with base, at most two threads, and a 60-second timeout.
- Web: Add two-line call rows, square brackets, status/time, a native dialog,
  speaker labels, speech times, and typing with reduced-motion support.
- Tests: Add directory, authorization, database, WAV, and browser checks.
  Extend E2E to transcribe real Piper speech during a recorded SIPp call.
- Documentation: Add ADR 0022 and English/German active-call guides.
- This summary: Save implementation and validation state.
- Conversation preview: Show inbound and outbound rows and a transcript demo.
  The preview does not connect to TalkOps or contain real transcript data.

## Decisions

The user approved the proposal. Use square brackets for every phone number.
Keep round brackets in contact names, such as Family Vollmeier (Maria).

- Enrich active calls in the API. Read TalkOps channel variables for tenant,
  direction, extension, trunk number, and destination. Do not infer direction
  only from FreeSWITCH channel direction or formatted caller ID.
- Read contacts once per refresh. Match all sections and all three number
  fields. Reuse tenant dialing settings for 0, +49, 0049, and provider formats.
  Check internal identities before external normalization. Keep service codes
  and anonymous numbers separate. Do not turn an extension into a local number.
- Prefer the internal extension identity for the local party. Use the actual
  call route number for the external number shown beside that extension.
  Handle duplicate contact matches without selecting an arbitrary person.
- Show direction and the local party in the first line. Show the remote party
  in the second line. Use full names and readable wrapping on narrow screens.
  Keep the status marker and duration on the right. Distinguish ringing,
  talking, held, parked, and system calls. Define talk time from answer time.
- Live transcription needs new backend work. Current jobs start after the CDR.
  Assess bounded stereo audio snapshots during authorized recorded calls.
  Use a separate bounded worker lane and local whisper.cpp by default.
  Send timestamped segments to an authenticated popup. Show recognition delay
  and unavailable states. Use existing content access rules and audit access.
  Do not start recording from a popup click. Keep final transcription intact.
- Animate only newly received text. Keep speaker labels and speech timestamps
  stable. Respect reduced motion. Stop subscriptions when the popup closes.
- Implement both reviewed scopes on this feature branch. Keep final transcripts
  separate from temporary live text. The preview begins at recent audio.

## Checks

- Passed: Remote HEAD lookup, git fetch origin, target commit lookup, original
  git status, worktree inventory, and feature branch creation.
- Passed: Open PR inventory with the GitHub connector. PRs 3, 4, and 5 are open.
- Passed: Preview browser checks with headless Edge. Both popup buttons work.
  Speaker labels change with direction. Typing completes without script errors.
  Escape and Close return to the overview. Reduced motion shows full text
  immediately. The overview and popup fit a 320-pixel viewport.
- Failed: App worktree registration and attachment. The checkout remains usable.
- Failed then corrected: Preview runner imports and the default Edge path were
  unavailable. Used the bundled CommonJS module and installed system Edge.
- Passed after approval: Rust API/worker check; three active-call unit tests;
  three core preview tests; active-call and existing dashboard API tests;
  two growing-WAV tests; web lint/check/build and browser fixtures.
- Passed: Local clippy with -D warnings -A clippy::nonminimal_bool. The single
  exclusion covers an existing identity.rs warning under MSVC Rust 1.94.1.
  CI must run full workspace clippy without that exclusion.
- Failed then corrected: API audit assertion omitted the earlier operator
  access. Updated the expected count; repeated polling does not duplicate it.
- Failed then corrected: Existing Piper test helpers used Unix permissions on
  Windows. Apply the existing Unix test scope to that helper module.
- Failed then corrected: Web lint reported Windows CRLF checkout files.
  Normalize local web text to LF without unrelated Git content changes.
- Passed: Final browser animation check. New segments appear in steps and do
  not replay on refresh. Reduced motion shows complete text immediately.
- Failed then corrected: The local preview server kept the previous build
  manifest. Restarted the task server after rebuilding; browser checks pass.
- Manual review: Checked tenant filtering, phonebook ambiguity, content access,
  job bounds, cleanup, growing WAV input, subprocess cancellation, and i18n.
- CodeRabbit CLI: Unavailable. Manual review completed.
- Pending: Linux CI/E2E.
- Final review: Remove old completed live jobs as well as abandoned previews.
  This prevents continuous popup polling from growing the jobs table.
- Stop the open preview when its remote party changes after a transfer.
  This keeps the initial speaker names separate from the new party.
- Share the existing phonebook name formatter. Include company/name suffixes
  in round brackets. Test a separate name and company that produce the example.
- Remote browser fixture: Wait for the incoming preview response before
  asserting its speaker labels. Keep all assertions and the typing check.
- First live-stack E2E found that the bridge caller ID can identify the
  originating extension. Use the peer's dialed destination or routing target
  for the called party. Add a regression test and repeat live-stack E2E.
- Failed remote web check: The existing mobile header exceeded 320 pixels with
  Linux fonts. Keep branding text on larger screens so mobile controls fit.
  Repeat the full browser check on Linux. No assertion was removed.
- STE review: Used repository writing rules. The full official standard was
  unavailable. Full language review is pending.

## Dependencies and open work

- No new package dependency. The local base model needs about 148 MB on disk.
- Migration ID: 20261026000003_live_transcript_previews. PR 3 uses ID 2 and
  the released voicebox work uses ID 1. Update server and worker together.
- PR 5 changes explicit internal dialing to **101. Preserve that distinction
  during contact matching. Do not map +499543101 to extension 101 by suffix.
- PRs 3, 4, and 5 overlap shared API or translation files. Preserve their work
  during release integration. No hard PR dependency is established.
- Add regression tests for both call directions, device login identities,
  all phonebook sections, all number fields, format variants, duplicate matches,
  anonymous callers, tenant boundaries, transfers, states, and duration.
- For live work, test speaker mapping, partial WAV input, chunk order, retries,
  access rules, disconnects, call end, and worker load. Verify with real audio.
- Run web lint/check/build and relevant Rust checks and database tests.
  Use CI and SIPp/browser checks for the Linux stack. Record exact results.
- Commit, push, and PR creation are authorized after reviewed implementation.
  The release chat owns target merges, versions, tags, and publication.

## Next action

Finish validation and manual review. Commit and push the feature branch.
Create a draft PR with test results, dependencies, migration ID, upgrade
details, and proposed English release notes. Wait for Linux CI/E2E. Update
this summary and mark the PR ready when final checks pass.

## References

- ../release-workflow.md
- ../development-chat-prompt.md
- ../architecture.md
- ../adr/0004-frontend-sveltekit.md
- ../adr/0005-job-queue-postgres.md
- ../adr/0012-aufzeichnung-und-transkription.md
- ../adr/0019-transkription-in-zwei-durchgaengen-und-ki-api.md
- https://github.com/itsh-neumeier/talkops/pull/5
