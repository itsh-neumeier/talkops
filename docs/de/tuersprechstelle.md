# Türsprechstelle (Dahua VTO)

TalkOps bindet Dahua-VTO-Türstationen (z. B. VTO2202F, VTO2211G, VTO3221E,
VTO4202F) direkt an: Die Station meldet sich wie ein Telefon an, das
Klingeln geht an eine Nebenstelle, Rufgruppe oder ein anderes Ziel, das
Kamerabild (H.264) kommt bei Videotelefonen mit. Die Tür öffnet man am
Telefon, in der Weboberfläche oder per Home Assistant.

> **Status:** Mit SIP-Simulator und einem nachgebauten Dahua-HTTP-API
> getestet, noch nicht mit echter Hardware. Dahuas HTTP-Schnittstelle ist
> nicht öffentlich dokumentiert; TalkOps nutzt die Aufrufe, die etablierte
> Open-Source-Integrationen verwenden. Rückmeldungen mit Modell und
> Firmware-Stand sind willkommen.

## 1. Nebenstelle für die Tür anlegen

**Einstellungen → Nebenstellen → Neue Nebenstelle**, z. B. `8001` „Haustür“. Darin
**Gerät hinzufügen**, Typ **Türsprechstelle**. Die angezeigten SIP-Zugangsdaten
(Benutzername `8001vto`, Passwort) braucht die Station.

Der SIP-Benutzername ist die Nebenstellennummer plus eine Endung. Ohne eigene
Endung verwendet TalkOps bei Türsprechstellen `vto`. Dahua-VTO- und
-VTH-Geräte akzeptieren kein `-` im Benutzernamen. Die Endung ist frei wählbar,
z. B. eine Nebenstelle je Gerät:

| Gerät | Nebenstelle | SIP-Benutzername |
|---|---|---|
| Außenstation (VTO) | `8000` | `8000vto` |
| Innenstation 1 (VTH) | `8001` | `8001vth` |
| Innenstation 2 (VTH) | `8002` | `8002vth` |

Erlaubt sind Buchstaben, Ziffern, `.`, `_` und `-`. Die Endung darf nicht mit
einer Ziffer beginnen. Ein Admin kann den Benutzernamen eines Geräts später
ändern (**Bearbeiten**). Danach muss sich das Gerät mit dem neuen Namen neu
registrieren.

## 2. Station einrichten (Weboberfläche der VTO)

Die Menüs unterscheiden sich je nach Firmware; die Namen stammen aus Dahuas
Anleitungen (VTO-Kurzanleitung v4.5, Handbuch VTO2311R-WP).

1. **Netzwerk → SIP-Server:**

   | Feld der VTO | Wert |
   |---|---|
   | SIP Server – Enable | **aus** (sonst spielt die Station selbst Telefonanlage) |
   | Server Type | *ThirdParty* (manche Firmware: *Asterisk*) |
   | IP Addr. | IP-Adresse oder Hostname des TalkOps-Hosts |
   | Port | `5060` |
   | Username + **Enable** | **Enable** anhaken, dann der SIP-Benutzername aus Schritt 1, z. B. `8001vto`. Ohne *Enable* nimmt die Station ihre eigene VTO-Nummer. |
   | Password | das SIP-Passwort aus Schritt 1 |
   | SIP Domain | `talkops.local` (Dahuas Vorgabe `VDP` funktioniert auch) |
   | SIP Server Username | **derselbe** SIP-Benutzername wie bei *Username* – nicht `admin` |
   | SIP Server Password | **dasselbe** SIP-Passwort |

   TalkOps nimmt eine Registrierung nur an, wenn der Anmeldename gleich dem
   SIP-Benutzernamen ist. *Username* und *SIP Server Username* müssen also
   übereinstimmen.
2. **Lokale Einstellungen → Basis:** Die **Rufnummer** („Call No.“, Standard
   `9901`) wählt die Station beim Klingeln. Bei Stationen mit mehreren Tasten
   bekommt jede Taste ihre Nummer.
3. **Türöffnung per Tastenton in der Station (optional):** Einen
   Entriegelungscode festlegen und als Verfahren **RFC 2833** wählen – **nur
   eines** der Verfahren (RFC 2833 *oder* SIP INFO), sonst öffnet laut Dahua
   keines. Alternativ erkennt TalkOps eigene Öffnungscodes (siehe Abschnitt 3).

Die Station erscheint danach in der **Übersicht** unter *Registrierte Geräte* als `8001vto`.

## 3. Türsprechstelle in TalkOps

**Einstellungen → Türsprechstellen → Neue Türsprechstelle** (Admin; der Menüpunkt **Tür** zum Öffnen erscheint, sobald es eine Türsprechstelle gibt):

| Feld | Bedeutung |
|---|---|
| Nebenstelle | die Nebenstelle aus Schritt 1 – jeder Anruf von ihr ist ein Klingeln |
| Klingeln geht an | Nebenstelle, Rufgruppe, Zeitsteuerung, Smart Attendant, Warteschlange oder Voicemail |
| Tasten | für Mehrfamilien-Stationen: gewählte Nummer (z. B. `9902`) → eigenes Ziel; `9902#0` und `9902` gelten als gleich |
| HTTP-Zugang | IP/Port und Web-Login der Station; ohne Host nur Klingeln, kein Türöffner/Bild/Protokoll |
| Schlösser | `2`, wenn ein zweites Schloss über das RS-485-Modul (z. B. DEE1010B) angeschlossen ist |
| Öffnungscode Tür 1 / Tür 2 | DTMF-Code, den TalkOps im Gespräch erkennt, siehe unten |
| Bild bei jedem Klingeln | speichert einen Schnappschuss im Ereignisprotokoll |
| Ereignisprotokoll | TalkOps folgt dem Ereignis-Stream der Station (Tür auf/zu, Entriegelung, Sabotage) |
| Webhook | jedes Ereignis als JSON an diese URL (Home Assistant) |

**Verbindung testen** zeigt das gemeldete Modell. Das Passwort der Station
wird verschlüsselt gespeichert.

## Tür öffnen

| Weg | Wie |
|---|---|
| Während des Gesprächs | Öffnungscode von TalkOps (siehe unten) oder Entriegelungscode der Station (Schritt 2.3) am Telefon tippen |
| Kurzwahl | `*85` (erste Türsprechstelle) bzw. `*85<Nebenstelle>`, z. B. `*858001`; `*86…` für das zweite Schloss. Eine Ansage bestätigt. Gut als Kurzwahltaste am Telefon. |
| Weboberfläche | **Tür → Tür öffnen** (alle Benutzer, mit Rückfrage) |
| Automation | `POST /hooks/door/<id>/open` mit Token, siehe unten |

Jede Öffnung steht mit Auslöser im Ereignisprotokoll; Öffnungen über die
Weboberfläche zusätzlich im Audit-Log.

### Öffnungscodes in TalkOps (DTMF)

Jedes Schloss kann einen eigenen Öffnungscode haben: 2 bis 8 Tasten aus
`0-9`, `*` und `#`, z. B. `*1` für Tür 1 und `*2` für Tür 2. TalkOps erkennt
den Code in diesen Gesprächen und öffnet die Tür über den HTTP-Zugang:

- im Gespräch nach dem Klingeln (das Telefon, das annimmt);
- bei einem direkten Anruf eines internen Telefons auf die Nebenstelle der
  Türsprechstelle (z. B. `8001`);
- in externen Gesprächen nur von den Nummern unter **Externe Nummern für
  Öffnungscodes**: Klingeln, das aufs Handy weitergeleitet wird, oder ein
  Anruf von außen auf eine Rufnummer, die zur Türsprechstelle geht.

Interne Telefone dürfen die Codes immer verwenden. Ist die Liste leer, kann
kein externes Gespräch die Tür per Code öffnen. Eine Nummer pro Zeile, in
jedem üblichen Format (`0170 1234567` oder `+491701234567`). TalkOps
speichert die Nummern in E.164.

Tasten an der Türsprechstelle selbst wertet TalkOps nicht aus. Eine Pause
von mehr als 4 Sekunden beginnt einen neuen Code. Je Gespräch wertet TalkOps
höchstens 40 Tasten aus.

Voraussetzungen und Hinweise:

- Der HTTP-Zugang der Türsprechstelle muss eingerichtet sein.
- TalkOps speichert die Codes verschlüsselt. Die Weboberfläche zeigt einen
  gespeicherten Code nicht an. Leeres Feld = Code bleibt. **Öffnungscodes
  entfernen** löscht sie.
- Die beiden Codes dürfen nicht aufeinander enden (z. B. `#1` und `1`).
- Einen anderen Code als den Entriegelungscode der Station verwenden. Sind
  beide gleich, öffnet die Tür zweimal.
- Bei Anrufen von außen vergleicht TalkOps die Rufnummer des Anrufers. Diese
  lässt sich fälschen. Bei erlaubten externen Nummern einen Code mit
  mindestens 4 Tasten verwenden.

## Livebild und Protokoll

**Tür** zeigt für alle Benutzer die Stationen, auf Wunsch das Kamerabild
(alle 2 Sekunden aktualisiert) und das Ereignisprotokoll mit Bildern.
Protokoll und Bilder werden wie Gesprächsaufnahmen nach der eingestellten
Aufbewahrungsfrist gelöscht (**Einstellungen → Gesprächsaufzeichnung**,
Standard 90 Tage).

## Home Assistant

**Ereignisse empfangen:** In Home Assistant eine Automation mit Auslöser
*Webhook* anlegen und deren URL (z. B.
`http://homeassistant.local:8123/api/webhook/haustuer-klingel`) bei der
Türsprechstelle als Webhook eintragen. TalkOps sendet:

```json
{
  "event": "ring",
  "event_id": "…",
  "door_station": { "id": "…", "name": "Haustür" },
  "detail": { "dialed": "9901" },
  "has_snapshot": true,
  "at": "2026-10-06T16:18:03Z"
}
```

`event` ist eines von `ring`, `open_command`, `opened`, `door_open`,
`door_closed`, `unlock_failed`, `alarm`, `online`, `offline`.

**Tür öffnen:** Bei der Türsprechstelle **Token für Automationen** erzeugen
(wird nur einmal angezeigt) und in `configuration.yaml`:

```yaml
rest_command:
  haustuer_oeffnen:
    url: "http://talkops.local:8080/hooks/door/<id>/open"
    method: post
    headers:
      authorization: !secret talkops_door_token   # "Bearer <token>"
    content_type: "application/json"
    payload: '{"door": 1}'
```

## Fehlersuche

- **Station registriert sich nicht:** „SIP Server“ an der Station aus?
  *Username* (mit *Enable*) und *SIP Server Username* beide auf den
  SIP-Benutzernamen (z. B. `8001vto`) und beide Passwörter gesetzt? Ist der
  SIP-Port der Anlage erreichbar?
- **Station lehnt den Benutzernamen ab:** einen Namen ohne `-` verwenden,
  z. B. `8001vto` (Gerät **Bearbeiten** → SIP-Benutzername).
- **Öffnungscode bewirkt nichts:** HTTP-Zugang eingerichtet (**Verbindung
  testen**)? Sendet das Telefon DTMF als RFC 2833? Das Ereignisprotokoll
  zeigt jede Öffnung mit der Quelle `dtmf`. Bei externen Gesprächen: Steht
  die Nummer unter **Externe Nummern für Öffnungscodes**?
- **Kein Bild am Telefon:** Das Telefon muss Video (H.264) können, z. B.
  Yealink T58W/VP59. Audio funktioniert immer. Eine Warteschlange als Ziel
  und ein WLAN-Handset mit dem Codec-Profil *HD* oder *Robust* erhalten kein
  Video. Die Türsprechstelle sendet ihr Bild an alle angerufenen Telefone;
  für diesen lokalen Anruf ist *Videoanrufe* an der Nebenstelle nicht nötig.
- **Tür öffnet per Tastenton nicht:** Nur ein DTMF-Verfahren an der Station
  wählen (RFC 2833).
- **„nicht erreichbar“ in TalkOps:** HTTP-Zugang (IP, Port, Login) prüfen –
  **Verbindung testen**. Manche Firmware erlaubt den Web-Login nur über HTTP
  im lokalen Netz.
