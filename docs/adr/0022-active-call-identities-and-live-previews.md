# 0022 - Active call identities and local transcript previews

- Status: Accepted
- Date: 2026-10-12
- Extends: [0012](0012-aufzeichnung-und-transkription.md) and
  [0019](0019-transkription-in-zwei-durchgaengen-und-ki-api.md)

## Context

FreeSWITCH call rows can show device logins or provider caller IDs.
These values do not identify the extension or call direction reliably.
The user requested phonebook names and a live transcript popup.
Existing transcription jobs start after call completion.

## Decision

Read TalkOps routing variables through `uuid_dump`. Filter calls by tenant.
Resolve the local extension and trunk number separately. Match all contact
sections and all three phone fields with tenant dialing settings. Keep internal
identities separate from external numbers. Do not select one ambiguous name.

Request previews through authenticated POST requests. Check current calls and
content permissions on each request. Admins can read all tenant calls.
Operators can read calls of their own extensions. Audit the first request and
popup close. Replies use `no-store`. The popup does not start recording.

Use the existing PostgreSQL queue with a separate `transcribe.live` lane.
Allow four active previews per tenant and one pending job per viewer and call.
Read at most eight seconds of PCM16 stereo. Keep 500 ms behind the writer.
Do not trust the unfinished WAV length. Start at the last eight seconds.
Keep only 200 segments. Use local `base` with at most two threads and a
60-second timeout. Cancel subprocesses on timeout. Final transcription keeps
its selected engine and quality. Live audio never goes to the external API.

Expire job eligibility after 30 seconds without requests. Delete text on close.
A running job cannot save into a deleted preview. Delete abandoned previews
during new requests and hourly retention. Cleanup can take up to one hour.

## Consequences

Migration `20261026000003_live_transcript_previews` adds a temporary content
table. Update server and worker together. No new package or service is needed.
The pinned `base` model downloads on first use if necessary. Work consumes
extra CPU and memory. Separate lanes do not reserve CPU. Text is provisional
and can lag behind speech. Snapshots can split words. The final complete-file
pass remains the source for search and export.

Speech times use answer time plus sample offsets. Queue setup and transfers
can affect this approximation. The preview does not provide diarization.

## References

- [FreeSWITCH stereo recording](https://developer.signalwire.com/freeswitch/media-and-codecs/audio-files/)
- [FreeSWITCH caller time headers](https://github.com/signalwire/freeswitch/blob/v1.10.12/src/switch_caller.c)
