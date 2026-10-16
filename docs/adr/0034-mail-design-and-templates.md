# 0034 - Mail design and editable mail templates

- Status: Accepted
- Date: 2026-10-16
- Extends: ADR 0010 (voicemail mails).

## Context

TalkOps sent voicemail mails and the SMTP test mail as plain text with fixed
German or English text. Administrators want mails that show their
installation (name, URL, logo, colors) and texts that they can change.
More mail types will follow (notifications, ADR 0035). A free HTML editor is
hard to use safely: mail clients render HTML differently, and a user text or
a caller name must never add markup or links to a mail.

## Options

1. Free HTML templates that the administrator edits.
2. A fixed layout with editable branding and texts in a Markdown subset with
   whitelisted placeholders.
3. Fixed texts with only a logo and a color.

## Decision

Option 2.

- Layout: one fixed, table-based HTML layout with inline CSS, 600 px wide,
  fluid on small screens. It sets `color-scheme` and has a dark-mode media
  query. Every mail has an HTML part and a plain-text part
  (`multipart/alternative`).
- Branding (table `mail_branding`): sender display name, accent color
  (`#rrggbb`), header text, footer text and logo. The link color is the
  accent color only if its contrast to white is 3:1 or more.
- Logo: PNG, JPEG or GIF, at most 256 KB and 4096 × 4096 pixels. The server
  reads type and size from the file header (no SVG). Mails embed the logo as
  inline image with Content-ID `logo@talkops` (`multipart/related`). The
  display size is at most 48 px high and 240 px wide.
- Instance information: new setting `tenant_settings.instance_name`. The
  public URL comes from the existing sign-in setting `public_url`. Host name
  and TalkOps version come from the server. When the instance name is set,
  each subject starts with `[name]`.
- The layout adds the instance line and "Powered by TalkOps vX.Y.Z" below
  the editable footer. No setting can remove this line.
- Texts (table `mail_templates`, one row per tenant, mail type and
  language): subject and body. No row means the built-in default text.
  "Reset to default" deletes the row.
- Mail types are a registry in `talkops_core::mail_render::KINDS`. Each type
  has its placeholders, sample values and default texts in German and
  English. New mail types add an entry; the API and the editor list them.
- Text syntax: paragraphs, line breaks, `**bold**`, `*italic*`, links
  `[text](https://…)` and `-`/`1.` lists, plus MiniJinja expressions
  (`{{ caller.name }}`, `{% if transcript %}`).
- Rendering: the Markdown subset becomes an HTML template first. Literal
  text is HTML-escaped and template tags stay unchanged. Then MiniJinja fills
  in the values with HTML auto-escaping. A value thus cannot add markup. The
  plain-text part comes from the same source without markup.
- Sandbox: MiniJinja has no loader (no `include`, `import`, `extends`), a
  fuel limit and a recursion limit. On save, the server compiles the text,
  rejects every variable that is not a placeholder of the mail type (this
  also rejects globals such as `range`), and renders the text with sample
  values.
- Links: a link target must be `http(s)://` or `mailto:`, or it must start
  with `{{ instance.url }}`. Other placeholders cannot set a link target.
  After rendering, the server replaces each other link target with `#`.
- API (`voicemail.settings` permission, like the SMTP settings):
  `GET /api/v1/settings/mail`, `PUT …/mail/branding`,
  `POST|GET|DELETE …/mail/logo`, `PUT|DELETE …/mail/templates/{kind}/{lang}`,
  `POST …/mail/preview` (sample values, logo as `data:` URL) and
  `POST …/mail/test`. Every change writes an audit entry.
- The UI shows the preview in an `<iframe sandbox="">` with `srcdoc`.

## Consequences

- Mails are larger (HTML part and logo).
- The voicemail mail text changes in form; the facts stay the same.
- Texts with a placeholder that a later version removes fail to render.
  The mail job then fails and shows the error; "Reset to default" repairs it.
- Header and footer text can use only the common placeholders.
- Option 1 stays possible later as an additional expert mode.

Language review: Clear English review only. The official ASD-STE100 standard
was not available. Full language review is pending.
