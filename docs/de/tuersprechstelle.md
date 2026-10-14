# Türsprechstellen

TalkOps bindet Türsprechstellen und Innenstationen als SIP-Geräte an: Die
Station meldet sich wie ein Telefon an, das Klingeln geht an eine
Nebenstelle, Rufgruppe oder ein anderes Ziel, das Kamerabild (H.264) kommt
bei Videotelefonen mit. Die Tür öffnet man am Telefon, in der
Weboberfläche oder per Home Assistant.

SIP, Klingelziele, Video und Öffnungscodes sind für alle Hersteller gleich.
Die HTTP-Anbindung (Türöffner, Bilder, Ereignisse) und die
Geräteeinstellungen hängen vom Hersteller ab (ADR 0026).

| Hersteller | Geräte | Einrichtung |
|---|---|---|
| Dahua | VTO-Außenstationen, VTH-Innenstationen | [Dahua VTO und VTH](tuersprechstelle-dahua.md) |

Die Weboberfläche zeigt eine Schritt-für-Schritt-Anleitung pro Hersteller
und Gerätetyp: **Einstellungen → Türsprechstellen → Einrichtungsanleitung**.

> **Status:** Mit SIP-Simulator und einer nachgebildeten Dahua-HTTP-API
> getestet. Praxisberichte mit echten Geräten stehen auf der
> Herstellerseite. Rückmeldungen mit Modell und Firmware-Stand sind
> willkommen.

## 1. Nebenstelle für jedes Gerät anlegen

**Einstellungen → Nebenstellen → Neue Nebenstelle**, z. B. `8000` „Haustür“. Darin
**Gerät hinzufügen**, Typ **Türsprechstelle**. Die angezeigten SIP-Zugangsdaten
(Benutzername, Passwort) braucht das Gerät.

Der SIP-Benutzername ist frei wählbar. Er muss die Nebenstellennummer nicht
enthalten: Nebenstelle `8000` kann `8000`, `6578` oder `123` verwenden.
Dahua-VTO- und -VTH-Geräte arbeiten am besten mit reinen Ziffern. Ohne
eigenen Namen erzeugt TalkOps einen zufälligen Namen, z. B. `28x1js`.
Beispiel mit einer Nebenstelle je Gerät:

| Gerät | Nebenstelle | SIP-Benutzername |
|---|---|---|
| Außenstation (VTO) | `8000` | `8000` |
| Innenstation 1 (VTH) | `8001` | `8001` |
| Innenstation 2 (VTH) | `8002` | `9902` |

Erlaubt sind 3 bis 64 Buchstaben, Ziffern, `.`, `_` und `-`. Jeder
Benutzername darf nur einmal vorkommen (Groß- und Kleinschreibung gelten als
gleich). Der Name darf auch die Nummer einer anderen Nebenstelle sein. Anrufe
gehen weiter an die Nebenstellennummer, nicht an den SIP-Benutzernamen. `web-`
ist für Browser-Telefone reserviert. Ein Admin kann den Benutzernamen eines
Geräts später ändern (**Bearbeiten**). Danach muss sich das Gerät mit dem
neuen Namen neu registrieren.

### SIP-Passwortformat

| Format | Länge | Verwendung |
|---|---|---|
| Standard | 20 Buchstaben und Ziffern | alle Geräte (Vorgabe) |
| Kompatibel | 15 Buchstaben und Ziffern | Geräte, die höchstens 15 Zeichen annehmen, z. B. Dahua VTO und VTH |

- TalkOps wählt **Kompatibel (15 Zeichen)** vor, wenn du ein Gerät vom Typ
  **Türsprechstelle** hinzufügst. Du kannst es im Dialog ändern.
- Das Format gilt, wenn TalkOps ein Passwort erzeugt. **Bearbeiten**
  speichert das Format für das nächste neue Passwort und behält das
  aktuelle Passwort.
- **Neues Passwort** erzeugt sofort ein Passwort. Ein Admin kann in diesem
  Dialog das Format wählen. Das alte Passwort gilt sofort nicht mehr; trage
  das neue Passwort im Gerät ein.
- TalkOps speichert SIP-Passwörter verschlüsselt. Das Audit-Log enthält das
  Format, nie das Passwort.

## 2. Gerät einrichten

Folge der Herstellerseite, z. B. [Dahua VTO und VTH](tuersprechstelle-dahua.md).
Prüfe die Registrierung: Die **Übersicht** zeigt den SIP-Benutzernamen unter
*Registrierte Geräte*.

## 3. Türsprechstelle in TalkOps

**Einstellungen → Türsprechstellen → Neue Türsprechstelle** (Admin; der Menüpunkt **Tür** zum Öffnen erscheint, sobald es eine Türsprechstelle gibt):

| Feld | Bedeutung |
|---|---|
| Hersteller | bestimmt die HTTP-Anbindung und die Einrichtungsanleitung (derzeit: Dahua) |
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

### Klingelziel und Rufgruppe

Um mehrere Innenstationen oder Telefone klingeln zu lassen, öffne
**Anrufsteuerung → Rufgruppen** und lege eine Rufgruppe an, z. B. mit `8001`
und `8002`. Setze dann **Klingeln geht an** auf diese Rufgruppe.

## Tür öffnen

| Weg | Wie |
|---|---|
| Während des Gesprächs | Öffnungscode von TalkOps (siehe unten) oder Entriegelungscode der Station (siehe Herstellerseite) am Telefon tippen |
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

## SIP-Anmeldeschutz

Der SIP-Anmeldeschutz sperrt Adressen nach fehlgeschlagenen Anmeldungen
(**Einstellungen → Anmeldung & Sicherheit → SIP-Anmeldeschutz**). Für eine
Türsprechstelle muss er nicht ausgeschaltet werden.

- Hat TalkOps die Adresse eines Geräts bei der Einrichtung gesperrt, wähle
  **Entsperren** für diese Adresse.
- Scheitert ein Gerät bei der Einrichtung oft, trage nur seine IP-Adresse
  unter *Vertrauenswürdige Netze* ein (z. B. `192.168.1.50`), nicht das
  ganze Netz.
- Entferne die Ausnahme nach erfolgreicher Einrichtung, wenn du sie nicht
  brauchst.

## Fehlersuche

- **`401`, danach `403 Forbidden`:** TalkOps hat die Anmeldung abgelehnt.
  Ursachen sind ein falsches oder gekürztes Passwort, ein unbekannter
  SIP-Benutzername, ein Authentifizierungsname, der vom SIP-Benutzernamen
  abweicht, oder eine gesperrte Adresse. Das TalkOps-Log zeigt
  Benutzernamen und Adresse:
  `docker compose logs -f talkops | grep "SIP authentication"`. Die
  Herstellerseite nennt die gerätespezifischen Prüfungen.
- **Station registriert sich nicht:** „SIP Server“ an der Station aus?
  *Username* (mit *Enable*) und *SIP Server Username* beide auf den
  SIP-Benutzernamen (z. B. `8000`) und beide Passwörter gesetzt? Ist der
  SIP-Port der Anlage erreichbar?
- **Station lehnt den Benutzernamen ab:** einen Namen nur aus Ziffern
  verwenden, z. B. `8000` (Gerät **Bearbeiten** → SIP-Benutzername).
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
