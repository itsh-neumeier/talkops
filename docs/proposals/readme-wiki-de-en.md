# README and bilingual handbook proposal

Status: Proposed. User feedback is required before implementation.
Review base: `b1fa28350dc1830e222a27ee770641edb1935ccc` on `origin/main`.
Review date: 2026-10-20 (Europe/Berlin).

## Review scope

This proposal defines the README, handbook structure and GitHub Wiki delivery.
It includes sample text for review. This PR adds only the proposal and its task
summary. The current README, user guides, application and live Wiki stay as they
are until the user reviews the proposal.

The user requested this review before changes. After approval, implement the
agreed scope on this branch and keep the PR in draft until checks pass.
The release chat owns merges, version changes, tags and publication.

## Current state and corrections

- The remote default branch is `main`. The base contains TalkOps version 1.8.5
  in [Cargo.toml](../../Cargo.toml) and [CHANGELOG.md](../../CHANGELOG.md).
- [README.md](../../README.md) still says `Status: 1.0`. Its feature list describes
  Yealink provisioning but does not explain the current multi-vendor support.
- The README has one long inline list of guide links. It has no German README.
- There are 25 Markdown user guides in each of `docs/en/` and `docs/de/`.
  The table below maps every current pair. Several main UI areas need a guide.
- [First steps](../en/first-steps.md) says Yealink provisioning follows in phase 2.
  Provisioning already exists. Replace this statement with current instructions.
- The installation guide uses `1.0` as a version example. Replace stale examples
  and verify the relation between Compose files, image tags and update steps.
- The backup introduction omits fax from its volume list. The current
  [backup implementation](../../crates/talkops-api/src/backup.rs) includes fax.
  Correct the EN and DE introductions after checking the complete procedure.
- GitHub reports `hasWikiEnabled: true`. A read of the Wiki Git remote returned
  `Repository not found`. This does not prove that the Wiki is empty. Check
  access and initialization before publication. No live Wiki content was read.
- No open PR was returned by `gh pr list` at the time of inspection.
  Check again before implementation; concurrent work can change this result.

## Proposed README

Keep `README.md` as the English entry point. Add `README.de.md` with the same
structure in German. Put language links at the top of both files.
Use a concise introduction and link to the handbook for detailed procedures.

Proposed section order:

1. Language selection, project name and one short product description.
2. Current capabilities, grouped by calls, devices, media and administration.
3. Requirements: Linux, Docker Compose, network ports and supported architectures.
4. Quick start: a new instance, required secrets, start and health check.
5. Handbook entry points for administrators and daily users, in DE and EN.
6. A small architecture table and links to architecture and ADRs.
7. Development, support through GitHub issues, contribution rules and license.

Replace the fixed `Status: 1.0` paragraph with links to releases and the
changelog. Keep hardware/provider verification limits beside the affected
feature. Source-template coverage must not imply successful hardware tests.
Do not add unverified support claims or a repeated manual to the README.
Keep existing contributor workflow links, but shorten their introduction.

### English opening sample

> # TalkOps
>
> Deutsch: README.de.md · English: README.md
>
> TalkOps is a self-hosted phone system for homelabs and small businesses.
> It combines SIP calls, phone provisioning, browser calls, voicemail, fax,
> door stations and local speech processing in a Docker Compose stack.
>
> Start here: Installation · Administrator handbook · User handbook
>
> Run TalkOps on a Linux host with Docker Engine and Docker Compose.
> See Releases and the Changelog for the current version and upgrade notes.

### German opening sample

> # TalkOps
>
> Deutsch: README.de.md · English: README.md
>
> TalkOps ist eine selbst gehostete Telefonanlage für Homelabs und kleine
> Unternehmen. Sie verbindet SIP-Telefonie, Telefon-Provisionierung,
> Browser-Telefonie, Voicemail, Fax, Türsprechstellen und lokale
> Sprachverarbeitung in einem Docker-Compose-Stack.
>
> Hier starten: Installation · Administrationshandbuch · Benutzerhandbuch
>
> TalkOps läuft auf einem Linux-Host mit Docker Engine und Docker Compose.
> Die aktuelle Version und Upgrade-Hinweise stehen unter Releases und Changelog.

These samples show the proposed wording and order. Implementation will replace
the plain navigation labels with tested links. The quick start will describe
only a new instance. It will link to recovery before credential generation.
Commands will use an agreed released version for both configuration and images.
Keep `latest` as an explained option. Do not choose a new release version here.

## Proposed handbook structure

Add `docs/en/index.md` and `docs/de/index.md`. Each index gives two reading paths:
administrators set up and operate the system; users make calls and use messages.
All pages include a language switch and a link back to the appropriate index.

Use the following order in both languages. Keep current filenames and links.
Each existing pair becomes two Wiki pages. For example, `docs/en/fax.md` maps
to `EN-Fax`, and `docs/de/fax.md` maps to `DE-Fax`.

| Section | Existing English source | Existing German translation | Work after approval |
| --- | --- | --- | --- |
| Setup | [Installation](../en/installation.md) | [Installation](../de/installation.md) | Verify requirements, secrets, ports, HTTPS and current version examples. |
| Setup | [Portainer](../en/portainer.md) | [Portainer](../de/portainer.md) | Verify stack files, mounts and update instructions. |
| Setup | [First steps](../en/first-steps.md) | [Erste Schritte](../de/erste-schritte.md) | Remove phase text; describe a first working call. |
| Identity | [Users and roles](../en/users-and-roles.md) | [Benutzer und Rollen](../de/benutzer-und-rollen.md) | Explain administrator and user access with current UI terms. |
| Identity | [Sign-in](../en/sign-in.md) | [Anmeldung](../de/anmeldung.md) | Check TOTP, OIDC, LDAP and role mapping. |
| Telephony | [LEONET](../en/leonet.md) | [LEONET](../de/leonet.md) | Preserve provider instructions and test limits. |
| Telephony | [Call routing](../en/call-routing.md) | [Anrufsteuerung](../de/anrufsteuerung.md) | Explain groups, schedules, attendants, queues, parking and blocking. |
| Devices | [Provisioning](../en/provisioning.md) | [Provisionierung](../de/provisionierung.md) | Explain transport, templates, keys and diagnostics. |
| Devices | [Manufacturer provisioning](../en/manufacturer-provisioning.md) | [Hersteller-Provisionierung](../de/hersteller-provisionierung.md) | Explain model settings, copies, file delivery and unsupported profiles. |
| Devices | [Yealink](../en/yealink.md) | [Yealink](../de/yealink.md) | Retain detailed phone setup, BLF, feature codes and directory behavior. |
| Devices | [Codec profiles](../en/codec-profiles.md) | [Codec-Profile](../de/codec-profile.md) | Check audio, video and Wi-Fi examples. |
| Daily use | [Softphone](../en/softphone.md) | [Softphone](../de/softphone.md) | Cover identities, calls, transfers, audio, video and TURN. |
| Daily use | [Active calls](../en/active-calls.md) | [Laufende Gespräche](../de/laufende-gespraeche.md) | Explain dashboard and live call information. |
| Daily use | [Call log](../en/call-log.md) | [Anrufliste](../de/anrufliste.md) | Explain filters, blocked reasons and whitelist permissions. |
| Daily use | [Voicemail](../en/voicemail.md) | [Voicemail](../de/voicemail.md) | Cover greetings, speech speed, playback and delivery. |
| Daily use | [Fax](../en/fax.md) | [Fax](../de/fax.md) | Cover send, receive, mail input, preview, permissions and storage failures. |
| Media | [Recording](../en/recording.md) | [Aufzeichnung](../de/aufzeichnung.md) | Explain enablement, announcements, playback, search and retention. |
| Media | [Transcriptions](../en/transcriptions.md) | [Transkriptionen](../de/transkriptionen.md) | Explain jobs, models, resources, retries and estimates. |
| Integrations | [Door stations](../en/door-station.md) | [Türsprechstellen](../de/tuersprechstelle.md) | Check ring targets, opening actions and Home Assistant. |
| Integrations | [Dahua](../en/door-station-dahua.md) | [Dahua](../de/tuersprechstelle-dahua.md) | Preserve VTO/VTH details and device verification limits. |
| Administration | [Mail](../en/mail.md) | [E-Mail](../de/e-mail.md) | Explain SMTP, mail appearance and message text. |
| Administration | [Notifications](../en/notifications.md) | [Benachrichtigungen](../de/benachrichtigungen.md) | Explain events, rules and delivery logs. |
| Administration | [Backup](../en/backup.md) | [Datensicherung](../de/datensicherung.md) | Include fax, encrypted configuration, targets and recovery checks. |
| Administration | [Monitoring](../en/monitoring.md) | [Monitoring](../de/monitoring.md) | Explain health, metrics, alerts and diagnostics. |
| Administration | [Audit log](../en/audit-log.md) | [Audit-Log](../de/audit-log.md) | Explain access, filters, export and retention. |

Add these missing pairs. Names below are proposed paths, not existing links:

| Proposed EN / DE filenames | Content and completion example |
| --- | --- |
| `overview.md` / `ueberblick.md` | Architecture for operators, terms, reading paths and tested versus untested capabilities. |
| `trunks-and-numbers.md` / `trunks-und-rufnummern.md` | Generic provider setup, accounts, incoming numbers, caller ID, default number and emergency routing. Reference LEONET and the preset catalog. |
| `extensions-and-devices.md` / `nebenstellen-und-geraete.md` | Users, extensions, devices, credentials, DND and forwarding; confirm an internal call. |
| `phonebook.md` / `telefonbuch.md` | Contacts, visibility, favorites, search and device directories; check a contact on the permitted devices. |
| `prompts.md` / `ansagen.md` | Generate, record, upload and select prompts; confirm playback in the target call flow. |
| `system-settings.md` / `systemeinstellungen.md` | Group settings by purpose; explain defaults, required rights and effects of changes. Reference detailed guides. |
| `updates.md` / `aktualisierung.md` | Version selection, backup, maintenance window, migrations, Compose mounts, health checks and recovery limits. |
| `troubleshooting.md` / `fehlerbehebung.md` | Symptom-based routes for registration, missing/one-way audio, WebRTC, fax, provisioning, mail and restore failures. |
| `reference.md` / `referenz.md` | Glossary, port and environment-variable references, feature-code links, API entry point and support evidence. |

This gives 34 topic pairs plus two language indexes. The Wiki also has `Home`,
`_Sidebar` and `_Footer`. The index offers a complete topic list; the sidebar
links to the language indexes and main sections to keep navigation short.

### Page format and completeness

Each procedure states its purpose, required role, prerequisites, numbered
steps, expected result and common errors. Use current UI labels. Add fictitious
examples without credentials or personal data. Explain sensitive changes, such
as replacing a voice route with a fax destination, where the step occurs.

English is the source language. German covers the same procedures and examples.
Apply [documentation-style.md](../documentation-style.md) to English text.
The full ASD-STE100 standard is not available locally. Record a pending formal
language review rather than claiming certification.

Completeness means coverage of the current user and administrator workflows.
It does not mean a manual for every external phone firmware. Check the UI,
API, settings and existing tests against a chapter coverage list. Keep supported
configuration separate from hardware/provider validation. Use text first;
add screenshots only where they clarify a procedure and can be kept current.

## GitHub Wiki delivery and maintenance

Keep `docs/en/` and `docs/de/` as the only edited handbook sources. This keeps
translations, review and documentation changes in the application PR workflow.
The Wiki is a generated copy. Avoid independent manual edits to both locations.

Proposed implementation files:

- `docs/wiki/pages.json`: explicit source, language, page slug and navigation map.
- `scripts/docs/build-wiki.mjs`: generate flat Wiki Markdown into `build/wiki/`.
  Add this output directory to `.gitignore`. Do not require new npm packages.
- `tests/docs/wiki.mjs`: fixtures and checks for translation pairs, unique slugs,
  source coverage, Markdown links, anchors, images and reproducible output.
- `.github/workflows/ci.yml`: run `node tests/docs/wiki.mjs` and
  `node scripts/docs/build-wiki.mjs --check` after their implementation.
- `docs/wiki/README.md`: build, preview and publication instructions.

The generator will rewrite relative guide links to Wiki slugs. Preserve fragment
links and rewrite links to source-only files, such as ADRs, to repository URLs
at the reviewed source commit. Copy local images with collision-free paths.
Reject unresolved local targets and duplicate page names. Every published page
will show its source link, language counterpart and source commit.

GitHub stores a Wiki in a separate Git repository. Only pushes to its default
branch become live. An initial page on GitHub precedes the documented clone
workflow. These properties are described in the official
[Wiki editing guide](https://docs.github.com/en/communities/documenting-your-project-with-wikis/adding-or-editing-wiki-pages).
GitHub also documents the special
[sidebar and footer files](https://docs.github.com/en/communities/documenting-your-project-with-wikis/creating-a-footer-or-sidebar-for-your-wiki).

Publication plan for the release chat:

1. Confirm access to the Wiki and inspect any existing pages and history.
2. If initialization is needed, create the first page after content approval.
3. Generate pages from the agreed source commit and review the generated diff.
4. Preserve unrelated Wiki pages. Stop on a conflict with an existing page slug.
5. Commit generated content in the Wiki checkout and push its verified default
   branch. Record both the source commit and the resulting Wiki commit.
6. Check Home, both language indexes, sidebar, language switches and sample
   procedures on GitHub. Record the published URLs and any failure.

Recommend an explicit release-chat publication step for the first delivery.
Do not add automatic Wiki pushes on every feature push. A main-repository PR
alone cannot publish the Wiki. Access/initialization and live publication remain
separate checks. An inaccessible Wiki must not be reported as published.

## Work order after feedback

1. Confirm the language layout, chapter scope and generated-Wiki approach.
2. Refresh the target and inspect concurrent documentation changes.
3. Write both READMEs and indexes; correct current guide facts.
4. Add the nine missing topic pairs and complete existing procedures.
5. Add the manifest, generator, tests and maintenance instructions.
6. Review language parity, source coverage and generated navigation.
7. Update this task summary and PR with exact check results and release notes.
8. Hand the ready PR and generated-Wiki instructions to the release chat.

## Acceptance and validation after implementation

- Every current guide and main UI workflow has an entry in both languages.
- Both READMEs give a short route to installation and the complete handbook.
- Existing guide paths continue to work. No change to runtime behavior, database
  migrations, release metadata or deployment defaults is part of this task.
- Manifest checks cover all source pages, EN/DE pairs and unique output slugs.
- Generator fixtures cover cross-language links, fragments, source-only links,
  images, missing targets, duplicate slugs and unchanged repeated builds.
- Check the generated Home, sidebar, footer and representative long pages.
- Check installation, first call, device provisioning, fax preview and backup
  instructions against source and existing tests. Report any unexecuted live
  procedure explicitly. Hardware and production recovery tests are separate.
- Run `git diff --check` and the new Node documentation checks. Application
  tests are required only if implementation changes application behavior.
  Existing repository CI remains the validation contract for the final PR.

## Feedback requested

Recommended scope: English `README.md` plus German `README.de.md`; the 34 topic
pairs above; repository sources with a generated Wiki; publication by the release
chat. The user can approve this scope or request changes before implementation.

No migration IDs, dependency upgrades or runtime compatibility changes are
proposed. Suggested release notes after implementation:

> Add English and German README entry points and a complete bilingual handbook.
> Add checked GitHub Wiki generation from the reviewed documentation sources.

Do not use those release notes for this proposal-only PR state.
