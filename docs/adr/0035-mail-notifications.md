# 0035 - Mail notifications for operations events

- Status: Accepted
- Date: 2026-10-16
- Extends: ADR 0034 (mail design and templates).

## Context

Administrators of a small installation do not watch the dashboard all the
time. They must know when a trunk loses its registration, FreeSWITCH is not
reachable, a backup fails, the disk is almost full or somebody changes the
configuration. Prometheus alerting (`/metrics`) is available, but most
homelab and small-office installations do not run Prometheus. Mail is
already configured for voicemail.

Requirements: no mail storm when a problem repeats, no mail for a short
interruption, a mail when the problem ends, configurable recipients, and
no change to the producers when the mail text or the rules change.

## Options

1. Send the mail directly where the problem is detected.
2. An outbox table: producers write events; one worker applies rules and
   sends mails.
3. External alerting only (Prometheus Alertmanager).

## Decision

Option 2.

- Event catalog in code: `talkops_core::notifications::CATALOG`. Each event
  type has a key (e.g. `trunk.down`), a category, a severity (`info`,
  `warning`, `critical`), a title in German and English with `{param}`
  placeholders and default rule values. A problem event names its recovery
  event (`trunk.down` → `trunk.up`). Event types without a producer are
  marked `implemented = false` and are shown as "planned".
- Outbox: table `notification_events` (`event_type`, `source`, `severity`,
  `params`). `source` identifies the object (gateway, SIP user, path) so that
  a recovery event closes the correct problem. `params` never contain
  secrets.
- Producers:
  - `audit::record` writes the matching event in the same SQL statement
    (`notifications::audit_event` maps entity type and action to
    `config.*`, `trunk.changed` and `security.*`). All mutations thus
    produce events without changes to the handlers.
  - The telephony poller compares FreeSWITCH states (ESL connection,
    uptime, gateway states, registrations) and emits `freeswitch.*`,
    `trunk.*` and `device.*` events on a change (`notifier::diff`).
  - Backups and backup targets emit events in `record_run`.
  - SIP guard bans, blocked web sign-ins, an administrator sign-in from a
    new address (table `login_addresses`) and failed voicemail mails emit
    events where they occur.
  - The worker checks free disk space, failing and waiting media jobs and
    the age of the last backup every 5 minutes.
- Rules: table `notification_rules`, one row per tenant and event type. No
  row means the catalog defaults with the built-in admin role as recipient.
  A rule has: enabled, minimum severity, recipients (users, roles, free
  addresses), delay, recovery mail, digest time and a limit of mails per
  hour. Recovery events use the rule of their problem event.
- Worker (`talkops-api::notifier`), every 5 seconds:
  - Problem event with a delay: open a `pending` alert. A recovery within
    the delay deletes it without a mail. After the delay the alert becomes
    `alerted` and the mail goes out. Further problem events of the same
    source are ignored while an alert is open.
  - Recovery event for an `alerted` alert: close it and send a recovery
    mail if the rule wants it.
  - Digest time > 0: collect the events and send one mail after this time.
  - Rate limit: more mails per rule and hour than the limit are logged as
    `rate_limited` and not sent.
  - Every delivery is logged in `notification_deliveries` (sent, failed,
    rate limited, no recipients, no SMTP).
  - Processed events and log entries are deleted after 30 days.
- Mail: new mail type `notification` in the ADR 0034 registry. The text is
  editable like every mail type. The worker renders the mail once in the
  default language of the tenant and sends it to each recipient.
- Permission: new permission `notifications.manage` (admin has it). API
  under `/api/v1/settings/notifications`. UI: Settings → Notifications.

## Consequences

- New producers only call `notifications::emit` and add a catalog entry.
- Event types `trunk.outbound_failures`, `freeswitch.xml_curl_failed`,
  `services.update_available`, `freeswitch.profile_down` and
  `freeswitch.channels_high` have no producer yet.
- The state comparison of the poller and the health checks is in memory.
  After a restart the first observation can send a recovery event; it has
  no effect if no alert is open.
- The outbox is processed by one server process. Several servers would need
  `FOR UPDATE SKIP LOCKED` on the alert table too.
