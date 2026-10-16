# Mail notifications for operations events

## Task state

- Task ID: 20261016-claude-notifications
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed
- Branch: `claude/notifications` (stacked on `claude/mail-design-editor`)
- Base commit: `da03f9f` (head of PR #40)
- Related commits or PR: Pending (see PR body)

## Goal

Send e-mail notifications for operations events (trunks, FreeSWITCH,
devices, security, backup and storage, services, configuration changes)
with configurable rules, delay with recovery, digest and rate limit. Use the
mail renderer and template registry of ADR 0034 (PR #40).

## Result

- Core module `talkops_core::notifications`: event catalog (40 types, 32
  with an own rule, 5 marked as planned), outbox `emit`, rules with defaults, recipient
  resolution, pure decision functions (`decide`, `on_recovery`), alert,
  digest and delivery-log storage, purge.
- Migration `20261103000002_notifications.sql`: `notification_events`,
  `notification_rules`, `notification_alerts`, `notification_deliveries`,
  `login_addresses`.
- Mail type `notification` in `mail_render::KINDS` (de/en default texts,
  editable in Settings → E-mail).
- Worker `talkops-api::notifier`: outbox processing every 5 s, due alerts
  and digests, health checks every 5 min (disk space, failing/waiting
  media jobs, backup age), purge after 30 days. `Monitor`/`diff` compares
  FreeSWITCH observations from the telephony poller.
- Producers: `audit::record` (same SQL statement), telephony poller,
  `backups::record_run`, `backup_targets::record_run`, SIP guard bans,
  blocked web sign-ins, admin sign-in from a new address, voicemail mail
  failures.
- Permission `notifications.manage`; API `/api/v1/settings/notifications`
  (view, rule save/reset, test, log).
- UI: Settings → Notifications (`NotificationSettings.svelte`).
- Docs: ADR 0035, `docs/en/notifications.md`, `docs/de/benachrichtigungen.md`,
  README link.

## Changes

- `crates/talkops-core/src/notifications.rs`: new module.
- `crates/talkops-core/src/audit.rs`: writes the matching notification event.
- `crates/talkops-core/src/{backups,backup_targets}.rs`: emit backup events.
- `crates/talkops-core/src/mail_render.rs`: mail type `notification`.
- `crates/talkops-core/src/roles.rs`: permission `notifications.manage`.
- `crates/talkops-api/src/notifier.rs`: worker, monitor, checks.
- `crates/talkops-api/src/telephony.rs`: uptime, completeness flag, monitor call.
- `crates/talkops-api/src/routes/{notifications,auth}.rs`, `sip_guard.rs`,
  `mailer.rs`, `main.rs`: API and producers.
- `web/src/lib/components/NotificationSettings.svelte` and settings
  navigation, API types, i18n en/de.
- `tests/browser/layout.mjs`: fixtures and route `/settings#notifications`.

## Decisions

- Outbox table instead of direct sends: producers stay simple; rules,
  delay and digests are in one place.
- Audit-based configuration events: one mapping function covers all
  mutation handlers.
- Recovery events have no own rule; they follow the problem rule.
- Mails use the tenant default language; one rendering per delivery.
- Monitor state is in memory; an extra recovery event after a restart has no
  effect without an open alert.

## Checks

- Passed: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test` per crate and per test target (see PR body for the
  final run).
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`,
  `tests/browser/layout.mjs`, `tests/browser/roles.mjs`.
- Manual: screenshots of the settings page at 360 px and 1280 px.
- Not run: E2E (Docker/SIPp), delivery to real mail clients, real
  FreeSWITCH gateway transitions.
- STE review: manual review of ADR and user docs; full standard
  unavailable.

## Dependencies and open work

- Depends on PR #40 (`claude/mail-design-editor`, migration
  `20261103000001`). Merge PR #40 first.
- Producers still missing: `trunk.outbound_failures`,
  `freeswitch.xml_curl_failed`, `freeswitch.profile_down`,
  `freeswitch.channels_high`, `services.update_available`.
- The settings page does not react to hash changes on the same page
  (existing behavior); the layout test opens `/settings#notifications` after
  another route.

## Next action

Release manager: review and merge after PR #40.

## References

- ADR 0034, ADR 0035.
- `docs/ai-memory/20261016-claude-mail-design-editor.md`.
