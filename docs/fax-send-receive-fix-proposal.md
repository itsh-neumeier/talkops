# Fax send and receive repair: proposal

Status: Approved by the user on 2026-10-19. Implementation and verification in progress.
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

## Approved repair scope

The user approved implementation and assigned spam exceptions and blocking
changes to a separate PR. Keep those changes out of this branch.

- Check actual server write access with a unique temporary file. Check the
  existing 100 MiB free-space requirement. Remove the probe after the check.
- Check storage before creating or queueing an uploaded fax. Map filesystem
  errors during upload to safe fax error responses. Remove incomplete uploads.
- Return `fax_storage_unavailable` (HTTP 503) or `fax_storage_full` (HTTP 507).
  Log the failed operation and path on the server. Do not expose internal paths
  or operating-system details in the API response.
- Show translated error messages. Keep the attachments so the user can retry
  after correcting the stack. Keep preview and permission checks unchanged.
- Add a fax storage check to the existing admin settings. State that this
  checks server access only. The worker and FreeSWITCH still need the same mount.
- Keep the correct production Compose mounts. Document how to repair an old
  deployed stack without disabling its read-only root filesystem or replacing
  existing fax data. No production settings were changed from this chat.
- Document the confirmed PhoneBlock rejection. Do not change spam code, its
  settings, exceptions, or call-list presentation. The separate PR owns these.
- Do not add routing warnings or change T.38 settings as part of this repair.
  The supplied logs do not establish a route or transport defect.

## Verification contract

Add API/database tests for rejected uploads, safe errors, permission checks,
cleanup, and recovery after a storage correction. Cover a non-writable directory
on Unix and an invalid mount path on all platforms. Cover low free space and
probe cleanup in storage unit tests.

Extend the browser fixture to test both error codes, retained attachments,
retry to preview, German and English text, admin status, and a repeated check.
Add image smoke tests for a read-only root without a fax mount and with a
writable fax mount. The media E2E fixture checks the storage API before its
existing G.711 and T.38 round trips.

Run the relevant Rust, web, Python, shell, Compose, image, and media checks.
Report local service limitations separately. A real provider test still needs
an operator after the deployment storage and spam decisions are corrected.

## Dependencies and release handoff

No new dependency or migration is planned. Do not edit released migrations
`20261106000001` or `20261106000002`. The spam PR is a separate contribution;
this storage repair has no code dependency on it. Receiving the previously
blocked test fax depends on resolving that blocking decision operationally.

Update draft PR 55 with the final implementation and check results. The
release chat owns merges, version changes, tags, and publication. This task
must not publish a release or claim that the live deployment was repaired.

Suggested release notes after verification:

> Report fax storage failures with actionable messages. Add an administrator
> storage check and document shared-volume repairs after stack upgrades.

Language review uses the repository clear-English rules. The official
ASD-STE100 standard was unavailable. Full language review remains pending.
