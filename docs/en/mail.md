# E-mail: design and texts

[Handbook](index.md) · [Deutsch](../de/e-mail.md)

## Before you start

You need: voicemail.settings and SMTP server credentials.

TalkOps sends mails for new voicemail messages and for the SMTP test. Each
mail has an HTML part and a plain-text part. You set the design and the
texts in **Settings → E-mail**. You need the permission
`voicemail.settings`.

## SMTP

1. Enter the SMTP server, port, encryption, user name and password.
2. Enter the sender address, for example `pbx@example.com`.
3. Click **Save**.
4. Enter a test recipient and click **Send test mail**.

## Mail design

- **Instance name**: the name of this installation, for example
  "Office Springfield". Each subject then starts with `[Office Springfield]`.
  Every mail shows the name in the footer.
- **Sender name**: the display name next to the sender address.
- **Accent color**: the color of the line above the mail and of the links.
  TalkOps uses dark teal for links if the color is too light for white
  backgrounds.
- **Logo**: PNG, JPEG or GIF, at most 256 KB. The mail shows the logo at
  most 48 px high. The logo is part of the mail (inline image), so mail
  clients show it without a download from the internet.
- **Header text** and **footer text**: short texts above and below the
  message. You can use `**bold**`, `*italic*`, links and the placeholders
  of the instance (`{{ instance.name }}` …).

Each mail footer also shows the instance name, the host name, the public
URL and the line "Powered by TalkOps vX.Y.Z". You cannot remove this line.
The public URL is the setting **Public URL** in **Settings → Sign-in &
security**.

## Mail texts

1. Select the mail type (for example **Voicemail**) and the language.
2. Change the subject and the text. The preview shows the mail with sample
   values.
3. To insert a placeholder, put the cursor in the subject or the text and
   click the placeholder.
4. Click **Save**. TalkOps rejects texts with unknown placeholders or syntax
   errors.

The text format:

| Input | Result |
| --- | --- |
| Blank line | New paragraph |
| `**text**` | **bold** |
| `*text*` | *italic* |
| `[Open](https://example.com)` | Link (only `https://`, `http://`, `mailto:` or `{{ instance.url }}`) |
| `- item` or `1. item` | List |
| `{{ caller.name }}` | Value of a placeholder |
| `{% if transcript %}…{% endif %}` | Text only if the placeholder has a value |
| `\*` | A literal `*` |

Values are inserted as text. A caller name can thus not add formatting or
links to a mail.

**Send test** sends the current text with sample values to one address.
**Reset to default** restores the built-in text of this mail type and
language.

The language of a voicemail mail is the language of the voicemail box.

## Check the result

Send a test message and inspect subject, content, links and delivery result.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
