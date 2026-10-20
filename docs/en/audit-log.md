# Audit log

[Handbook](index.md) · [Deutsch](../de/audit-log.md)

## Before you start

You need: audit.view for the log; audit.manage for retention and export configuration.

TalkOps writes an audit entry for each administrative change. An entry has
the time, the user, the IP address, the action, the object and details. The
details never contain passwords or keys.

Open **Settings → Audit log**. This needs the permission `audit.view`.

## Filter and sort

- **From** and **To**: days in local time. **To** includes the full day.
- **User**, **Action** and **Object**: lists of the values in the log.
- **Search**: text in the object ID or in the details. Upper and lower case
  are the same.

Click a column heading to sort by time, user, action or object. Click it
again to change the direction. The list shows 50 entries on each page and the
number of all matching entries.

## Download

**CSV** and **JSON** download all entries that match the filter, in the
selected sort order, without pages. One download contains at most 100,000
entries. Make the filter narrower if more entries match. Each download writes
an audit entry with the format, the number of entries and the filter.

- CSV: RFC 4180 with a header line and UTF-8. The column `details` contains
  JSON. A cell that starts with `=`, `+`, `-` or `@` gets a leading `'`, so a
  spreadsheet does not run it as a formula.
- JSON: an array of entries.

## Retention and automatic export

The tab **Retention and export** needs the permission `audit.manage`. Users
also need `audit.view` to open the page.

**Retention in the database**: TalkOps deletes entries that are older than
the set number of days, once an hour. `0` keeps all entries (default). While
the automatic export is on, TalkOps deletes only entries that an export file
already contains.

**Automatic export** writes new entries to a
[storage target](backup.md#backup-targets) (SMB, S3, WebDAV, SFTP, FTP or a
local folder):

1. Add a storage target under **Settings → Storage**.
2. On the tab **Retention and export**, select the storage target, a
   sub-folder (default `audit`) and the format.
3. Set the schedule and the number of export files to keep. Then click
   **Save**.

Each run writes all entries after the last exported entry into a new file:

- `talkops-audit-YYYYMMDD-HHMMSS-FIRST-LAST.jsonl` (JSON Lines: one entry on
  each line) or `.csv`. `FIRST` and `LAST` are the entry IDs in the file.
- `<file>.sha256` next to it, in the format of `sha256sum`. TalkOps also shows
  the checksum in the list of export files.

A run without new entries writes no file. One file contains at most 50,000
entries; the next run continues. **Export now** starts a run immediately.
The status shows the last run, the last error and the next run.

Check a file on the storage target:

```sh
sha256sum -c talkops-audit-20261016-021500-1-120.jsonl.sha256
```

The file retention uses the rules of backup jobs (see
[Smart retention](backup.md#smart-retention)). It deletes only TalkOps
export files and their checksum files in the folder.

A storage target that the export uses cannot be deleted. Remote storage
targets need no backup password for the audit export. The files are not
encrypted by TalkOps; use a storage target with encrypted transport (SFTP,
FTPS, HTTPS, S3 with TLS or SMB 3).

## API

- `GET /api/v1/audit`: parameters `since`, `until`, `user_id`, `action`,
  `entity_type`, `search`, `sort` (`time`, `user`, `action`, `entity`),
  `order` (`desc`, `asc`), `limit` (1 to 500), `offset`. The header
  `X-Total-Count` has the number of matching entries.
- `GET /api/v1/audit/facets`: actions, object types and users in the log.
- `GET /api/v1/audit/export?format=csv|json` with the same filter.
- `GET` and `PUT /api/v1/audit/settings`, `POST /api/v1/audit/export/run`
  (`audit.manage`).

See [ADR 0037](../adr/0037-audit-export-and-retention.md).

## Check the result

Make a harmless audited change and inspect its actor, action and permitted export.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
