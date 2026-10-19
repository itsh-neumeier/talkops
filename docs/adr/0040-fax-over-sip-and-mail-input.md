# 0040 - Fax over SIP and mailbox input

- Status: Accepted by the user on 2026-10-18.
- Extends: ADRs 0002, 0005, 0006, 0029 and 0034.

## Decision

Use the existing FreeSWITCH `mod_spandsp` for fax transmission. Keep number
selection, permissions, queue state and admission rules in Rust. Add `fax`
as an explicit destination for incoming numbers. Existing voice routes do
not change during upgrade. Do not add automatic voice/fax detection.

Keep fax attempts separate from generic jobs. Preparation jobs can create
files again. An uncertain transmission must not dial again automatically.
Store the call UUID before originate. Consume session results and replayed
CDRs. SIP answer and accepted originate commands do not prove fax delivery.
Use at most three attempts for confirmed busy or unanswered calls with no
transferred pages. Require an explicit resend for uncertain outcomes.

Use a dedicated existing SMTP mailbox for mail input. Read the mailbox over
IMAPS with certificate verification. Keep the password encrypted with the
existing secret box. SMTP output supplies rejection notices and fax reports.
The mail server must bind sender identities to authenticated accounts.

The user confirmed these mail rules:

- The subject contains only an international number such as `+49123456`.
- The complete sender email address must have an enabled whitelist entry.
- A rejected sender receives a not-permitted notice. An invalid subject
  receives instructions for a number-only subject.
- Only PDF and image file attachments become fax pages. Mail body text,
  HTML, quoted messages and inline signature images never become pages.
- The fax header contains the accepted sender email address. The saved
  footer is separate from mail content.

Store a mailbox UID receipt and message fingerprint with each accepted
fax transaction. Acknowledge the message only after a durable receipt.
Queue rejection notices separately. Ignore automatic replies and reports
to prevent response loops.

Use Python standard-library adapters for IMAPS/MIME and bounded subprocess
conversion. The existing Rust worker invokes the conversion adapter. Use
Poppler for PDF rasterization, Pillow/libtiff for image decoding and Group 4
TIFF, and ReportLab for the PDF preview. Generate the preview and TIFF from
the same page images. No Python adapter can originate a call or select a
sender number.

## Consequences

The server image needs Python. The media worker also needs Poppler, Pillow,
ReportLab and DejaVu fonts. All services share a new `fax` volume with the
existing UID/GID 10001. Include this volume in backup and restore.

Fax is disabled by default. New role permissions are opt-in for non-admin
roles. Migration `20261106000001` adds the enum value. Migration
`20261106000002` adds the tables and tenant-safe foreign keys. The upgraded
schema is forward-only. Older binaries cannot handle a configured fax route.

Transport and provider compatibility need a real fax peer. Local unit and
database tests do not prove provider interoperability.

Language review: clear English review. Official ASD-STE100 standard unavailable.
