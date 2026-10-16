# Codec-Profile und Video je Gerät

Ein Codec-Profil legt fest, welchen Audio-Codec TalkOps mit einem Gerät
verwendet. TalkOps (FreeSWITCH) wandelt den Ton für die Gegenseite um; die
Gegenseite behält ihren Codec. Profile helfen bei Geräten in einem WLAN mit
Paketverlust oder Jitter.

Architekturentscheidungen: [ADR 0023](../adr/0023-device-codec-profiles.md),
[ADR 0028](../adr/0028-device-media-settings.md).

## Profile

| Profil | Codecs zum Gerät | Einsatz |
|---|---|---|
| Standard | Systemcodecs: Opus, G.722, G.711 (Video, wo erlaubt) | Tischtelefone, DECT, Softphones |
| HD (G.722) | G.722, Rückfall G.711, Jitter-Puffer | WLAN-Handsets in gutem WLAN |
| Robust (iLBC) | iLBC (20 ms, dann 30 ms), Rückfall G.711, Jitter-Puffer | Geräte in einem Netz mit Paketverlust |
| Nur G.711 | PCMA, PCMU, kein Jitter-Puffer | Geräte, die nur mit G.711 gut funktionieren (z. B. Türsprechstellen) |
| Automatisch | WLAN-Handsets: HD. Alle anderen Geräte: Standard | Voreinstellung |

Auswirkungen im Gespräch:

- **HD** klingt breitbandig. Bei Paketverlust hört man kurze Aussetzer.
- **Robust** klingt schmalbandig (wie ein normales Telefon). Bei etwa 5 bis
  15 % Paketverlust bleibt die Sprache gut verständlich. Jeder iLBC-Rahmen
  lässt sich allein dekodieren; ein verlorenes Paket beschädigt die
  folgenden nicht.
- Kein Codec hilft, wenn das Netz länger ausfällt (Roaming-Lücken über
  etwa 200 ms, kein WLAN-Empfang).
- Der Jitter-Puffer auf TalkOps-Seite glättet den Ton vom Gerät. Er fügt
  60 bis 200 ms Verzögerung hinzu.
- Ein Gerät mit HD, Robust oder Nur G.711 erhält kein Video, außer Sie
  stellen beim Gerät **Video** ein (siehe [Video je Gerät](#video-je-gerät)).
- Der Codec wechselt nicht während eines Gesprächs. Eine Profiländerung gilt
  ab dem nächsten Anruf.

## Profil einstellen

1. **Nebenstellen** öffnen und die Nebenstelle wählen.
2. Unter **Geräte** beim Gerät auf **Bearbeiten** klicken.
3. Ein **Codec-Profil** wählen.
4. Auf **Speichern** klicken.

Die Geräteliste zeigt „HD (G.722)“ oder „Robust (iLBC)“ neben dem Gerätetyp,
wenn ein Gerät eines dieser Profile nutzt.

## Video je Gerät

Die Einstellung **Video** legt die Video-Codecs eines Geräts bei Anrufen
fest, die Video erlauben. Ob ein Anruf Video haben kann, entscheidet weiterhin
die [Video-Regel](../adr/0025-video-policy.md) (zum Beispiel: Nebenstelle mit
Video zu Nebenstelle mit Video, Türsprechstelle zu ihrem Ziel).

| Einstellung | Video-Codecs zum Gerät |
|---|---|
| Automatisch | Profil Standard: H264 und VP8. Alle anderen Profile: aus |
| H264 und VP8 | H264 und VP8 |
| Nur H264 | H264. Für Geräte ohne VP8 (z. B. Dahua-VTH). |
| Nur VP8 | VP8 |
| Aus | Kein Video. Das Gerät erhält nur Audio-Codecs. |

Die Einstellung gilt für Anrufe zum Gerät und für Anrufe vom Gerät.
FreeSWITCH wandelt Video nicht um. Haben die beiden Geräte eines Anrufs keinen
gemeinsamen Video-Codec, zeigt der Anruf kein Bild.

Bei einem Anruf mit Video übernimmt die Seite des Anrufers den Audio- und
Video-Codec des Geräts, das abhebt (`inherit_codec`). So nutzt ein Browser,
der ein Gerät mit nur H264 anruft, H264.

Die Geräteliste zeigt „Video: …“, wenn ein Gerät nicht H264 und VP8 nutzt.

## Provisionierte Yealink-Telefone

Für ein Gerät mit HD, Robust oder Nur G.711 schaltet die
Telefonkonfiguration auf dem Konto des Geräts diese Codecs ein:

| Codec | Yealink-Payload | Priorität |
|---|---|---|
| G.722 | `g722` | 1 |
| iLBC | `ilbc` (AX83H), `ilbc_15_2kpbs` und `ilbc_13_33kpbs` (Firmware mit zwei iLBC-Raten) | 2, 3 |
| G.711 A-law | `pcma` | 4 |
| G.711 µ-law | `pcmu` | 5 |
| G.729, G.726, G.723.1 | `g729`, `g726_*`, `g723_*` | aus |

Das Telefon ignoriert Payload-Namen, die seine Firmware nicht kennt. TalkOps
wählt den Codec jedes Anrufs; ein Wechsel zwischen HD und Robust braucht
daher keinen Resync. Nach einem Wechsel von oder zu Standard auf
der Telefonseite auf **Resync** klicken. Mit Standard ändert TalkOps
die Codec-Einstellungen des Telefons nicht.

Quellen: Yealink AX83H Administrator's Manual, Abschnitt „Audio Codecs
Configuration“ (`account.X.codec.<payload_type>.enable` und `.priority`),
und Yealink SIP-T2 Administrator's Manual V85, gleicher Abschnitt, für die
beiden iLBC-Raten.

## WLAN

Diese Einstellungen im WLAN helfen oft mehr als der Codec:

- WMM (Wi-Fi Multimedia) an den Access Points einschalten. Yealink-Telefone
  markieren Sprachpakete ab Werk mit DSCP 46 (EF).
- Schnelles Roaming (802.11r/k/v) einschalten, wenn die Handsets es können.
- Für die Handsets das 5-GHz-Band nutzen.

## Codec prüfen

**Einstellungen → System → Status → Aktive Kanäle** zeigt den Codec jedes Kanals
(`read_codec` und `write_codec`). Ein Anruf zu einem Gerät mit Robust
zeigt `iLBC` auf dem Kanal des Geräts und den Codec der Gegenseite auf
dem anderen Kanal.
