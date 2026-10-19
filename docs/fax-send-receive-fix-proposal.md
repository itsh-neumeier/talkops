# Fax send and receive repair: proposal

Status: Pending user review. No implementation changes are approved yet.
Date: 2026-10-19 (Europe/Berlin).
Base: `origin/main`, `488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d`.

## Reported behavior

- The web send form shows `internal server error` with an image attached.
- An external fax to the receiving number fails. TalkOps shows an incoming
  call with no duration and a busy/rejected result. The fax inbox is empty.

The screenshots show symptoms. They do not identify the failed HTTP request,
the SIP response, the configured destination, or the server error.

## Deployment evidence supplied after the first proposal

The user confirms the stack and fax route are configured. The settings image
shows fax enabled, a global sender number, and T.38 preferred with audio fallback.
Treat an absent incoming route as a diagnostic possibility, not an established
cause. Do not copy private numbers, hosts, or complete logs into this PR.

- The supplied startup log confirms TalkOps `1.8.4` and an up-to-date schema.
- Three HTTP 500 events accompany `Read-only file system (os error 30)` at
  09:01:55, 09:01:58, and 09:18:26 UTC on 2026-10-19. In the reported upload
  context, this identifies filesystem access as the first repair target.
  The generic error does not contain the path or HTTP route. The exact failing
  mount or path must still be checked in the running stack.
- The incoming test call at 08:59:22 UTC is explicitly blocked with reason
  `phoneblock:4`. Its call record has zero duration and `CALL_REJECTED`.
  The code rejects blocked callers with SIP `603 Decline` before it starts
  the fax receiver. This explains the reported busy/rejected call for this test.
- `4` is the number of PhoneBlock reports returned by the check. This result
  meets the configured blocking threshold. It is not a fax protocol error.
- Initial FreeSWITCH connection refusals end with a successful connection at
  08:56:17 UTC, before the test call. These startup messages do not establish
  an ongoing FreeSWITCH outage.

The logs do not establish a T.38 negotiation or document conversion failure.
The filesystem failure and PhoneBlock rejection must be resolved before a
provider transport test can assess those later stages.

## Verified code and release state

- Remote HEAD points to `main`. PR 53 is merged into the 1.8.4 release line.
- The upload handler writes attachments before it queues document preparation.
  Directory, write, free-space query, and database errors can return HTTP 500.
  The API hides internal details and logs them on the server.
- The send page uses the same error box for initial load, periodic history
  refresh, upload, and submit. The screenshot alone cannot identify the step.
- Conversion runs in the media worker after the upload. A conversion failure
  sets the fax state to `failed`. It normally does not fail the upload request.
- The production Compose file mounts one shared `fax` volume in the API,
  worker, and FreeSWITCH. The API and worker have read-only root filesystems.
  Updating images without updating an older stack definition can leave the
  fax directory unwritable. An absent mount remains a hypothesis; the supplied
  logs prove a read-only filesystem error, not which mount is missing.
- Incoming fax requires an enabled fax destination and an assigned incoming
  number. A global outgoing fax number does not create an incoming route.
- The dialplan rejects missing or disabled destinations. Spam blocking also
  applies before the receiver starts. The fax handler can fail before answer.
- Current Fax settings list destinations, but do not show their assigned
  incoming numbers or a fax storage readiness result.
- The earlier task records passing G.711 and T.38 round trips in CI. Those
  results do not prove compatibility with the reported provider or deployment.

## Proposed work after approval

### 1. Establish the failed stage

Confirm the installed version and the deployed stack definition. Identify
the failed HTTP request and its server log entry. For reception, inspect the
number destination, enabled state, call UUID, SIP result, and fax handler log.
Check shared mounts and UID/GID 10001 access in all three services.

Use this evidence to select the repair. A deployment problem needs a stack
or routing correction. Change transmission code only when evidence shows
a transmission defect. Keep live configuration changes subject to the agreed
deployment scope. This development task has no production access configured.

Prioritize inspection of the effective fax path and its mounts. The same
writable volume must be mounted in all three services at the configured fax
path. Prepare the exact stack correction from the deployed definition. Keep
the existing read-only root filesystem and persistent fax data.

For the incoming test, first review the PhoneBlock decision. The current code
has a tenant-wide PhoneBlock switch and report threshold, but no per-caller
allowlist. A controlled retest can temporarily disable PhoneBlock, save its
configuration, repeat the incoming fax, and restore the original setting.
Do not disable protection or alter its threshold automatically. A permanent
caller allowlist or per-fax exemption would be a separate product decision.

### 2. Make send failures actionable

- Add a bounded fax storage check that can distinguish an unwritable
  directory from insufficient free space. Clean up temporary probe files.
- Return stable fax error codes with safe German and English UI messages.
  Keep SQL details, secrets, and internal paths in protected server logs.
- Identify upload, preparation, and send failures separately in the UI.
  Keep the attachment selection available after a failed preparation request.
- Keep the preview requirement and sender permission checks.

Likely files: `routes/fax.rs`, fax storage helpers, the fax page, shared
fax types, and both translation files. Do not expose arbitrary internal
errors through a global API error change.

### 3. Make reception configuration and failures visible

- Explain that PhoneBlock also applies to incoming fax. Make the blocking
  reason visible in the relevant call details. Reuse the stored
  `talkops_blocked` result. Check existing presentation before adding UI.
- Keep the spam policy unchanged in this repair. A blocked call must remain
  distinct from a started fax receiver that fails during reception.
- Show the assigned incoming numbers for each fax destination in Fax settings.
  Show a warning when an enabled destination has no enabled incoming number.
  Link to number configuration. Do not reassign voice numbers automatically.
- Add structured reasons for fax route rejection and failures before answer.
  Include the call UUID where available so an operator can find the call.
- If a valid fax destination was resolved and the receiver then fails, keep
  a failed fax record with a safe reason when the database is available.
  A database outage must still produce a correlated server log.
- Check number matching and routing with the reported provider's call data.
  Add a provider-specific repair only if that data proves it is necessary.

Likely files: the public dialplan, `talkops-api/src/fax.rs`, Fax settings,
and database/API regression tests. Automatic voice/fax detection remains
outside this proposal, as specified by ADR 0040.

### 4. Verify and document the repair

- API tests: image upload queues preparation; storage failures have safe
  errors; rejected uploads leave no partial document or queued job.
- Routing tests: active fax number starts the receiver; missing, disabled,
  and blocked routes give the expected result; voice routes keep their behavior.
- Reproduce the observed PhoneBlock threshold result with a local test server.
  Check that a blocked fax route produces `603 Decline` without starting the
  receiver, and that disabling PhoneBlock permits the configured fax route.
- Receiver tests: failure before answer is traceable; failed fax records
  remain tenant-scoped and use the existing inbox access rules.
- Browser tests: actionable errors, retained attachments, and number warnings.
- Extend image smoke checks to exercise fax directory access as the service
  user. Verify shared storage and conversion in the stack E2E fixture.
- Run the relevant Rust, web, Python, Compose, and fax media checks from CI.
  Report local tool or service limitations separately.
- Add English and German troubleshooting steps for stack upgrades, shared
  mounts, incoming routes, and useful logs. Record a task summary and PR handoff.
- Verify one outbound and one inbound fax with the operator's real provider
  after deployment. CI peers alone cannot close provider acceptance.

## Review and release boundary

This PR contains the proposal and task summary only. Wait for user feedback
before changing runtime code, tests, deployment files, or live settings.
Commit, push, and draft PR creation are authorized by the user's task prompt.
The release chat owns merges, version changes, tags, and publication.

No new dependency or migration is planned. Reassess this if runtime evidence
requires a schema change. Do not edit released migrations `20261106000001`
or `20261106000002`.

Suggested release notes after implementation and verification:

> Improve fax upload error messages and incoming fax diagnostics. Show missing
> incoming number assignments. Document shared fax storage and PhoneBlock
> behavior for incoming fax calls.

Language review uses the repository clear-English rules. The official
ASD-STE100 standard was unavailable. Full language review remains pending.
