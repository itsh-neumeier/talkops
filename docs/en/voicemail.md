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
- **Greeting**:
  - *Standard greeting* – "The person you are calling is not available …"
  - *Text* – read by the computer voice (Piper, runs locally); generating it
    takes a few seconds after saving, then it can be played in the browser.
  - *Own recording* – dial `*97` on the phone and press **5**.
- **Send by e-mail** – to the e-mail address of the extension's user,
  optionally with the recording attached as WAV. With transcription switched
  on, the mail also contains the text of the message (see
  [recording & transcription](recording.md)).

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
