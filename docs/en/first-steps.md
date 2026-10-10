# First steps

After the [installation](installation.md) TalkOps is available at
`http://<host>:8080`. The dashboard shows admins a *Getting started*
checklist that ticks off these steps as they are done.

1. **Create the administrator.** On first access TalkOps asks for username and
   password (at least 10 characters) of the first admin account.
2. **Dialing rules.** *Settings*: country code (default 49) and area code
   without 0. Emergency numbers (110, 112) are preset.
3. **Trunk.** *Settings → Trunks → New trunk*: pick the provider preset and enter your
   credentials (e.g. [LEONET](leonet.md)). Unknown provider: preset
   “Generic SIP provider”.
4. **Default number.** Set *Settings → Default number* – without it, no
   external or emergency calls are possible.
5. **Users and extensions.** *Settings → Users → New user*, then *Settings → Extensions → New
   extension* (from 100, 3–8 digits, e.g. any of 100–9999; not starting with 0,
   not 115, and nothing that starts or is part of an emergency number such as
   110, 112, 1120) and assign the user. `*1`–`*99` are reserved for system
   codes, so internal numbers start at 100 – for groups, time conditions,
   menus and queues as well. 2-digit numbers of older installations are kept,
   new ones can no longer be created.
6. **Devices.** In the extension, *Add device*. TalkOps generates SIP username
   and password. Configure the phone with:
   - SIP server/registrar: IP address of the TalkOps host, port 5060 (UDP)
   - username / authentication name: the shown SIP username
   - password: the shown SIP password

   All devices of an extension ring at the same time. Yealink
   auto-provisioning follows in phase 2.
7. **Route numbers.** *Settings → Phone numbers* (or on the trunk): choose the (main)
   extension each number rings at. *Ring more extensions …* adds extensions that ring at
   the same time; unanswered calls go to the main extension's voicemail. The same works
   from the extension: tick the numbers under *Incoming numbers that ring here* in its
   *Edit* dialog.

After login, users find their devices and credentials under *Settings → My phones* and
their calls under *Call log*.

## Dialing

| Input | Dialed as |
|---|---|
| `210`, `610` | extension 210 or 610 |
| `*610` | also extension 610 (star + internal number, from `*100`, 3–4 digits) |
| `*1` … `*99` | system codes, see [Yealink phones](yealink.md#4-feature-codes) |
| `110`, `112` | emergency – always via the default number, never with hidden caller ID |
| `115`, `11833` | service number, unchanged |
| `030 1234567` | national |
| `0043 1 234567`, `+43…` | international |
| `1234567` | local (with the configured area code) |
| `*31 030 1234567`, `#31#030 1234567` | this call with the caller ID hidden |
| `name@domain.com` | SIP address: through a trunk account of the same domain, otherwise directly over the internet |

### SIP addresses and accounts without a phone number

Some providers (e.g. sip2sip.info / SIP Thor) only give you a SIP address like
`name@sip2sip.info`, no phone number:

1. *Settings → Trunks → New trunk*, preset *Generic SIP provider*, the
   provider's registrar (e.g. `sip2sip.info`).
2. In the trunk, *Add account* with username and password – no phone number
   needed. Under *Inbound calls to this account* pick the extension (or
   group, voicemail …) that should ring for calls to the SIP address.
3. Calling out: enter the address in the softphone or on the phone, e.g.
   `anna@sip2sip.info`. Addresses of the same domain go through the account,
   all others directly over the internet.

SIP addresses are recognized by letters in the name; plain digits
(`12345@domain`) stay phone numbers. Switch off under *Settings → Telephony →
Allow dialing SIP addresses*. For direct calls over the internet the external
IP should be set, otherwise audio may not arrive.
