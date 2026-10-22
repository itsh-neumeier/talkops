# Voicemail

[Handbook](index.md) · [Deutsch](../de/voicemail.md)

## Before you start

You need: An owned mailbox; voicemail.access_all for other mailboxes and voicemail.settings for system texts.

Every extension can have a voicemail box. Unanswered, busy and do-not-disturb
calls go there; new messages light up the phone's message lamp (MWI), and an
e-mail with the recording can be sent.

## Setup

**Voicemail → Voicemail settings** (your own extensions) or, as an admin, on
the extension page:

- Turn **Voicemail enabled** on.
- **Voicemail answers after** … seconds (5–300, about 5 s per ring) – also
  used for forwarding on no answer.
- **PIN** (4–10 digits) – needed to check the box from another phone.
- **Prompt language**: German or English (default from the settings).
- **Greeting**: *Default*, *Generate*, *Record*, *Upload* or *None* (callers
  hear the beep right away) – see below. You can also record on the phone:
  dial `*97` and press **5**.
- **Send by e-mail** – to the e-mail address of the extension's user,
  optionally with the recording attached as WAV. With transcription switched
  on, the mail also contains the text of the message (see
  [recording & transcription](recording.md)). To change the design and the
  text of the mail, see [e-mail design and texts](mail.md).

## Speech speed

Set **Speech speed** in the settings of each voicebox. The range is 0.5× to
2.0×. The default, 1.0×, keeps the previous voice speed. Lower values are
slower. This setting controls generated greetings, menu prompts, spoken
numbers, and caller and date announcements. Recordings and uploaded audio
keep their original speed.

An admin can set a speed for each prompt and language in **Settings →
Voicemail**. An empty field uses the voicebox speed. An explicit 1.0 override
uses normal speed, even if the voicebox uses another speed. **Speed for
numbers and digits** controls the numbers in prompt sequences.

To change the speed of part of a TTS text, use an inline tag:

```text
Welcome. <speed:0.7>Please have your customer number ready.</speed> Thank you.
```

The tagged text uses 0.7×. The selected prompt or voicebox speed resumes
after the closing tag. Tag values are absolute factors, from 0.5 to 2.0.
Use a decimal point. Do not nest tags or leave a tagged section empty.
TalkOps rejects invalid tags when you save or generate the text.

The shared **Generate** editor also has a speech speed control and a preview.
For a voicebox greeting, the preview uses that voicebox's speed. If you change
the text or speed, generate a new preview to hear the result. Saving also
generates the changed clip. Other users of a shared clip keep their clip.

The media worker generates changed prompts and stored message announcements
in the background. Standard prompts can play until new prompts are ready.
The message recording keeps its original speed.

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

Main menu (default): **1** listen to messages, **5** record greeting,
**\*** exit. During a message: **1** repeat, **7** delete, **9** save, **#**
next message. When leaving a message, **#** ends the recording.

For two new messages out of four total messages, the German overview says
"2 von 4 Sprachnachrichten sind neu." The English overview says "2 of 4
voice messages are new." Empty boxes, single messages, and boxes with only
new or saved messages have their own prompt forms.

Before each message TalkOps says its position, who called, and when.
For example: "Message 1 of 4 from
Anna Müller, 0 3 0, 1 2 3, 4 5 6. Received on Thursday, October 8, at 2:32 PM."
The name comes from the phone book or the extension list, otherwise – if
enabled – from the provider (CNAM). The announcement is made when the message
is stored; if it is not ready (yet), TalkOps reads only the number.

### Customizing

*Settings → Voicemail* (admin):

- **Keys:** put every function of both menus on any key (0–9, \*, #; each key
  once per menu). The menu prompts name the chosen keys automatically.
- **Announcement before each message:** name from the phone book, number
  (digit by digit, in groups), name sent by the provider, date and time –
  each can be switched off.
- **Voice:** one of the bundled voices per language. The language a mailbox
  speaks is set at the extension's mailbox.
- **Prompt texts:** every voicemail prompt per language can be reworded;
  *Default* restores the original. Placeholders: `{listen}` `{greeting}`
  `{exit}` in the main menu, `{repeat}` `{delete}` `{save}` `{next}` in the
  message menu, `{caller}` and `{date}` in the announcement before each
  message.

The media worker renders changed prompts in the background (seconds to a few
minutes); until then the previous prompt plays.

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

## Check the result

Leave a test message and confirm greeting, browser playback and configured email delivery.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
