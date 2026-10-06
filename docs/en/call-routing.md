# Call routing

**Call routing** decides where calls go. Phone numbers, fallbacks, menu keys
and opening hours can point to any destination: extension, an extension's
voicemail, ring group, time condition, voice menu or queue. Internal numbers
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

## Voice menus

Callers hear a greeting and choose with the keypad. The greeting is spoken by
the computer voice from text or uploaded as a WAV file. Each key (0–9, \*, #)
gets a destination; optionally callers may dial extensions directly. Without
valid input after the configured attempts the call goes to the "no input"
destination (or ends).

## Queues

Callers wait with music until an agent is free (FreeSWITCH `mod_callcenter`).
Strategies: longest idle, ring all, round robin, in order, fewest calls,
random. You can set the maximum wait, ring time per agent and a pause after
each call. Agents on do-not-disturb or without devices get no calls. After the
maximum wait – or when no agent is available for 30 seconds – the call goes to
the fallback destination.

## Parking and transfers

- **Parking:** transferring a call to `*51` … `*59` parks it in that slot;
  dialing the same number from another phone picks it up. Yealink BLF keys can
  watch a slot with the value `park+*51`.
- **Blind transfers** from phones work to internal numbers, park slots and
  external numbers (via the default number).
