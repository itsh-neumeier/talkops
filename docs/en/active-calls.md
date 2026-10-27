# Active calls

[Handbook](index.md) · [Deutsch](../de/laufende-gespraeche.md)

## Before you start

You need: calls.active for the dashboard; live transcript permissions are separate.

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

## Country flags and loading

Active and recent calls show a flag beside an inbound caller or an outbound
destination. The flag identifies the number range. It does not identify the
caller's current location. Internal, hidden and unresolved numbers have no
flag. Hover over a flag to read the country name. The name is also available
to screen readers. The softphone recent calls show flags for raw numbers
with an international `+` or `00` prefix.

The two dashboard call cards load independently. Each card shows placeholders
immediately during its first load, for at least 300 ms. Content then fades
in over 180 ms. Later polling keeps the current content visible. An empty
message appears only after a successful empty response. A failed request
shows an error and keeps existing content. Reduced motion removes the
minimum display interval and animations.

## Recent calls

The dashboard card **Recent calls** shows ended calls in the same format as
active calls. Names, square brackets and **+N more** operate in the same way.
The first line also shows the own number:

- Inbound: the number that the caller dialed, for example
  `Incoming call to extension: Family Zweig [101] via +4995431241`.
- Outbound: the number that the called party sees as the caller ID.
- Internal: no own number.

Older records without a stored number do not show **via**. Time and duration
are on the right. Missed inbound calls are red and show **Missed**.
Select **All calls** to open the [call log](call-log.md).

## Short lists on the dashboard

The cards **Calls in progress** and **Recent calls** show three entries.
Below the list, **Show N more** shows all entries. **Show less** collapses the
list again. **Recent calls** shows up to eight calls; **All calls** opens the
call log. A ringing call always stays visible, also in a collapsed list. The
browser keeps the open state of each card. In a private window, the lists
start collapsed.

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

### Live preview with Coral

With a Coral Edge TPU in the server, the *Live preview* can use
**Coral (this server)** with streaming
([Coral Edge TPU](recording.md#coral-edge-tpu-this-server)). The text appears
while people speak, and audio does not leave the server. Each line shows the
detected language (`DE` or `EN`). If Coral is not available, the popup shows
*Coral is not available now; local Whisper makes the preview* and the local
`base` model continues in 8-second parts. After 30 seconds TalkOps tries
Coral again. This is no error.

An administrator can enable **Publish complete words (experimental)** in
the live transcription profile when Coral Realtime is selected. Save the
engines and open a new preview. The default is off. The environment setting
`TALKOPS_CORAL_STABLE_PREVIEW` is only the default for direct service clients;
TalkOps sends the saved profile setting for each session.
This mode holds an unfinished word until more audio closes it or a speech
pause ends the turn. It uses the existing model context and turn detection.
It does not change final recording or voicemail transcription.

Allow about 4–5 seconds for this preview with the tested German model.
First text can take longer. This is a model-dependent estimate, not a
maximum delay guarantee. Recognition quality still needs improvement.
If a later result changes a published word, the stream reports an error
and TalkOps uses its existing local Whisper fallback. Disable the checkbox
and open a new preview to restore the previous preview mode.

For a production trial, use **Settings → System → Diagnostics → Coral
transcription diagnostics**. Start a five-minute capture before the call.
Keep the live preview open and download the report promptly. The report
contains technical events, model hashes, TPU state, processing times and
available fallback state. No audio is stored in the diagnostic buffer.
Recognized text requires explicit selection and permission to view all live
transcripts. Audio can be downloaded separately from a recorded call with
the existing recording permissions. See the
[production trial guide](../research/coral-production-trial.md).

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
[ADR 0024](../adr/0024-transcription-engines-per-purpose.md),
[ADR 0046](../adr/0046-coral-edge-tpu-speech-recognition.md)).

## Check the result

Place a test call and confirm the live view and its completed call-log entry.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
