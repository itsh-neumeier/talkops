# 0047 - Call map device markers and emergency dialing alerts

- Status: Accepted by the user on 2026-10-27.
- Supersedes: Only the combined country grouping and circular marker rules
  of ADR 0044. Other map decisions remain in effect.
- Extends: ADR 0035 with an immediate, opt-in emergency dialing event.
- Number: 0046 is used by the concurrent Coral contribution (PR #81).

## Context

Users need statistics before they select a map marker. A round country
marker can imply a local area for a mobile number. The user selected device
shapes in a supplied reference: desk phones for local areas, smartphones
for mobile numbers, and a circular category pie in each display. The user
then requested the label "DE ohne Ortsbezug" instead of "DE Mobil".

An administrator must be able to notify users or roles when any extension
dials a configured emergency number. Notification failures must not reject
the call. The existing notification outbox and mail renderer cover delivery.

## Decision

Split known-country groups into `<country>-mobile` and
`<country>-unlocated`. Add `marker_kind` while retaining `kind: area | none`.
Keep `<country>-none` as a combined call-list filter alias. Totals and access
scope remain unchanged. Unknown-country and anonymous calls stay off the map.

Use device silhouettes and category pies. Show counts beside markers when
space permits; omit these counts on dense maps and narrow screens. Hover
and focus open a summary without selecting a group. Touch opens the summary
first, with an explicit details action. Mobile labels use country code and
"without local area". These labels never claim a caller location. No new
map data provider or browser dependency is needed.

Add `calls.emergency_dialed` to the notification catalog, disabled by default.
Use existing user, role and email recipients. A configured rule requires an
active recipient with an email address. Show missing SMTP in settings.
Do not delay, digest, recover or apply hourly suppression to this event.
Each distinct call can produce a delivery. The saved rule and worker both
enforce immediate semantics.

Produce the event after authenticated extension resolution and emergency
classification, before profile or trunk lookup. Also cover transfers with
a resolved transferring extension. Use the configured emergency list.
Store the event through a bounded 250 ms best-effort insert. Log storage
failure and continue routing. SMTP runs only in the notification worker.

Migration `20261113000001_emergency_dial_alert.sql` adds a partial unique
index for the event's tenant, type and originating call UUID. The producer
uses `ON CONFLICT DO NOTHING`. Other event types are unchanged.

## Consequences

- Existing country filter bookmarks continue to work.
- A feature upgrade changes no existing enabled notification rule.
- SMTP remains asynchronous. A dialing alert confirms an attempt, not an
  answered call or a physical location.
- Event insertion and SMTP delivery can fail. The existing worker marks
  events processed before delivery. It does not guarantee retry delivery or
  exactly one email across a process crash. Delivery results remain in the log.
- Event deduplication lasts for the existing event retention period. General
  notification retry changes are outside this decision.
