# 0027 - Shared phonebook numbers in active calls

- Status: Accepted
- Date: 2026-10-15
- Supersedes: The rule "Do not select one ambiguous name" in
  [ADR 0022](0022-active-call-identities-and-live-previews.md)

## Context

One external number can be in more than one phonebook section. For example,
two families keep the same company in their own sections with different names.
ADR 0022 showed no name for such a number. The dashboard showed only the
label "Multiple phonebook entries". Users could not see the person or the
entries. The user requested one name in the call row, a hover card with all
entries, and a link to each entry.

## Options

1. Keep the label without a name (ADR 0022).
2. Show the first entry in alphabetical order.
3. Prefer an entry from a phonebook section of the local extension in the
   call. Use the first entry in alphabetical order when no such entry exists.

## Decision

Use option 3. The local extension owns these sections:

- The sections that the provisioned phones of its devices show
  (`phones.phonebook_sections`).
- A section with the same name as the display name of the extension
  (case-insensitive).

The API party object keeps `ambiguous`. It adds `contact_id` (the entry
that supplies the name) and `matches` (all entries when `ambiguous` is true).
Each match has the contact ID, the name, the section ID and the section name.
The list starts with global entries, then sorts by section and name.

The web UI shows the name, the number and a "+N more" button. A click on
the button or the pointer on the party opens a card with three columns:
phonebook, entry and "Go to entry". The link opens `/phonebook?contact=<id>`. The phonebook page
selects the section of the entry, scrolls to the entry and highlights it.
With one match, the name is the link and no card opens.

The card is a custom component with `popover="manual"` and fixed coordinates.
It does not use interest invokers or CSS anchor positioning. Browser support
for these features is limited, and a polyfill adds a dependency.

## Consequences

No migration is necessary. The API change only adds fields.
The overview loads phonebook sections and phones once per request in
addition to the old queries. TalkOps supports up to approximately
50 extensions, so the extra load is small.

The name for a shared number is a selection, not a certainty. The "+N more"
marker and the card show that other entries exist. Users who can read the
dashboard can also read the phonebook, so the card shows no new data.
