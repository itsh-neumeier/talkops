# Voicemail

Every extension can have a voicemail box. Unanswered, busy and do-not-disturb
calls go there; new messages light up the phone's message lamp (MWI), and an
e-mail with the recording can be sent.

## Setup

**Voicemail → Voicemail settings** (your own extensions) or, as an admin, on
the extension page:

- Turn **Voicemail enabled** on.
- **PIN** (4–10 digits) – needed to check the box from another phone.
- **Prompt language**: German or English (default from the settings).
- **Greeting**: *Default*, *Generate*, *Record*, *Upload* or *None* (callers
  hear the beep right away) – see below. You can also record on the phone:
  dial `*97` and press **5**.
- **Send by e-mail** – to the e-mail address of the extension's user,
  optionally with the recording attached as WAV. With transcription switched
  on, the mail also contains the text of the message (see
  [recording & transcription](recording.md)).

## Greetings and audio

Wherever TalkOps plays a prompt – voicemail, Smart Attendant, queues – you get
the same choice:

- **Generate:** enter text, pick language and voice (two per language: German
  *Thorsten* / *Kerstin*, English *Linda* / *Joe*), click **Generate** and
  listen. The computer voice (Piper) runs locally in the media worker; nothing
  leaves the server. If you change the text and save without generating again,
  TalkOps generates it on save.
- **Record:** right in the browser with the microphone (up to 10 minutes).
- **Upload:** WAV, MP3, OGG or M4A; the browser converts the file to 16 kHz
  mono.

The player shows the waveform, seeks on click and plays at 1×, 1.5× or 2×.
Audio no longer in use is deleted after a day.

## Listening on the phone

| Dial | Function |
|---|---|
| `*97` | own mailbox (from the extension's own phone, no PIN) |
| `*98` | any mailbox: extension + `#`, PIN + `#` |

Main menu: **1** listen to messages, **5** record greeting, **\*** exit.
During a message: **1** repeat, **7** delete, **9** save, **#** next message.
When leaving a message, **#** ends the recording.

## In the browser

**Voicemail** lists all messages with player, download, "mark as heard" and
delete. Playing a message marks it as heard and updates the phone's lamp.

## E-mail (SMTP)

**Settings → E-mail (SMTP)** (admin): server, port, encryption (STARTTLS 587
or TLS 465), credentials and sender, e.g. `TalkOps <phone@example.com>`.
**Send test mail** checks the configuration. The password is stored encrypted.

## Notes

- The `media-worker` service renders the prompts on its first start (about
  20 s). Without it voicemail still works, just without spoken prompts.
- Messages shorter than one second (caller hung up during the greeting) are
  discarded; the maximum length is set per box (default 3 min).
- Recordings live in the `voicemail` volume – include it in your backups.
