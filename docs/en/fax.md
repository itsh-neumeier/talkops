# Fax

[Handbook](index.md) · [Deutsch](../de/fax.md)

## Before you start

You need: fax.manage for setup and the listed fax permissions for sending and reading.

TalkOps can send PDFs and images as fax documents through a SIP trunk.
An incoming number can deliver fax documents to an inbox and an optional
email address. Fax is disabled after an upgrade until an administrator
enables it.

## Configure fax

1. Open **Settings → Fax**.
2. Select **Enable fax**.
3. Select a global outgoing number if users need a shared sender number.
4. Select whether all outgoing faxes must use the global number.
5. Set the saved header and footer.
6. Select the retention time and the fax transport for each trunk.
7. Save the settings.

The available transports are T.38 preferred with G.711 fallback, G.711 only,
and T.38 required. Your provider and the remote fax device must support the
selected transport. Test with your provider before relying on delivery.
Fax does not use Opus or video codecs.

Create a fax destination for each personal or shared inbox. Select its owner,
users who can send through its numbers, and users who can read its inbox.
Set an optional email address for received PDFs. Then open **Settings →
Numbers** and assign an incoming number to that fax destination. Changing
a voice number to a fax destination stops its previous voice routing.

Give users the required permissions under **Settings → Users → Roles**:

| Permission | Function |
| --- | --- |
| `fax.send` | Create and submit outgoing fax documents. |
| `fax.mail_submit` | Submit from an enabled whitelist email address. |
| `fax.templates.edit` | Save a personal header and footer. |
| `fax.use_global` | Use the configured global sender number. |
| `fax.access_all` | Read all fax documents of the tenant. |
| `fax.delete` | Delete fax documents within the user's access. |
| `fax.manage` | Configure fax, destinations and the sender whitelist. |

Users can select only their assigned outgoing numbers, shared destination
numbers granted to them, and the permitted global number. The API checks
the permission again before dialing.

## Send from the web interface

1. Open **Fax → Send**.
2. Enter the recipient number in international form, such as `+49123456`.
3. Select a sender number.
4. Attach one or more files.
5. Select **Prepare preview**.
6. Check the pages and the sender number.
7. Select **Send fax now**.

Supported files are PDF, JPEG, PNG, TIFF, BMP and WebP. Files form one fax
in attachment order. Each image becomes a page; all pages of a TIFF are
included. Animated images, encrypted PDFs and invalid files are rejected.
The limit is 10 files, 20 MiB of decoded attachments and 50 output pages.
Each user can keep at most 20 pending fax documents. New uploads require
at least 100 MiB of free space in the fax directory.
Images also have dimension and pixel limits. One invalid attachment prevents
the entire fax from being sent.

Saved web fax text supports `{sender}`, `{recipient}`, `{date}`, `{page}`
and `{pages}`. The renderer reserves margins and fits the document without
cropping. Users need the template permission to save personal text.

## Send by email

Use a dedicated mailbox on your existing mail server. Users send email to
that address through SMTP. TalkOps reads unread messages over IMAPS every
30 seconds. It does not open an SMTP listening port.

1. Configure outgoing SMTP under **Settings → E-mail**.
2. Configure the dedicated address, IMAPS host, port, user, password and
   folder under **Settings → Fax**.
3. Configure the mail server to accept messages for this mailbox only from
   authenticated users with bound sender addresses.
4. Confirm this restriction in the fax settings.
5. Add complete email addresses to the whitelist and select their TalkOps
   user. Give each user fax send and mail submission permissions.
6. Enable mailbox retrieval and save.

Use a dedicated folder that another mail client does not mark as read.
The configured server certificate must be valid. TalkOps has no insecure
TLS option. Keep SMTP, IMAPS and SIP credentials out of shared mail messages.
Mail-server sender validation is required because a From header can be forged.
An address whitelist alone does not authenticate the sender.

Example message:

```text
From: permitted-user@example.com
To: faxin@example.com
Subject: +49123456
Attachments: document.pdf, photo.jpg
```

The subject must contain only one clean international number. `Re:`, extra
text, spaces inside a number, multiple numbers and a caret are rejected.
The sender must match one enabled whitelist entry. Address matching uses
complete ASCII addresses and ignores letter case. Wildcards are not supported.
An empty whitelist permits no submissions.

Only file attachments become fax pages. The message body, HTML, signatures,
quoted threads and inline images are ignored. Mail-to-fax always prints the
accepted sender email address in the fax header. The configured fax footer
can still appear. It never contains the mail body.

TalkOps queues a rejection notice to the sender for an invalid subject or
a sender outside the whitelist. It also reports missing or invalid attachments.
Notices do not include the original mail content or files. Automatic replies,
delivery reports and messages without a usable sender do not trigger another
reply. SMTP failures retry the notice; they do not create another fax.
Keep the rejected mail in the dedicated mailbox for diagnosis.
The fax settings show the last 100 mail input results and report deliveries.

## Read results and receive faxes

The outbox shows preparation, queue, transfer and final states. A successful
fax needs a complete fax protocol result for all pages. The app also sends
reports to an accepted mail sender. Received PDFs appear in the inbox and
can be sent to the destination's configured reception address.

Busy and unanswered calls with no transferred pages can retry, up to three
attempts. A connection loss without a final result shows **Delivery unknown**.
Check with the recipient before selecting **Prepare again**. The recipient
could already have the document. Mail delivery failure does not change a
successful fax into another transmission.

Retention removes old completed documents and their input files. It skips
active preparation and transfer. The backup includes the `fax` volume even
when voice recordings are excluded. Keep `TALKOPS_SECRET_KEY` with the
protected backup configuration to recover the mailbox password.

## Runtime requirements

All three containers mount `/var/lib/talkops/fax`. They use the existing
UID/GID 10001. The API and media worker must use the same fax directory.
`TALKOPS_FAX_DIR` changes the path for both binaries; update every service
and mount consistently when using a custom path.

The server invokes the bundled Python standard-library mailbox adapter.
The worker image contains Python, Pillow, ReportLab, Poppler and DejaVu fonts.
For development, `TALKOPS_FAX_PYTHON` and `TALKOPS_FAX_HELPER` can select the
Python executable and the helper script. Run the format fixtures with:

```sh
python3 -m unittest discover -s tests/fax -v
```

Migration IDs: `20261106000001` and `20261106000002`. Apply both with the
upgraded server. Do not run an older binary after assigning a fax destination.

## Troubleshoot storage after an update

An image update does not update an existing Compose or Portainer stack
definition. The server and worker have read-only root filesystems. A missing
fax mount can cause `Read-only file system (os error 30)` during upload.

Open **Settings → Fax → Fax storage**. Select **Check fax storage again**.
This check creates, writes, and removes a small temporary file as the server
user. It also checks for at least 100 MiB of free space. A successful check
proves server access only. Check worker and FreeSWITCH access separately.

Add these entries to the existing stack definition if they are missing.
Keep its other volumes and service settings. Use the same named volume for
all three services. Keep any existing fax data in that volume.

```yaml
services:
  talkops:
    volumes:
      - fax:/var/lib/talkops/fax
  media-worker:
    volumes:
      - fax:/var/lib/talkops/fax
  freeswitch:
    volumes:
      - fax:/var/lib/talkops/fax
volumes:
  fax:
```

If fax data is in another volume or bind mount, use that existing source
for all three services. The destination must match the configured fax path.
For a custom `TALKOPS_FAX_DIR`, set the same path for the API and worker.
Mount the volume at that path in all three containers. The API passes the
path to FreeSWITCH.

Apply the changed stack definition. With Compose, run this command from the
existing deployment directory, with its usual environment and override files:

```sh
docker compose up -d talkops media-worker freeswitch
```

In Portainer, edit and update the existing stack. Keep the same project and
volume sources. A restart alone does not apply a changed volume definition.
Keep `read_only: true` for the API and worker. Only the fax volume needs writes.

Check access as the configured service user in each container:

```sh
for service in talkops media-worker freeswitch; do
  docker compose exec -T "$service" sh -ec '
    id
    probe=$(mktemp /var/lib/talkops/fax/.fax-check-XXXXXX)
    trap '\''rm -f "$probe"'\'' EXIT
    printf "fax storage check\n" > "$probe"
    cat "$probe"
  '
done
```

Use the custom destination path in this command if configured. Each service
must run as UID/GID 10001 and must use the same shared volume. For a bind
mount, give UID/GID 10001 write access to the existing fax directory.

Uploads return `fax_storage_unavailable` (HTTP 503) for an inaccessible
directory, or `fax_storage_full` (HTTP 507) for insufficient space. The send
page shows a translated explanation and keeps the selected attachments.
After the repair, prepare the preview again. No fax is queued by a rejected
upload. Server logs include the failed storage operation and path.

## Troubleshoot rejected incoming calls

PhoneBlock and the local call blocking settings also apply to incoming fax.
A log entry `inbound call blocked` with `phoneblock:4` means that four spam
reports met the configured blocking threshold. TalkOps rejects that call
before it starts the fax receiver. The call can have zero duration and
`CALL_REJECTED`, with no fax document.

Review the blocking decision under the call blocking settings. Then test
reception with an allowed caller. This storage repair does not change spam
rules, exceptions, or T.38 settings.

## Check the result

Check storage, preview and provider delivery with a harmless test document in both directions.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
