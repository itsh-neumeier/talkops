# Active calls

The dashboard shows direction and local extension in the first line.
The second line shows the remote party. Names come from all phonebook sections
and all three number fields. National and international formats match the
same contact. Numbers use square brackets:
`Family Vollmeier (Maria) [+4995458589]`. Select a name from the phonebook
to open the phonebook. The phonebook shows the section of the entry and
highlights the entry.

### Numbers in more than one phonebook entry

One number can be in more than one phonebook section. The row then shows one
name and the button **+N more**. TalkOps selects the name in this sequence:

1. An entry in a phonebook section of the local extension in the call.
   These are the sections that the provisioned phones of the extension show,
   and a section with the same name as the extension.
2. The first entry in alphabetical order.

Put the pointer on the party or select **+N more** to see all entries.
The card has three columns: **Phonebook**, **Entry (name)** and
**Go to entry**. The label **shown** identifies the entry in the row.
Press Esc to close the card.

Status and duration remain on the right. Talk time starts at answer.
Before answer, the timer shows waiting time. Long names wrap on small screens.

## Live transcript

Select **Live transcript** for an answered, recorded call with transcription
enabled. Admins can read all tenant calls. Operators can read calls of their
own extensions. The popup never starts a recording.

By default, the worker uses the local `base` model with at most two threads.
It downloads the pinned model on first use if necessary. The engine of the
*Live preview* in the
[transcription settings](recording.md#engines-per-purpose) can select the
local *Fast* model or a cloud model. A cloud model can also stream over the
Realtime API, with about one second of delay
([live preview in the cloud](recording.md#live-preview-in-the-cloud)). The preview starts with the last
few seconds and adds new contributions with speaker labels and speech times.
New text has a short typing animation. Reduced motion disables the animation.

Text is provisional and can lag behind speech. Without streaming, each job
reads at most eight seconds. Each tenant can have four previews. The popup keeps the last 200
segments. Use the final transcript for complete text and search.

If the call is transferred to another party, reopen the popup. The old
preview stops so that its speaker names do not identify the new party.

Close the popup to stop requests and delete temporary text. Abandoned previews
stop work after 30 seconds without requests. Cleanup deletes abandoned text
within one hour. Final transcripts keep normal retention.

If no text appears, check recording, transcription, the worker, model download
access, and preview capacity. Silence can produce no text. Update server and
worker together. Run migrations before use.

See [recording settings](recording.md) and
implementation ([ADR 0022](../adr/0022-active-call-identities-and-live-previews.md),
[ADR 0024](../adr/0024-transcription-engines-per-purpose.md)).
