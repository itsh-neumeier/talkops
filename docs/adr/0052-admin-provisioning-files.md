# 0052 - Admin-edited provisioning files

- Status: Accepted
- Date: 2026-10-30
- Extends: [0049](0049-cisco-enterprise-tftp-bootstrap.md),
  [0051](0051-cisco-enterprise-service-control-ringtones-directory.md)

## Context

TalkOps generates all provisioning files from the database. The TFTP exporter
writes the Cisco Enterprise files again every 30 seconds and after each change.
The user needs these functions:

- Look at the files that a phone gets.
- Change a generated file for a test or a setting that TalkOps does not
  support yet.
- Add files that TalkOps does not generate, for example the Cisco network
  locale `g3-tones.xml`.

A changed file in the export directory is overwritten at the next export.
A copy of a generated file can contain SIP passwords.

## Options

1. A read-only file view only.
2. Stored overrides only.
3. One table of stored files. A file with the name of a generated file
   replaces it. A file with another name is an extra file.

## Decision

Option 3. Table `provisioning_files` with transport `http` or `tftp`, an
optional phone (TFTP only) and a path.

- HTTP: the provisioning endpoint looks for a stored file at the requested
  path after the credential check and serves it instead of the generated
  file. The path rules keep firmware, media, events and the Cisco directory
  out.
- TFTP: the exporter adds the stored files after the generated files. Files
  for every phone come first, then the phone's own files, so the phone's own
  file wins. The names must be names that the TFTP map serves.
- The content is encrypted with `TALKOPS_SECRET_KEY` (ADR 0008). The limits
  are 1 MiB per file and 500 files per tenant.
- The UI shows the generated files of a phone, opens a generated file as the
  start of a new stored file and lists the stored files. Removing a stored
  file returns to the generated file.

## Consequences

- An admin can fix or extend a phone without a new release.
- A stored copy of a generated file does not follow later changes in TalkOps.
  The UI marks such a file as "Replaced" and shows a warning.
- A stored HTTP file is served without the phone-specific side effects of the
  generated file, for example the "last contact" update.
