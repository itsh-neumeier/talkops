# Mail design and template editor

## Task state

- Task ID: 20261016-claude-mail-design-editor
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed
- Branch: `claude/mail-design-editor`
- Base commit: `0f128ad` (release 1.7.14)
- Related commits or PR: Pending

## Goal

Send HTML + plain-text mails in a robust layout with branding, instance
information and editable texts per mail type and language. Supply a mail
type registry that the notification task (ADR 0035, branch
`claude/notifications`) can extend.

## Result

- New core modules: `mail_render` (registry, Markdown subset, MiniJinja
  sandbox, layout) and `mail_templates` (storage of branding, logo, texts).
- Voicemail mail and SMTP test mail use the new renderer. Mails are
  `multipart/alternative`; the logo is an inline CID image
  (`multipart/related`); attachments go into `multipart/mixed`.
- New API under `/api/v1/settings/mail` (design view, branding, logo,
  templates, preview, test send). Permission `voicemail.settings`.
- UI: two new cards in Settings → E-mail (`MailDesign.svelte`): mail design
  and mail texts with placeholder picker, live preview in a sandboxed frame,
  test send and reset.
- Docs: ADR 0034, `docs/en/mail.md`, `docs/de/e-mail.md`, links from README
  and the voicemail docs.

## Changes

- `migrations/20261103000001_mail_templates.sql`: `tenant_settings.instance_name`,
  tables `mail_branding` and `mail_templates`.
- `crates/talkops-core/src/mail_render.rs`, `mail_templates.rs`: rendering and storage.
- `crates/talkops-core/Cargo.toml`: `minijinja` 2.24 with feature `fuel`.
- `crates/talkops-api/src/mailer.rs`: multipart builder, `render_for`, `send_test`.
- `crates/talkops-api/src/routes/mail.rs`: new endpoints.
- `crates/talkops-api/tests/mail.rs`: sqlx integration tests.
- `web/src/lib/components/MailDesign.svelte`, `web/src/lib/api.ts`, i18n en/de.
- `tests/browser/layout.mjs`: fixtures and route `/settings#email`.

## Decisions

- Instance name is a column of `tenant_settings`, but it is not part of the
  `TenantSettings` struct. Only the branding endpoint writes it. Thus the
  telephony settings form cannot overwrite it with an old value.
- The public URL is the existing `identity_settings.public_url`. No second
  URL setting.
- Markdown first, then MiniJinja with HTML auto-escaping: values never add
  markup. Link targets: http(s), mailto, or `{{ instance.url }}` prefix only.
- Placeholder check uses `Template::undeclared_variables(true)`; exact names
  only. Globals such as `range` are thus rejected.
- Logo stored in the database (max 256 KB, PNG/JPEG/GIF, size read from the
  header), so backups contain it.
- "Powered by TalkOps vX.Y.Z" is part of the layout, not of an editable text.

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test` for all crates and all `talkops-api` test targets
  (run target by target because of low disk space; `recovery` failed once
  in `setup_admin` during a disk-full phase and passed on the rerun).
- Passed: `npm run lint`, `npm run check`, `npm run build`, `npm test`.
- Passed: `tests/browser/layout.mjs` against a local preview.
- Manual: screenshots of the rendered mail (light, dark, 380 px) and of the
  settings page.
- Not run: E2E (Docker/SIPp), real SMTP delivery to mail clients.
- STE review: Clear English review only. The official ASD-STE100 standard
  was not available. Full language review is pending.

## Dependencies and open work

- Migration ID `20261103000001`. ADR 0034.
- Branch `claude/notifications` (ADR 0035) is stacked on this branch.
- Open: test in real clients (Outlook, Gmail, Apple Mail).

## Next action

Review and merge the PR; then merge the stacked notifications PR.

## References

- ADR 0010, ADR 0029, ADR 0034.
