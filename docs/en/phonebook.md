# Phone book

[Handbook](index.md) · [Deutsch](../de/telefonbuch.md)

The phone book stores shared contacts. The internal directory comes from
extensions. A phone book section selects contacts for particular provisioned
phones; it is not a private user address book or an access-control boundary.

## Before you start

Sign in to read the shared phone book. Editing, deleting, section management
and CSV import require `phonebook.edit`. Phone assignments require
`phones.manage`. See [users and roles](users-and-roles.md).

## Add and find a contact

1. Open **Phone book → New contact**.
2. Enter a name, optional company and work, mobile or other number.
3. Select a section, or leave **Global (all phones)**.
4. Save and search for the name or number.

Use international numbers such as `+49891234567` where practical. An internal
contact can use `**101` to force internal dialing. The search also matches
company and section. Column filters select contacts with or without each number
type. Sort by name, company or section. Reset filters if a saved entry is hidden.
Deleting a section also deletes its contacts; review the confirmation.

## Import a CSV file

1. Open **Phone book → Import** and download the sample file.
2. Export contacts with a header row and comma, semicolon or tab separators.
3. Select the file and inspect the preview.
4. Select the section for rows without one.
5. Enable **Update existing contacts** only if existing values should change.
6. Review created, updated, skipped and failed row counts, then select **Import**.

The preview does not save contacts. The import recognizes name or first/last
name, company, work, mobile, other/home and section columns, including common
Outlook and Google exports. Use UTF-8; the browser also accepts Windows-1252.
The same name in the same section identifies an existing contact. New section
names in the file create sections. Failed rows are skipped and reported with
line numbers. Correct the failed rows before importing again.

Expected result: the imported names appear in the selected section. Use a
small sample first when updating an existing phone book.

## Phone directories, caller names and favorites

Global contacts and the internal directory are available to supported
provisioned phones. Assign additional sections on a phone's detail page.
Yealink supports up to three additional sections in this UI.
Device directory formats and support vary by model; see
[Yealink directories](yealink.md#5-phonebook) and
[manufacturer settings](manufacturer-provisioning.md).

Call views can resolve numbers to contacts and link to the matching entry.
The [softphone](softphone.md) offers contacts and favorites for dialing.
Favorites are not a separate phone book section.

To verify, search for a unique contact in the browser, then refresh the directory
on a supported configured phone. If it is missing, check section assignment,
directory URL, provisioning credentials and model support. Section selection
does not make shared contact data confidential in the web interface.
