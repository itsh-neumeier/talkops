# Setting up LEONET

LEONET lines register **every phone number separately** with its own
credentials at the registrar `sip.leovoice.online`. TalkOps creates one
registration (FreeSWITCH gateway) per number.

> Preset status: **untested** – the parameters come from the project's
> reference configuration. Please report back after the first successful test
> (issue/PR) so the preset can be marked as verified.

## What you need

- your phone number(s) incl. area code, e.g. `08331 123456`
- the SIP password of each number from the LEONET customer portal / access data

The username is derived automatically: `leo` + the number in international
format without `+`, i.e. `leo49` + area code without 0 + number, e.g.
`leo498331123456`.

## Steps

1. **Settings → Dialing:** enter your area code without 0 (e.g. `8331`) so
   local calls work without area code.
2. **Trunks → New trunk:** choose “LEONET – SIP-Anschluss”, enter a name, add.
3. In the trunk, **Add phone number:** number in international format
   (`+498331123456`), SIP password and the extension that should ring. Repeat
   for every number.
4. After a few seconds the registration shows **registered**; otherwise the
   error is shown (e.g. `403` = wrong password).
5. **Settings → Default number:** pick the main number. It is used for outgoing
   and emergency calls of extensions without an own outgoing number.

## Technical details

| Parameter | Value |
|---|---|
| Registrar / realm | `sip.leovoice.online` |
| Transport | UDP 5060 |
| Username | `leo{E.164 without +}` |
| Dialed number | national (`0831…`, `0049…` abroad) |
| Caller ID | international without `+` |
| Keepalive | SIP OPTIONS every 30 s |

All values can be overridden per trunk under **Edit → Advanced settings**.

## Troubleshooting

- **`403 Forbidden`**: wrong password or username.
- **`TRYING`/`FAIL_WAIT`**: registrar unreachable – check the firewall (UDP
  5080 outbound from the TalkOps host, RTP 16384–16999).
- **No or one-way audio**: set *Settings → Public IP for trunks* (or
  `stun:stun.l.google.com:19302`).
