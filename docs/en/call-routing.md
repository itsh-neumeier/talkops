# Call routing

[Handbook](index.md) · [Deutsch](../de/anrufsteuerung.md)

## Before you start

You need: routing.manage for flow changes; blocking permissions are separate.

**Settings → Call routing** decides where calls go. Phone numbers, fallbacks, menu keys
and opening hours can point to any destination: extension, an extension's
voicemail, ring group, time condition, Smart Attendant or queue. Internal numbers
are unique across all of them.

## Ring groups

Several extensions ring **at once** or **one after another** (ring time per
member). Members on do-not-disturb are skipped. If nobody answers, the call
goes to the fallback – for example an extension's voicemail. An optional
prefix ("Support: ") shows the phones which group the call came through.

## Opening hours (time conditions)

- Time ranges per weekday (several per day, e.g. 8–12 and 13–17),
- public **holidays** nationwide or per German federal state (computed;
  holidays that only apply to some municipalities can be added as closed days),
- additional **closed days** (company holidays, bridge days),
- one destination each for "open" and "closed".

The mode can be switched at any time: *Automatic*, *Always open*, *Always
closed* – in the web UI (operators too) or on the phone with `*30<number>`
(e.g. `*3060`; a low double beep means "closed", a high one "automatic
again"). The time zone from the settings applies.

## Smart Attendant

A Smart Attendant is a call flow for incoming calls, like in UniFi Talk. It is
built under **Settings → Call routing → Smart Attendant** as a tree of steps: **+** adds a
step, clicking a step opens its settings on the right.

| Step | What happens |
|---|---|
| Keypress menu | A prompt; callers choose with a key (0–9, \*, #), each key has its own branch, plus "No input". Optionally callers may dial extensions directly. |
| Ring phones | The chosen extensions ring at once or one after the other; if nobody answers, the "No answer" branch continues. |
| Play audio | Plays a prompt, then continues. |
| Schedule | Branches on a time condition (business hours, holidays, closures) into "Open" and "Closed". |
| Voicemail | Callers leave a message; every chosen recipient gets it in their voicemail box (with e-mail and transcription as usual). |
| Forward | To an extension, ring group, queue, another Smart Attendant … |
| Park call | Parks in a free slot `*51`–`*59`, to be picked up from any phone. |
| Go to step | Jumps to another step, e.g. "back to the main menu". |
| Hang up | Says goodbye and ends the call. |

An empty branch ends the call. **Right-clicking** a step opens a menu to edit it, replace it with another step or remove it (with the steps below); right-clicking a key removes that branch. Prompts are generated with the computer voice,
recorded in the browser or uploaded, as for voicemail (see
[voicemail](voicemail.md#greetings-and-audio)). Voice menus from TalkOps 1.0 are
converted into a Smart Attendant with a keypress menu on update.

## Queues

Callers wait with music until an agent is free (FreeSWITCH `mod_callcenter`).
Every queue has its own page with three tabs:

- **General:** number, name, agents (order with the arrows). Agents on
  do-not-disturb or without devices get no calls.
- **Schedule:** a time condition as business hours; outside them the call goes
  to the "outside business hours" destination.
- **Call handling:**
  - *Greeting* (once before waiting) and *music on hold* (looped; without an
    own file the music from Settings → *Music on hold*),
  - *Call distribution*: longest idle, ring all, round robin, in order, fewest
    calls, random; ring time per agent, pause after each call,
  - *Queue size*: callers waiting at most – more go to the overflow destination,
  - *When nobody answers* (after the maximum wait, or when no agent is
    available for 30 seconds): forward **or** take a message for one or more
    recipients.

## Parking and transfers

- **Parking:** transferring a call to `*51` … `*59` parks it in that slot;
  dialing the same number from another phone picks it up. Yealink BLF keys can
  watch a slot with the value `park+*51`.
- **Blind transfers** from phones work to internal numbers, park slots and
  external numbers (via the default number).

## Call blocking

**Settings → Call routing → Call blocking** rejects inbound calls (SIP `603 Decline`):

- **Blocked numbers**: single numbers (`+4930123456`) or whole prefixes with
  a trailing `*` (`+49900*` for premium-rate numbers).
- Reject **anonymous callers** (number withheld).
- **PhoneBlock** ([phoneblock.net](https://phoneblock.net/phoneblock/)): a free
  community list of spam and cold-call numbers. Create an account on
  phoneblock.net, create an API key (`pbt_…`) under *Settings → API keys* and
  enter it here. Each inbound call is checked with at most 1.5 s delay;
  answers are cached for 6 hours. If PhoneBlock is unreachable, the call
  rings normally. *Reports needed*: PhoneBlock itself blocks from 4.
  PhoneBlock advises caution on business lines.
- **Test a number** shows whether and why a call would be blocked.

Blocked calls are logged as `inbound call blocked` with the reason.

The call log and recent calls also keep the reason in the call record.
See [blocked calls](call-log.md#blocked-calls-and-whitelist-exceptions).

### Local whitelist

Open **Settings → Call routing → Call blocking → Whitelist** to add or
remove exact international numbers. This area needs `blocking.allowlist`.
The permission alone gives access to this area; it does not give access
to routing configuration, block lists or PhoneBlock credentials.
Only administrators have this permission by default. An administrator can
grant it to another role in **Users → Roles**.

An entry permits future inbound calls from that exact number to bypass
the local block list and PhoneBlock. It can override a block prefix for
one number. A successful save applies to the next call, including when
PhoneBlock has a cached rejection. Schedules and other routing still apply.
Anonymous callers cannot match an entry. Prefix whitelist entries are not
supported. The exception is local; it does not change a PhoneBlock account.

To remove an entry, select **Delete** and confirm that blocking can apply
again. Existing block rules remain stored. Every whitelist change has an
audit entry. Backups include the whitelist and stored call decisions.

The API provides `GET` and `POST /api/v1/call-allowlist` and
`DELETE /api/v1/call-allowlist/{id}`. Each operation requires
`blocking.allowlist`. Duplicate creation returns the existing entry.

## Check the result

Call a test number through each changed branch, including timeout and closed hours.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
