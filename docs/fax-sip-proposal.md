# PDF fax over SIP: proposal for user review

Status: Proposed. Implementation needs user feedback first.
Date: 2026-10-18 (Europe/Berlin).
Base: `origin/main`, `7beb63d5b8e3917b7621588413afcec2a8893456`.

## Goal and review boundary

Add PDF fax send and receive functions to TalkOps. Users can send from the
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

The subject contains exactly one international number after MIME decoding
and removal of outer whitespace. Example: `+491245235`. Proposed syntax:
`^\+[1-9][0-9]{1,14}$`. This checks E.164 shape, not whether a number is
allocated. Apply the configured destination policy after this check.
Do not extract a number from arbitrary subject text. Reject multiple
numbers, `Re:`, control characters and the literal `^` inside a number.
The user's `+49^1245235` example needs clarification before this is fixed.

Accept PDF attachments only, in MIME order. Ignore the mail body. Reject an
entire submission if it has no PDF, an unsupported document attachment or
an invalid PDF. Ignore ordinary inline signature images. Do not send a
partial set of attachments. Apply the same limits and conversion as web
uploads. Proposed limits: 20 MiB decoded PDFs, 50 pages, 10 attachments,
and a bounded 30 MiB raw MIME message.

Map a verified mail identity to an enabled TalkOps user. That user needs
both fax send and mail submission permissions. The mail server must restrict
this mailbox to authenticated submissions and bind sender addresses to the
authenticated account. A `From` header or address allowlist alone is not
proof of identity. Arbitrary Internet mail must not create paid fax calls.
The deployment guide must state this requirement and provide a tested
mail-server configuration. Mail input stays disabled by default.

Store a unique mailbox identity plus UIDVALIDITY and UID before marking a
message processed. Store a bounded message fingerprint for duplicate
detection. A database transaction creates the fax and conversion job before
the message is moved to Processed. Reconnects must not submit it again.
Invalid messages move to Rejected and appear in an input log. Send reports
only to the configured verified user address, never an arbitrary Reply-To.

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
| Core | Add fax settings, endpoints, grants, templates, documents, transmissions, attempts and mail receipts. Keep tenant-safe relations and typed errors. |
| Database | Add forward-only schema migrations. Extend the destination enum in a separate migration before any migration uses the new value. Add role keys without granting new chargeable actions to existing user/operator roles. |
| API | Add `/api/v1/faxes` for upload, preview, submit, history, files, reports, cancel and resend. Add settings and endpoint management routes. Register OpenAPI schemas and audit mutations. |
| Routing | Extend `trunks`, `numbering`, XML dialplan generation, destination validation and all routing selectors with the fax destination. Keep business rules in Rust. |
| Transmission | Add an API-side fax dispatcher and ESL result handling. Tag calls with fax and attempt IDs. Correlate final results and CDR data. |
| Worker | Add bounded PDF normalization, page composition, multipage fax TIFF creation and received TIFF-to-PDF conversion jobs. |
| Mail | Add the selected ingress adapter, duplicate protection and fax report types to the existing renderer. Keep secrets encrypted and out of logs. |
| UI | Add the Fax page, Fax settings, endpoint access controls, role labels, API types and both translations. |
| Operations | Add a `fax` volume to all three services, media paths, capacity checks, backup/restore, retention and deployment docs. |

Use Poppler for PDF rendering and libtiff tools for fax TIFF assembly and
received PDF conversion, subject to an image-level compatibility fixture.
Render at a supported monochrome fax resolution. Test page dimensions,
compression and multipage order against the pinned FreeSWITCH build.
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
`fax.manage` controls endpoints, shared grants, global templates and policy;
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
| Unit | Strict MIME subject parsing, number policy, template precedence, header/footer margins, limits, retry decisions and duplicate events. Include the literal caret example as a rejected case if strict E.164 is approved. |
| PostgreSQL | Migration from existing data, destination validation, every new domain query, tenant isolation, grants, revoked rights, transactional queue creation, concurrent claims and duplicate mail receipts. |
| API | Authorized and forbidden upload, send, settings, download, deletion and resend; invalid PDF; inaccessible global/own number; session and CSRF checks. |
| Conversion | Multipage portrait/landscape PDFs, small text, Unicode, no cropped content, visible header/footer, TIFF resolution/compression, corrupt/encrypted input, timeout and cleanup. |
| Browser | Upload → preview → send → status; inbox; role editor; hidden/disabled unauthorized controls; mobile and desktop layouts; German and English. |
| Mail integration | SMTP submission into the selected test mail server, trusted identity mapping, forged sender rejection, invalid subject/MIME, restart and duplicate handling, report delivery failure without another fax. |
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

Implementation needs new PDF/TIFF utilities and a MIME/IMAP client if the
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

1. Confirm or change the overall scope and default behavior above.
2. Select mail input: existing SMTP mailbox with IMAPS retrieval
   (recommended), or direct authenticated SMTP gateway to TalkOps.
3. Confirm that the subject must contain only a number such as
   `+491245235`. Is the `^` in `+49^1245235` a typing error?
4. Confirm dedicated incoming fax routes (recommended), or request shared
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
