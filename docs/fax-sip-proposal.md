# PDF fax over SIP: proposal for user review

Status: Proposed. Implementation needs user feedback first.
Date: 2026-10-18 (Europe/Berlin).
Base: `origin/main`, `7beb63d5b8e3917b7621588413afcec2a8893456`.

## Goal and review boundary

Add document fax send and receive functions to TalkOps. Users can send from the
web interface or through a configured mail address. Authorized users can
change the fax header and footer. An outbound fax uses an assigned own
number or a configured global number.

The user requested a proposal before implementation. This contribution
contains only this proposal and a task summary. It adds no runtime code,
database migration, dependency, UI control or configuration change.
Commit, push and draft PR creation are authorized. The release chat owns
integration, version changes, tags and publication.

## Verified current state

- The remote default branch is `main`. No open PR was returned at task start.
- The source checkout has one local commit and three untracked task summaries.
  This task preserves those files and uses its own worktree and branch.
- The FreeSWITCH image pins version `v1.10.12`. It builds and loads
  `mod_spandsp`. There is no application fax service, fax route or fax UI.
  Manufacturer provisioning reference files contain separate ATA fax settings.
- `talkops_core::trunks::outbound_route` resolves an enabled number to its
  trunk and account. The existing dialplan applies provider number formats.
- Roles have explicit permission keys (ADR 0029). Role checks read current
  permissions for each request. The admin role has all permissions.
- PostgreSQL stores jobs (ADR 0005). The API controls FreeSWITCH through
  XML lookups and ESL (ADR 0006).
- Mail settings and `talkops-api::mailer` send mail through SMTP. They do
  not receive SMTP messages or read a mailbox. Editable mail templates and
  operation notifications already exist (ADRs 0034 and 0035).
- Backup volumes are listed explicitly in `talkops-api::backup`.
  New fax files therefore need explicit backup and restore support.

## Proposed user behavior

### Web interface

Add a main menu entry **Fax** with **Send**, **Outbox** and **Inbox** tabs.
The send form has a destination number, one or more PDF files, a sender
number selector, and a preview. It shows page count and the rendered header
and footer before submission. Multiple files form one fax in selection order.
The form shows the resolved sender number before the user sends the fax.

The outbox shows queue state, attempts, pages, result and the send report.
The inbox shows the source number, receiving number, time, pages and PDF.
Downloads, deletion, cancellation and explicit resend each check access.
A resend creates a new linked transmission and warns about duplicate delivery.

Use the shared page, tab, table and dialog components. Follow
`docs/ui-guidelines.md`. Supply German and English UI strings.

### Header, footer and station identity

Add **Settings → Fax** for global defaults, number policy, mail input,
document limits and retention. An authorized user edits header and footer
once and reuses them for later jobs. Personal overrides need a separate
permission. A user without that permission can send with the stored template.

Use bounded plain text and a fixed placeholder list: sender name, sender
number, recipient number, date/time and page/total pages. Do not accept
HTML, scripts, arbitrary template expressions or FreeSWITCH variables.
The UI supplies a sample preview and a reset to global defaults action.

Reserve page margins for the header and footer. Scale the original page
into the content area without cropping. Render both texts into the document
before transmission. This gives the footer a defined implementation and
keeps the preview consistent with the transmitted file.
The SIP caller ID and fax station identity use the resolved sender number.
Avoid a second automatic page header from SpanDSP.
Snapshot the effective template and sender details when a job is created.
Changes apply to later submissions.

For mail-to-fax, the user's confirmed rule overrides the saved header:
print only the accepted sender email address in the fax header. Do not add
the sender display name, subject or mail body. The saved footer remains
available, but must not use mail body content. The protocol station identity
and SIP caller ID remain the resolved outgoing fax number.

### Own and global numbers

Offer two outbound policies: **own number, otherwise global** and
**always global**. The administrator selects the global number from enabled
TalkOps trunk numbers. A user can select only a number assigned to an owned
extension or an explicitly granted shared fax endpoint. Arbitrary caller IDs
are rejected. Use the existing trunk account and provider formatting rules.
Recheck permissions, number assignment and route availability before dialing.
Do not silently change the sender of an already queued job.

For incoming calls, add `fax` as a routing destination. A fax endpoint has
an owner or explicit shared users, an optional reception mailbox, and one
or more assigned incoming numbers. The global fax endpoint has an explicit
access list; it does not make all received documents public to fax senders.
An endpoint number need not equal an outbound caller ID.

Recommendation: receive on numbers explicitly routed to fax. Keep the
existing voice destination until an administrator changes it. Automatic
voice/fax detection on a shared number requires an additional agreed call
flow and separate interoperability tests.

### Mail-to-fax

Recommendation for the first implementation: use a dedicated mailbox on an
existing mail server. A user sends the mail through SMTP to, for example,
`faxin@example.com`. TalkOps reads that mailbox through IMAPS. The settings
contain the address, IMAP host, port, TLS mode, folder and encrypted login
secret. The existing outgoing SMTP configuration sends reports.

Confirmed by the user: the subject contains only one clean international
number, such as `+49123456`. The caret in the original example was a typing
error. Validate after MIME decoding and removal of outer whitespace. Syntax:
`^\+[1-9][0-9]{1,14}$`. This checks E.164 shape, not whether a number is
allocated. Apply the configured destination policy after this check.
Do not extract a number from arbitrary subject text. Reject multiple
numbers, `Re:`, control characters and the literal `^` inside a number.

Confirmed by the user: use only file attachments as fax pages. Accept one
or more PDFs or image files, including a mixture, in MIME attachment order.
Do not convert any plain-text body, HTML body, quoted thread, signature or
inline body image into a fax page. The only mail metadata printed on the
fax is the accepted sender email address in the header.

Proposed image formats: JPEG, PNG, TIFF, BMP and WebP. Check file signatures
and decode the file; do not trust its extension or MIME label. Apply image
orientation, preserve all pages of a multipage TIFF, and fit each image on
a fax page without cropping. Reject animated images with an explicit error.
Reject an entire submission with no supported file attachment, an unsupported
file attachment, or any invalid or encrypted document. Do not send a partial
set of attachments. Proposed limits: 20 MiB decoded attachments, 50 output
pages, 10 attachments, a 30 MiB raw MIME message, and bounded decoded image
dimensions and pixel count. Use the same preparation pipeline as web uploads.

Confirmed by the user: the sender email address must be on a whitelist.
Add an administrator-managed list under Fax settings. Each enabled entry
contains one complete sender address and the sending user or shared fax
endpoint used to resolve permissions and the outgoing number. Compare the
parsed single `From` address, not its display name or an address substring.
Do not accept wildcard entries, multiple From addresses or Reply-To as a
replacement identity. Apply a documented address normalization policy.
An empty whitelist permits no mail-to-fax submissions.

Apply the subject check and whitelist check before creating any conversion
or send job. A match does not bypass the configured fax number and access
policy. Recheck the entry before dialing a queued mail fax. Store the
accepted email address with the transmission and use it as the fax header.

The whitelist is the application admission rule. The existing mail server
must also supply a trusted sender identity or enforce sender binding for
authenticated submissions. A forged From header must not bypass the list.
Document and test this transport requirement. Mail input stays disabled
by default.

Store a unique mailbox identity plus UIDVALIDITY and UID before marking a
message processed. Store a bounded message fingerprint for duplicate
detection. A database transaction creates the fax and conversion job before
the message is moved to Processed. Reconnects must not submit it again.
Invalid messages move to Rejected and appear in an input log.

Confirmed by the user: reject a message with an invalid subject or a sender
outside the whitelist and send a rejection notice to the sender. The notice
states that submission is not permitted, or that the subject must contain
only a number such as `+49123456`. Invalid or missing attachments also produce
an actionable rejection. Address the notice to the parsed sender address,
including a sender outside the whitelist; never substitute Reply-To. Do not
echo attachments, mail body, whitelist contents or fax credentials. Queue
the notice independently: notification retries must never create a fax.
Use duplicate protection and suppress response loops for automatic messages
and delivery reports. A message with no usable sender is rejected and logged.
Ordinary fax reports go to the accepted sender address.

Examples for the implementation tests:

| Subject | Sender | Attachments | Result |
| --- | --- | --- | --- |
| `+49123456` | Whitelisted | One PDF | Submit; sender email in header. |
| `+49123456` | Whitelisted | Two PDFs and a JPEG | One fax in attachment order; sender email in header. |
| `+49123456` | Whitelisted | PNG only; body contains other text | Fax the PNG only; no body text. |
| `+49123456` | Not whitelisted | Valid PDF | No fax; send not-permitted notice to sender. |
| `Fax +49123456`, `Re: +49123456`, or `+49^123456` | Whitelisted | Valid PDF | No fax; send invalid-subject notice to sender. |
| `+49123456` | Whitelisted | No file attachments; body and inline logo only | No fax; send missing-attachment notice. |
| `+49123456` | Whitelisted | Valid PDF and invalid image | No partial fax; send invalid-attachment notice. |

Alternative for review: direct SMTP submission to a dedicated gateway.
Use a maintained mail server such as Postfix for TLS, authenticated users,
sender binding and queueing. A restricted delivery adapter submits the
message and trusted account identity to TalkOps. Do not build an SMTP
server into the HTTP service. This alternative needs additional deployment,
credential integration and image tests. Select the transport before coding;
do not add both transports by default.

### Incoming fax and reports

FreeSWITCH receives into TIFF. The worker creates a PDF for the inbox and
optional delivery to the endpoint's configured mail address. Preserve the
received page image; local outgoing templates do not change incoming pages.
Use the existing mail template registry for received, sent and failed fax
mails. Include the fax ID, numbers, page count, time and protocol result.
Distinguish a completed fax transmission from a delivered notification mail.
A mail failure must not resend the fax. Report partial reception explicitly.

## Proposed implementation

| Area | Planned changes |
| --- | --- |
| Core | Add fax settings, endpoints, grants, templates, documents, transmissions, attempts, sender whitelist and mail receipts. Keep tenant-safe relations and typed errors. |
| Database | Add forward-only schema migrations. Extend the destination enum in a separate migration before any migration uses the new value. Add role keys without granting new chargeable actions to existing user/operator roles. |
| API | Add `/api/v1/faxes` for upload, preview, submit, history, files, reports, cancel and resend. Add settings and endpoint management routes. Register OpenAPI schemas and audit mutations. |
| Routing | Extend `trunks`, `numbering`, XML dialplan generation, destination validation and all routing selectors with the fax destination. Keep business rules in Rust. |
| Transmission | Add an API-side fax dispatcher and ESL result handling. Tag calls with fax and attempt IDs. Correlate final results and CDR data. |
| Worker | Add bounded PDF and image normalization, page composition, multipage fax TIFF creation and received TIFF-to-PDF conversion jobs. |
| Mail | Add the selected ingress adapter, whitelist checks, strict subject validation, rejection notices, duplicate protection and fax report types. Keep secrets encrypted and out of logs. |
| UI | Add the Fax page, Fax settings, endpoint access controls, role labels, API types and both translations. |
| Operations | Add a `fax` volume to all three services, media paths, capacity checks, backup/restore, retention and deployment docs. |

Use Poppler for PDF rendering and libtiff tools for fax TIFF assembly and
received PDF conversion, subject to an image-level compatibility fixture.
Render at a supported monochrome fax resolution. Test page dimensions,
compression and multipage order against the pinned FreeSWITCH build.
Decode supported image files with a maintained image library and the same
resource limits. Match preview output to the final monochrome document.
Run converters without a shell, with fixed server-generated paths,
resource limits, a timeout and a private temporary directory. Reject
encrypted, corrupt and oversized PDFs. Do not pass user filenames to ESL.

Use `mod_spandsp` `txfax` and `rxfax` with the existing SIP gateway route.
Offer per-trunk modes **T.38 preferred with audio fallback**, **audio only**
and **T.38 required**. Use G.711 for audio fax. Do not use Opus or video.
Check actual behavior with FreeSWITCH `v1.10.12`; current upstream manuals
can describe newer behavior. Provider and remote fax support need a live test.

Create the document, transmission and initial job in one transaction.
Track queued, preparing, ready, dialing, sending, succeeded, failed,
cancelled and unknown outcomes. Lease preparation jobs and serialize active
fax attempts per configured concurrency limit (default: one per trunk).
Start with at most three attempts and delayed retries for confirmed busy
or temporary pre-transmission failures. Never treat SIP answer or ESL
command acceptance as delivery success.

Save the call UUID before originate. On a restart, reconcile it with active
channels, fax results and persisted CDR evidence. If delivery cannot be
established, mark the outcome unknown and require an explicit resend.
Do not requeue an active fax through the generic stale-job retry path.
Preserve the final state when duplicate or late events arrive. Keep fax
calls outside voice recording, transcription and voicemail flows.

## Permissions and data access

Proposed role keys: `fax.send`, `fax.mail_submit`, `fax.templates.edit`,
`fax.use_global`, `fax.access_all`, `fax.delete` and `fax.manage`.
`fax.manage` controls endpoints, shared grants, global templates, the sender
whitelist and policy;
`fax.templates.edit` controls personal overrides. Shared endpoint grants
separately specify send and inbox access. A settings permission alone does
not grant document access. Admin retains all permissions.

Check the tenant, ownership/grant and permission for each list, file,
preview, report and mutation. Check mail permissions again at submission
and before dialing. Audit settings, submission, cancellation, resend and
deletion without storing PDF contents or secrets in the audit log.
Retention skips active transmissions and removes all document derivatives.
Proposed retention default: 30 days, configurable separately for each
direction. Backups include fax documents independently of voice recordings.

## Planned verification and acceptance

| Test layer | Required evidence |
| --- | --- |
| Unit | Strict MIME subject parsing, exact sender whitelist matching, number policy, template precedence, sender email header, header/footer margins, PDF/image limits, retry decisions and duplicate events. Reject the literal caret example. |
| PostgreSQL | Migration from existing data, destination validation, every new domain query, tenant isolation, grants, revoked rights, transactional queue creation, concurrent claims and duplicate mail receipts. |
| API | Authorized and forbidden upload, send, settings, download, deletion and resend; invalid PDF; inaccessible global/own number; session and CSRF checks. |
| Conversion | Multipage PDFs, each supported image format, mixed attachment order, image orientation, multipage TIFF, small text, Unicode, no cropped content, sender email in mail fax header, saved footer, TIFF resolution/compression, corrupt/encrypted input, animation rejection, pixel limits, timeout and cleanup. |
| Browser | Upload → preview → send → status; inbox; role editor; hidden/disabled unauthorized controls; mobile and desktop layouts; German and English. |
| Mail integration | SMTP submission into the selected test mail server, whitelist add/remove/empty list, forged sender rejection, invalid subject/MIME, sender rejection notice even outside the whitelist, duplicate/loop protection, no fax from body/HTML/inline signatures, restart handling, report failure without another fax. |
| FreeSWITCH | Send and receive a real fixture through the pinned engine. Cover G.711, T.38 negotiation, refusal/fallback, busy, partial pages, cancellation and lost ESL connection. SIPp signaling alone does not prove document transfer. |
| Operations | Compose validation, amd64/arm64 image builds, converter availability, non-root volume access, quota/retention, backup and restore with fax fixtures. |

Run the exact Rust and web commands in `.github/workflows/ci.yml`, including
`cargo fmt --all --check`, locked clippy and workspace tests, plus web lint,
check, tests and build. Run applicable image and end-to-end workflows.
Record local results, CI results and unavailable live-provider checks
separately. Do not describe a planned test as passed.

## Dependencies, migrations and release handoff

This proposal needs no other open PR, runtime dependency or migration.
The verified base contains roles, mail templates, notifications and storage
targets. Recheck the remote and concurrent PRs before implementation.

Implementation needs PDF/TIFF utilities, image decoding and a MIME/IMAP client if the
recommended mailbox transport is selected. Check versions, licenses,
image size and amd64/arm64 support before adding them. Direct SMTP gateway
mode has a different deployment dependency and needs its own review decision.

Allocate migration IDs only when implementation starts. The greatest current
ID is `20261105000001`; do not choose a lower timestamp from today's date.
Use unique IDs after the verified migration sequence. Record every actual
ID and upgrade effect in the implementation PR. Add an ADR after feedback;
do not reserve a shared ADR number in this proposal.

The feature is disabled by default on upgrade. Existing numbers retain
their routes. New enum values require the upgraded API/UI and must not be
used with an older binary. Document this forward-only compatibility limit.

Suggested release notes after implementation and verification:

> Add PDF fax send and receive over SIP. Add saved fax headers and footers,
> own and shared fax numbers, role permissions, mail-to-fax submission and
> delivery reports. Add fax document backup and retention controls.

This proposal alone has no user feature release note.

## Feedback required before implementation

Confirmed mail rules: a number-only subject, whitelisted sender, rejection
notice to the sender, PDF/image attachments only, no mail body content, and
the sender email address in the fax header. The original caret was a typo.

1. Confirm or change the remaining scope and default behavior above.
2. Select mail input: existing SMTP mailbox with IMAPS retrieval
   (recommended), or direct authenticated SMTP gateway to TalkOps.
3. Confirm dedicated incoming fax routes (recommended), or request shared
   voice/fax detection on an existing voice number.

## Sources

- [FreeSWITCH fax source at the deployed v1.10.12 tag](https://github.com/signalwire/freeswitch/blob/v1.10.12/src/mod/applications/mod_spandsp/mod_spandsp_fax.c).
- [SignalWire fax and T.38 manual](https://developer.signalwire.com/freeswitch/applications/fax/).
- [Poppler project](https://poppler.freedesktop.org/).
- [libtiff tiffcp documentation](https://libtiff.gitlab.io/libtiff/tools/tiffcp.html).
- [Postfix SMTP access controls](https://www.postfix.org/SMTPD_ACCESS_README.html)
  and [SASL authentication](https://www.postfix.org/SASL_README.html).
- Project ADRs 0002, 0005, 0006, 0007, 0008, 0029, 0034, 0035 and 0036.

Language review: clear English review using `docs/documentation-style.md`.
The official ASD-STE100 standard was not available. Full review is pending.
