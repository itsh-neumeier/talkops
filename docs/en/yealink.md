# Setting up Yealink phones

TalkOps configures Yealink desk phones (T3x, T4x, T5x, VP59, CP9x0), DECT
bases (W60B, W70B, W80B) and Wi-Fi handsets (AX83H, AX86R) automatically: SIP
accounts, line keys with BLF, phonebooks, time zone, language, admin password
and firmware.

**AX83H/AX86R:** the handsets have no LAN port; hand out the provisioning URL
via DHCP option 66 on the Wi-Fi network or enter it on the phone under
*Settings → Auto Provision*. Up to 4 SIP accounts, 16 line keys (line and
BLF, keys 1–4 on the idle screen; the handsets have no speed dial keys),
custom ringtone and wallpaper. Line keys need **firmware 180.87.0.15** or
later; AX83H and AX86R use the same firmware file (`AX86(AX83,AX86)-….rom`) –
upload it under *Firmware* for both models. Set up the Wi-Fi itself (SSID,
password) on the handset; TalkOps does not distribute it.

> **Status:** All parameters come from the official *Yealink Auto
> Provisioning Guide*. Tests with real devices are still pending – feedback
> (including "works with T46U, firmware …") is very welcome.

## 1. Add the phone

1. **Phones → Add phone**: enter a name, the MAC address (label on the back)
   and the model.
2. **Settings → Extensions → extension → Add device**: choose the phone under *Phone*.
   The *account* (1, 2, …) is assigned automatically; on DECT bases it is the
   handset number.

One phone can carry several extensions (e.g. account 1 = own extension,
account 2 = reception).

## 2. Point the phone to TalkOps

**Phones → Auto-provisioning → Show provisioning URL** shows the URL including
credentials, e.g.

```
http://provision:Ab3dE…@192.168.1.10:8080/provisioning
```

**Option A – DHCP option 66 (recommended):** add the URL to your DHCP server
as option 66 (type *text/string*). New or factory-reset phones then configure
themselves.

| DHCP server | Setting |
|---|---|
| UniFi | *Settings → Networks → network → DHCP → Custom DHCP Option*: code 66, type text |
| OPNsense / pfSense | *Services → DHCPv4 → LAN → Additional options*: number 66, type text |
| dnsmasq / Pi-hole | `dhcp-option=66,"http://provision:…@192.168.1.10:8080/provisioning"` |
| FRITZ!Box | no option 66 – use option B |

**Option B – manually:** in the phone's web interface (default login
`admin`/`admin`) under *Settings → Auto Provision*: *Server URL* =
`http://192.168.1.10:8080/provisioning`, *username* `provision`, *password*
from TalkOps, then **Autoprovision Now**.

The phone reboots and registers. Afterwards the **phone admin password** shown
in TalkOps is required for the phone's web interface. The phone list shows the
last contact, IP address and firmware version.

## 3. Line keys and BLF

On the phone's page you can assign keys:

- **Line** – key for one of the accounts (default for the first keys).
- **BLF (extension)** – shows whether an extension is idle, ringing or busy.
  Pressing a flashing BLF key picks up the call.
- **Speed dial** – dials a number.

After saving press **Resync**: the phone reboots and applies the changes.
Phones that are not registered fetch the configuration on their next start or
at night between 2 and 4 am.

### Label and display name

Under *Accounts on this phone* every account has two fields (also in the
extension's device dialog once a phone is selected):

- **Label on the phone** – shown on the line key and the idle screen (empty =
  extension number), at most 32 characters.
- **Display name** – the caller name the phone sends (empty = name of the
  extension), at most 64 characters.

Besides *Desk phone* and *DECT handset*, the device type **Wi-Fi handset**
(AX83H/AX86R) is available.

### Ringtone and wallpaper

Upload them under **Settings → Phones → Ringtones and wallpapers**:

- **Ringtone:** any audio file the browser plays (MP3, WAV, …). It is
  converted to the Yealink format (WAV, 8 kHz, mono, 16 bit) and cut to 6, 15
  or 30 seconds. T42U, T43U and T53W take ringtones up to 100 KB (about 6
  seconds), T46U/T48U/T54W/T57W up to 8 MB.
- **Wallpaper:** any picture (JPEG, PNG, …); it is scaled to at most
  1280 × 800 and stored as JPEG (Yealink: at most 5 MB). Supported by T43U,
  T46U, T48U, T53W, T54W and T57W.

Select them on the phone's page under *Ringtone, wallpaper and phone book*,
save, **Resync**. The phone downloads the file (`ringtone.url` /
`wallpaper_upload.url`) and selects it (`phone_setting.ring_type` /
`phone_setting.backgrounds`). *Keep the phone's own setting* leaves both
untouched. Models without documented support (T31G, T33G, T58W, VP59,
conference phones, DECT) do not show the selection.

Sources: Yealink *SIP-T5 Series Administrator's Guide*, sections *Ring
Tones* and *Wallpaper Customization*; for AX83H/AX86R the Yealink
documentation *Ring Tones*, *Wallpaper Settings* and *Line Key*
(support.yealink.com).

## 4. Feature codes

| Code | Function |
|---|---|
| `*78` / `*79` | do not disturb on / off |
| `*72<number>` / `*73` | forward all calls (extension or external number) / off |
| `**<extension>` | pick up a ringing call |
| `*51` … `*59` | park a call / pick up a parked call ([call routing](call-routing.md)) |
| `*30<number>` | time condition "closed" on/off |
| `*97` / `*98` | own voicemail / any voicemail with PIN ([voicemail](voicemail.md)) |
| `*85` / `*85<extension>` | open the door (first door station / the one with this extension), `*86…` for the second lock ([door station](door-station.md)) |

The phone's DND key toggles *do not disturb* of the extension on account 1.
Both settings are also available in the web UI on the extension page and
under *Settings → My phones*.

### Dial now (dial plan)

Without a dial plan a Yealink waits a few seconds after the last digit
before it dials (or until you press `#` or *Send*). TalkOps therefore sends
dial-now rules to the phones (`dialplan.dialnow.rule.1`–`20`); these numbers
go out after one second:

- emergency numbers (110, 112 or the ones set under *Settings → Telephony*),
- the fixed codes `*51`–`*59`, `*73`, `*78`, `*79`, `*97`, `*98`,
- all internal numbers (extensions, groups, menus, queues).

A number is left out if another dialable number starts with it – with 300 and
3000, the phone keeps waiting after 300. `*1`–`*99` are reserved for system
codes and take precedence; `*<number>` for internal numbers starts at `*100`.
Internal numbers that begin with a system code (e.g. 510 → `*51`) are
therefore dialed without the star on a Yealink. Internal numbers starting with `11`
(service numbers such as 11833) and – when an area code is set – all internal
numbers also have no dial-now rule, because local numbers can then be dialed
without the area code. New extensions reach the phone with the next resync
(at the latest overnight).

Sources: Yealink documentation *Dial Plan* (support.yealink.com) and *Using
Dial Plan Feature on Yealink SIP-T3XG Phones*.

### Comfort settings

Under **Phones → Comfort settings** you set what TalkOps configures on all
matching phones; each value can be set differently on a phone's page.
*Keep the phone's setting* (the default) writes nothing – the phone keeps
what is set on the device; the factory value is shown where Yealink
documents it. Changes reach the phone with the next **Resync**.

Currently for the AX83H/AX86R Wi-Fi handsets:

| Area | Setting | Parameter |
|---|---|---|
| Tones | key tone, charging tone, vibration | `features.send_key_tone`, `features.charging_tone.enable`, `phone_setting.vibrate.enable` |
| Charging cradle | lifting answers, placing ends the call | `phone_setting.off_cradle_auto_answer.enable`, `phone_setting.end_call_on_hook.enable` |
| Calls | call waiting, call waiting tone | `call_waiting.enable`, `call_waiting.tone` |
| Display | backlight time, brightness, 12/24 h | `phone_setting.backlight_time`, `phone_setting.active_backlight_level`, `local_time.time_format` |
| Notifications | missed calls, new voicemail | `features.missed_call_popup.enable`, `features.voice_mail_popup.enable` |
| Audio | noise filter, acoustic shield | `features.noise_filtering_rev.enable`, `features.acoustic_shield.mode` |

The list lives in `presets/phones/yealink.yaml` (`settings:`) and can be
extended there. Sources: Yealink AX86R documentation (firmware 180.87) on
support.yealink.com; vibration only appears in Yealink's AX8X provisioning
template 180.87.0.5.

## 5. Phonebook

Every phone shows **Internal** (all enabled extensions) and **Contacts** (the
global phone book: all contacts without a section). Numbers are shown in
national format and can be dialed directly.

**Sections** are additional phone books that appear only on selected phones –
e.g. *Family* only in the living room, *Suppliers* only in the office:

1. *Phone book → Sections*: add a section.
2. In the contact choose the section instead of *Global*.
3. On the phone's page tick up to three sections under *Phone book sections
   on this phone*, save, **Resync**.

A deleted section disappears with its contacts from the phones as well.

### Importing contacts (CSV)

*Phone book → Import*: choose a CSV file; TalkOps shows a preview right away
(new / unchanged / rows with errors and their line number), only **Import**
stores anything. The dialog offers the **sample file** for download
(`phonebook-sample.csv`):

```csv
Name,Company,Work,Mobile,Other,Section
Erika Mustermann,Musterfirma GmbH,030 23125 101,0171 3920001,,
Max Mustermann,,,0171 3920002,030 23125 102,Family
```

- **Delimiter** semicolon, comma or tab; encoding UTF-8 or Windows (Excel
  "CSV (comma delimited)") – both are detected.
- **Columns** are matched by their header name, in any order: *Name* (or
  *First name*/*Last name*, else *Company*), *Company*, *Work*, *Mobile*,
  *Other*/*Home*, *Section* (German headers work too). Exports from Outlook
  and Google Contacts work as they are.
- **Numbers** national or international; spaces, `-`, `/`, `.` and
  `+49 (0)89 …` are fine. Excel likes to show long numbers as `4.9171E+11` –
  format the column as *Text* first.
- **Section:** empty = the section chosen in the dialog (default *Global*);
  unknown sections are created.
- **Duplicates:** a contact with the same name in the same section is not
  created twice; with *Update existing contacts* its numbers are replaced by
  the ones from the file.

Up to 5000 rows per file; rows without a name or number are skipped.

## 6. Firmware

Download firmware from the Yealink support site (`.rom` or `.bin`), upload it
under **Phones → Firmware** for the matching model and **activate** it. Phones
update on their next resync. At most one image per model is active.

## Troubleshooting

- **Phone fetches nothing:** is port 8080 reachable from the phone network?
  The `talkops` container log shows `provisioning request from unknown phone`
  when the MAC address has not been added.
- **401 in the phone log:** wrong or regenerated credentials – update DHCP
  option 66 or the auto-provision settings. After 10 failed attempts TalkOps
  blocks the IP for 5 minutes.
- **BLF key stays dark:** the monitored extension needs at least one enabled
  device; run a resync after changing keys.
- **Check the configuration:** *Show configuration* on the phone page shows the
  generated file (it contains SIP passwords, every view is audited).

## Security

Provisioning uses HTTP on the LAN. The files contain SIP passwords, so run
TalkOps in a trusted network only, or behind the optional Caddy with HTTPS
(then use `https://…/provisioning`). **Generate new passwords** replaces the
provisioning and phone admin passwords.
