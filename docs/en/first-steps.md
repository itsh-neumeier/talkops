# First steps

After the [installation](installation.md) TalkOps is available at
`http://<host>:8080`. The dashboard shows admins a *Getting started*
checklist that ticks off these steps as they are done.

1. **Create the administrator.** On first access TalkOps asks for username and
   password (at least 10 characters) of the first admin account.
2. **Dialing rules.** *Settings*: country code (default 49) and area code
   without 0. Emergency numbers (110, 112) are preset.
3. **Trunk.** *Trunks → New trunk*: pick the provider preset and enter your
   credentials (e.g. [LEONET](leonet.md)). Unknown provider: preset
   “Generic SIP provider”.
4. **Default number.** Set *Settings → Default number* – without it, no
   external or emergency calls are possible.
5. **Users and extensions.** *Users → New user*, then *Extensions → New
   extension* (2–8 digits, not starting with 0 or 11) and assign the user.
6. **Devices.** In the extension, *Add device*. TalkOps generates SIP username
   and password. Configure the phone with:
   - SIP server/registrar: IP address of the TalkOps host, port 5060 (UDP)
   - username / authentication name: the shown SIP username
   - password: the shown SIP password

   All devices of an extension ring at the same time. Yealink
   auto-provisioning follows in phase 2.
7. **Route numbers.** *Phone numbers*: choose the extension each number rings at.

After login, users find their devices and credentials under *My phones* and
their calls under *Call log*.

## Dialing

| Input | Dialed as |
|---|---|
| `21` | extension 21 |
| `110`, `112` | emergency – always via the default number, never with hidden caller ID |
| `115`, `11833` | service number, unchanged |
| `030 1234567` | national |
| `0043 1 234567`, `+43…` | international |
| `1234567` | local (with the configured area code) |
| `*31 030 1234567`, `#31#030 1234567` | this call with the caller ID hidden |
