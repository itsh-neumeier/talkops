# Active calls

The dashboard shows direction and local extension in the first line.
The second line shows the remote party. Names come from all phonebook sections
and all three number fields. National and international formats match the
same contact. Numbers use square brackets:
`Family Vollmeier (Maria) [+4995458589]`. Ambiguous matches show a label.

Status and duration remain on the right. Talk time starts at answer.
Before answer, the timer shows waiting time. Long names wrap on small screens.

## Live transcript

Select **Live transcript** for an answered, recorded call with transcription
enabled. Admins can read all tenant calls. Operators can read calls of their
own extensions. The popup never starts a recording.

The worker uses the local `base` model with at most two threads. It downloads
the pinned model on first use if necessary. The preview starts with the last
few seconds and adds new contributions with speaker labels and speech times.
New text has a short typing animation. Reduced motion disables the animation.

Text is provisional and can lag behind speech. Each job reads at most eight
seconds. Each tenant can have four previews. The popup keeps the last 200
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
[implementation](../adr/0022-active-call-identities-and-live-previews.md).
