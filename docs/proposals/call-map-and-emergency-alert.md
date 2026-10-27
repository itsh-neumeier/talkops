# Call map markers and emergency call alerts

- State: Proposal. User review is required before product changes.
- Date: 2026-10-27 (Europe/Berlin).
- Base: `origin/main`, `f8bdfb37cf3f7705d7fbbaf63d6bd6148caba400` (1.8.11).
- Preview: [Interactive review mockup](../mockups/call-map-emergency.html).

## Scope

The user requested map previews, distinct mobile markers and an email alert
when an extension dials an emergency number. The user also requested a review
before implementation. This contribution contains a proposal and a mockup.
It does not change the application, database or mail delivery.

## Current behavior

- `CallMap.svelte` draws circular markers. Its native title contains a name,
  prefix and count. Category statistics appear after selection below the map.
- `routes/call_map.rs` combines mobile, service and other numbers without a
  local area into one country group, such as `DE-none`.
- The map already has category counts, duration and recent calls. Access uses
  the same extension scope as the call list.
- `fsxml/dialplan.rs` classifies configured emergency numbers in authenticated
  internal routing. These calls bypass dialing profiles.
- ADR 0035 provides event rules, users, roles, SMTP, editable mail templates
  and delivery logs. The worker polls every five seconds. The event catalog
  has no emergency dialing event at this base commit.

## Proposed map behavior

### Marker meaning

| Group | Marker | Label and position |
| --- | --- | --- |
| Local network area or known place | Desk phone with handset, keypad and category pie in its display | Place, area code and country at the existing area point |
| Mobile numbers | Phone outline with category pie inside the display | Mobile, country and calling code at the country label point |
| Service or unresolved numbers with a known country | Rounded square with category pie | No local area, country and calling code at the country label point |
| Unknown country or anonymous calls | List entry | No map position is invented |

Use the same shapes for all countries. A local network area indicates the
number range, not a verified caller location. A mobile marker indicates only
the country assigned to the number. The preview must state this distinction.

Separate mobile calls from other calls without an area. A phone must not
represent a mixed group of mobile and service numbers. The attached screenshot
contains 147 mobile calls and two other calls in the same country group.
The new groups must add up to the same 149 calls.

The user supplied a second reference on 2026-10-27. It replaces the earlier
proposal to keep local area markers circular. Use a desk phone for local
areas and a smartphone for mobile groups. Each device has a circular pie
in its display. The user reference is a design correction, not approval to
implement the application or the emergency alert.

The mockup has two density previews. **Devices with counts** shows four
category counts beside each marker. **Compact devices** uses the same device
shapes and shows exact counts in the hover/focus preview. Use the first density
when markers have sufficient space. On narrow screens or with overlapping
markers, use the compact density. Keep place labels and the mobile country
label, such as "DE Mobile", visible. Apply this rule to all countries.

Keep the map theme consistent with TalkOps. Subtle circular bases can anchor
the devices. Do not infer call routes from the decorative dashed lines in
the reference: aggregated groups do not supply relationships between markers.
The mockup therefore has no connecting routes. A pie clipped to a complete
device silhouette would distort visible segment areas; keep the pie circular.

Marker size continues to indicate call count. Use the pie area for the size
scale, before the device frame is added. Use minimum 44 px hit targets
on touch devices. Separate country markers with screen offsets when they share
an anchor. Do not alter their geographic coordinates. At small sizes, keep the
shape visible and show exact counts in the preview.

### Preview before selection

- Pointer hover and keyboard focus open a preview without selecting a group.
- Show place or country, prefix, group type, selected time range, total calls,
  answered, missed, outgoing, blocked calls and talk duration.
- Use the current category filters for counts, pie segments and duration.
- A mobile preview says "Country assignment only; no caller location".
- A preview contains no individual phone numbers or contact names.
- Keep the preview inside the map bounds. The pointer can enter it without
  closing it. Escape dismisses it. Moving focus away also closes it.
- Click, Enter or Space opens the existing detailed panel. Hover must not
  move focus, change the selected group or load an area boundary.
- A touch device opens the same preview on tap. An explicit details action
  opens the detailed panel. A tap outside closes the preview.
- The compact dashboard gets the same shape language and preview. Its mouse
  wheel continues to scroll the page.

### API and compatibility

Keep the existing `kind: area | none` field. Add a `marker_kind` field with
`area`, `mobile` or `unlocated`. Split known-country non-geographic groups
into `<country>-mobile` and `<country>-unlocated` in the aggregation response.
Existing local area IDs stay unchanged. Keep `<country>-none` as a combined
alias in call-list filters so saved list links still work. New links use the
specific group ID. Unknown-country `none` and anonymous totals stay separate.

The new UI and server ship together. Older clients still receive valid `kind`
values, but can show more country groups. Add OpenAPI examples and tests for
the split and for old filter aliases. Do not change the geo data files.

This changes the grouping rule in ADR 0044. After approval, add a new ADR that
supersedes only that rule. Keep the offline map, attribution and tile policy.
No map database migration or new package is expected.

## Proposed emergency alert

### Settings and email

Extend **Settings > Notifications** with the event `calls.emergency_dialed`:

- Label: "Emergency number dialed". Severity: critical.
- Initial state: disabled. The administrator enables the rule explicitly.
- Recipients: selected users or selected roles. The built-in admin role is
  the proposed default recipient. Existing recipient validation and address
  deduplication apply. Disabled users do not receive mail.
- Source: every authenticated extension in the tenant, including devices
  without a user and the browser softphone. The caller's web role does not
  control whether an event is produced.
- Numbers: use `tenant_settings.emergency_numbers`, including changed country
  settings. Do not hard-code 110 and 112 in the producer.
- Trigger: a dialing attempt, before trunk lookup. An unavailable trunk or
  missing outbound number must not suppress the event. An answer is not required.
- Delivery: asynchronous, normally processed on the next worker cycle.
  SMTP and mailbox delivery time remain external factors.
- Treat this as a one-time event per call. Do not use recovery messages,
  delay or digest. Do not merge distinct calls through a source cooldown.
- Do not apply the normal per-rule hourly suppression to this event. Show
  that behavior in the editor. Deduplicate repeated processing of the same call.
- Missing SMTP, empty recipients or delivery errors remain visible in the
  existing delivery log. Saving the rule must show missing SMTP or recipients.
- A test button uses synthetic data and the existing test-mail path. It must
  never originate a real call or dial an emergency number.

Suggested subject: "Emergency number 112 dialed by extension 21".
Include the extension number and display name, dialed emergency number,
attempt time with tenant time zone, call UUID and a clear "dialing attempt"
status. Do not include audio, credentials or a claimed physical location.
Reuse the existing notification mail design and German/English text support.

### Producer and persistence

Produce the event after the internal caller is resolved and the dial plan
classifies the number as `Dialed::Emergency`. Place this before profile and
outbound route lookups. The producer must never send SMTP in the routing
request. Use a bounded best-effort insert; an insert error must not reject
or indefinitely delay the call. Log the error without secret values.

Use tenant ID, `calls.emergency_dialed` and the originating call UUID as the
idempotency key. Add a forward migration with a partial unique index on
`notification_events(tenant_id, event_type, source)` for this event only.
Use `ON CONFLICT DO NOTHING` in its producer. Do not change other producers.
Reserve the timestamp ID only when implementation starts, after checking
remote migrations and concurrent contributions. No migration ID is allocated
in this proposal.

The existing worker has no guaranteed SMTP retry delivery. This proposal
reuses its delivery log and failure behavior. Event deduplication does not
guarantee exactly one email across SMTP failures or process crashes. Changes
to general retry delivery are outside this task. Test restart behavior and
record the observed limit in the user documentation.

Only a trusted, resolved extension can produce the event. An unauthenticated
public inbound call to digits that resemble an emergency number must not
produce it. Check direct dialing, browser dialing, conference loopback and
extension-initiated transfer paths. Use the original call identity for
repeated routing of one attempt. Do not enable emergency forwarding paths
that the current routing rules reject.

Rule changes use `notifications.manage` and the existing audit path. Email
recipients can receive extension details through an enabled rule. State this
in the settings hint. Preserve emergency routing and caller ID behavior.

## Implementation sequence after approval

1. Record the accepted marker variant and any user changes in this document.
2. Add the grouping ADR, server aggregation and filter alias tests.
3. Add marker and preview helpers, Leaflet interactions and translations.
4. Add the event definition, idempotent producer, migration and rule behavior.
5. Extend settings, mail text, integration tests and browser checks.
6. Update `docs/en/call-log.md`, `docs/de/anrufliste.md` and notification docs.
7. Run the applicable CI checks. Update the PR and mark it ready only after
   the accepted implementation and checks are complete.

## Acceptance and check plan

| Area | Required evidence after implementation |
| --- | --- |
| Marker types | Local areas use desk phones; DE and foreign mobile groups use smartphones; service/other groups use a separate shape; no calls are lost or counted twice |
| Preview | Hover and focus show current statistics before selection; keyboard, Escape, pointer movement and map edges work |
| Touch and themes | Tap preview and details action; 320 px layout, 44 px targets, light and dark themes, reduced motion |
| Selection | Hover preserves selection; click and keyboard open details; filters and refresh do not leave stale previews |
| Map scope | Tenant and extension visibility remain intact; no new external requests with the offline map |
| Links | New split IDs filter the right calls; old `<country>-none` links return the combined calls |
| Emergency matching | Configured numbers match after existing normalization; 1120, ordinary service numbers, inbound calls and invalid callers do not trigger |
| Routing | Two extensions and the softphone trigger; no-route and unanswered attempts trigger; SMTP or event-insert failure does not block routing |
| Duplicates | Repeated and concurrent processing of one call creates one event; a second call from the same extension creates another event |
| Recipients | Admin role, custom role and user selection, inactive users, duplicate addresses, empty recipients, cross-tenant IDs and permission denial |
| Delivery | Disabled rule, no SMTP, SMTP failure, normal success, worker restart, one-time semantics and hourly suppression exception |
| Test action | Synthetic test mail only; no FreeSWITCH originate or real emergency call |

Use `web/tests/callMap.test.ts`, `tests/browser/call-map.mjs`,
`crates/talkops-api/tests/call_map.rs` and notification/dialplan tests.
Add `#[sqlx::test]` coverage for every new domain query and the migration.

Run `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked` with CI service prerequisites.
Run web `npm ci`, `npm run lint`, `npm run check`, `npm test` and
`npm run build`. Run the applicable Playwright checks from `ci.yml` and
isolated SIP fixtures from `e2e.yml`. Use mock trunks for emergency fixtures.
Also run the handbook checks: `node tests/docs/wiki.mjs` and
`node scripts/docs/build-wiki.mjs --check`.

## Release handoff

- Dependencies: existing map and notification features are on the base.
  Open PRs #61, #81 and #82 have no identified dependency for this proposal.
  Recheck this at implementation time.
- Migration IDs: none in this contribution. One event-deduplication index
  migration is proposed. Allocate its ID during implementation.
- Upgrade: proposal only. A later feature upgrade is additive, with the new
  emergency rule disabled and existing combined map filter links supported.
- Suggested future release notes, only after implementation:
  - "Preview call counts and duration before selecting a map marker."
  - "Distinguish local areas, mobile numbers and other numbers without a local area."
  - "Configure email alerts to users or roles when an extension dials an emergency number."
- Release version, target merges, tags and publication belong to the release
  manager. This draft must not be released as an implemented feature.
