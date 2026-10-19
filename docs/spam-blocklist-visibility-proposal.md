# Blocked calls and local whitelist: proposal for user review

Status: Proposed. User feedback is required before implementation.
Date: 2026-10-19 (Europe/Berlin).
Base: `origin/main`, `488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d`.

## Goal and review boundary

Show why TalkOps blocked an inbound call in the call list and recent calls.
Let an authorized user put a blocked number on a local whitelist.

The user requested a proposal before application changes. This revision adds
only this proposal and a task summary. It does not change application code,
database migrations, permissions or call routing. Commit, push and a draft
PR are authorized. The release chat owns integration and publication.

## Verified baseline

- The remote default branch is `main`. The base contains release 1.8.4.
- The original checkout has one local commit and three untracked Dahua task
  summaries. This task preserves them and uses an exclusive worktree.
- `talkops-api/src/spam.rs` checks anonymous callers, the tenant's own block
  list and PhoneBlock. Own list entries can contain a number or a prefix.
- `fsxml/dialplan.rs` sets `talkops_blocked` before `603 Decline`. Values are
  `anonymous`, `list:<pattern>` or `phoneblock:<votes>`.
- `fsxml/cdr.rs` does not read this variable. The `cdr` table and `Cdr` type
  do not store a block reason. Loading a call cannot recover that reason.
- The call list classifies `CALL_REJECTED` as busy or declined. Recent calls
  classify every unanswered inbound call as missed. Dashboard statistics
  also count such calls as missed.
- Blocking happens before the dialplan sets a destination extension. The
  screenshots therefore fit the current behavior. They do not prove which
  blocking source caused that particular call.
- `blocking.manage` permits block list changes and number tests.
  `blocking.settings` permits settings changes. There is no call whitelist.
- ADR 0029 defines permissions on roles. Administrators have all permissions.
  Each request checks the current permissions. Call visibility has a
  separate scope: all calls or only calls attached to the user's extensions.

## Proposed interface

### Call list and recent calls

Use one shared result rule in both views. A stored block reason takes
priority over the general hangup cause.

| Stored source | Call list result | Recent calls state |
| --- | --- | --- |
| Own number or prefix list | `Blockiert · Sperrliste` | `Blockiert · Sperrliste` |
| PhoneBlock | `Blockiert · PhoneBlock` | `Blockiert · PhoneBlock` |
| Anonymous caller rule | `Blockiert · Anonym` | `Blockiert · Anonym` |
| No stored block reason | Existing result | Existing result |

Show a shield icon and a text badge with the existing theme colors. Do not
depend on color or a tooltip to explain the result. Show the matching list
pattern or PhoneBlock vote count in an accessible detail area. Keep the
technical hangup cause available as diagnostic information.

For a blocked call, show the dialed own number as the destination. Do not
describe a missing destination extension as an anonymous receiving party.
Do not imply that a phone rang or that an extension answered the call.

Add `Blockiert` to the result filter. Its server query, total count and
pagination must use the same rule as the visible result. New blocked calls
do not also match the busy or missed filters.

Example, with a fictitious number:

```text
Anrufliste
Zeit           Von             An             Ergebnis
19.10., 10:59  +493012345600    +493012345699  [Blockiert · PhoneBlock]
                                             [Zur Whitelist hinzufuegen]

Letzte Anrufe
Eingehender Anruf an +493012345699
von +493012345600
[Blockiert · PhoneBlock]  [Zur Whitelist hinzufuegen]
```

The real German button text uses `hinzufügen`. Keep number flags, phone book
names, existing loading states and narrow-screen layouts.

### Add a whitelist entry from a call

Show **Zur Whitelist hinzufügen** for a blocked inbound call when its caller
has a valid normalized international number and the user has the new
permission. Hide this control for anonymous calls and unauthorized users.

Open the shared modal. Show the exact number, an optional label, and this
effect: future inbound calls from this number bypass the local block list
and PhoneBlock for the tenant. The user confirms with **Nummer freigeben**.
Disable repeat submission while the request is pending. Retain the dialog
and its input if the request fails.

After success, show **Auf Whitelist** for that number in the loaded views.
Keep **Blockiert** and its original source on the historical call. A whitelist
entry changes future decisions; it does not change a past decision.
Creating an entry that already exists succeeds without a duplicate.

The whitelist is local to TalkOps. This action does not submit a report or
change a list in the user's PhoneBlock account.

### Manage the whitelist

Add a **Whitelist** area beside the block list in call blocking settings.
An authorized user can add an exact number, view its label and creation
time, and remove the exception. Warn before removal that the number can
be blocked again by existing rules. Preserve the underlying block entries.

Make this area accessible with the new permission alone. A role does not
need block list or spam settings rights to manage whitelist entries. Do not
fetch protected settings or the whole block list for such a role.

## Permissions and scope

Add `blocking.allowlist`, with the German label **Whitelist verwalten** and
the English label **Manage call whitelist**. This permission permits read,
create and delete operations on the tenant's whitelist.

- Administrators have it through their existing all-permissions behavior.
- Existing non-admin roles receive no new grant automatically. An
  administrator selects the permission in the role editor.
- `blocking.manage` and `blocking.settings` keep their current purposes.
- Check the permission on the server for every whitelist endpoint. A hidden
  button is not an access check. A revoked permission rejects the next write.
- Whitelist changes write audit entries with the actor, number and action.
  Save the change and audit record in one transaction.
- Whitelist access does not grant access to all calls. Keep existing tenant
  and extension restrictions. A call blocked before extension routing can
  remain visible only to users with `calls.view_all`. Do not assign a false
  extension to make it visible to other users.

This uses the existing role model. It does not add permission overrides on
individual user records.

## Proposed implementation

### Stored decisions

Add nullable `cdr.blocked_reason`. Store and validate the existing reason
format from `talkops_blocked`. Carry it through the CDR parser, insert,
selects, spool import and API response. Preserve duplicate-import behavior.
Set the dialed destination before the early block response. Only an inbound
call with a stored block marker gets the blocked result.

Add the optional field to the web API type and use a shared result helper.
Return current whitelist membership with call entries for authorized users,
using a batch query. Keep current membership separate from the stored reason.
List refreshes must reflect changes made in the other view.

Old rows have `NULL` and keep their existing classification. Do not infer a
historical block from `CALL_REJECTED`, today's block lists or a new PhoneBlock
lookup. Old XML still in the retry spool can supply its original block marker
when it is first imported after the update. Existing stored rows are not
automatically reconstructed from server logs.

### Whitelist and decision order

Add a tenant-scoped `call_allowlist` table with an ID, exact normalized number,
label and creation time. Enforce uniqueness on `(tenant_id, number)`.
Reject prefixes, anonymous markers, internal extensions and malformed numbers.
Use the existing tenant dial plan for normalization from call metadata.

For a known inbound caller, use this order:

1. Exact local whitelist match: continue normal routing.
2. Local block list match: block the call.
3. PhoneBlock threshold reached: block the call.
4. Otherwise: continue normal routing.

The anonymous-caller rule remains separate because no exact number exists.
An exact whitelist entry can override a broad local block prefix for just one
number. Whitelisting does not bypass schedules, destinations or other routing
rules. Check the whitelist before a cached PhoneBlock result, so a successful
save takes effect on the next call lookup without a cache wait or restart.
Removing the entry restores normal checks, including the existing cache.

Add OpenAPI routes under `/api/v1/call-allowlist` for list and create, plus
`/api/v1/call-allowlist/{id}` for delete. Apply tenant scope, validation,
permission checks and audit recording. Reuse the existing number test path
for users with `blocking.manage`; its result must honor the whitelist.

### Statistics consistency

Exclude known blocked calls from the dashboard's missed count. Keep them in
total and inbound volume, and expose a blocked count in the statistics API.
Calculate answer rate from answered inbound calls divided by non-blocked
inbound calls. Return no rate when that denominator is zero. Do not present
a blocked call as answered by merely subtracting it from missed calls.

The requested UI change remains in the call list and dashboard recent calls.
Apply the stored field to shared result logic where needed so another view
does not relabel a known blocked call as missed. A wider softphone redesign
and new reporting screens are outside this proposal.

### Migrations and dependencies

No migration is added or reserved in the proposal. Implementation is expected
to add one forward migration for the CDR field and whitelist table. Its ID
must follow the latest migration in the verified target and be checked against
concurrent PRs. At this base, the highest ID is `20261106000002`.

Update the permission catalog, role editor translations and role tests.
Verify backup and restore include the new table and CDR field. New runtime
packages and new external services are not expected. PR 55 concerns fax and
is not a prerequisite. Recheck the target and open PRs before implementation.

## Acceptance and verification plan after feedback

| Area | Required evidence |
| --- | --- |
| Stored reason | Parser and PostgreSQL tests cover own exact list, prefix list, PhoneBlock and anonymous sources. HTTP and spool ingestion preserve the reason and duplicate behavior. |
| Classification | A normal declined call stays busy or declined. Legacy rows stay unchanged. Blocked filtering, combined filters, counts and pagination agree. |
| Whitelist priority | Exact whitelist beats exact and prefix blocks and a cached PhoneBlock rejection. A neighboring number stays blocked. Deletion restores checks. Anonymous rules and service-failure behavior stay valid. |
| Access | Unauthorized direct API requests fail. New grants and revocations take effect. Tenant isolation and own-call scope remain intact. An allowlist-only role can manage entries without reading protected settings. |
| Writes | Duplicate creation is safe. Invalid numbers and wildcard entries fail. Every actual change has an audit record. Failed writes do not leave partial changes. |
| UI | Both views show the source, offer the permitted action, confirm the exact number, handle failures and show current membership while retaining the historical reason. Cover anonymous callers and old records. |
| Statistics | Mixed answered, missed and blocked calls produce correct counts and answer rates. All-blocked traffic has no answer rate. |
| Layout and language | Browser checks cover German and English, light and dark themes, keyboard use, narrow screens, phone book toggle, loading and polling. |
| Upgrade | Existing records migrate with a null reason. Backup and restore retain the new data. Migration IDs are unique. |
| SIP | A controlled blocked inbound call returns 603 and creates a CDR with the reason. After whitelisting, the next call proceeds to its configured route. |

Run the applicable commands from `.github/workflows/ci.yml`: Rust formatting,
clippy and workspace tests with PostgreSQL; web lint, type checks, unit tests,
build and browser regressions. Extend the FreeSWITCH blocking integration
test and add a controlled SIPp scenario. Report unavailable service checks
separately from passing checks. This proposal revision runs documentation
checks only; it makes no claim that the proposed behavior is implemented.

Update English and German call log, call routing and roles documentation.
Save implementation results and migration IDs in the task summary and PR.

## Suggested release notes after implementation

Show the reason for blocked inbound calls in the call list and recent calls.
Let authorized users add exact numbers to a local whitelist. Apply whitelist
exceptions before local block lists and PhoneBlock. Keep blocked calls
separate from missed calls and retain their original reason in call history.

## Review requested

The recommended scope is the complete proposal above. Confirm or change the
separate role permission, tenant-wide exact-number exceptions, and exclusion
of known blocked calls from missed-call statistics before implementation.

## References

- [Development workflow](release-workflow.md)
- [UI guidelines](ui-guidelines.md)
- [Roles and permissions](adr/0029-roles-and-permissions.md)
- [Call log](en/call-log.md)
- [Call routing](en/call-routing.md)
