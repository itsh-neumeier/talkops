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
(Benutzername `8001-1`, Passwort) braucht die Station.

## 2. Station einrichten (Weboberfläche der VTO)

Die Menüs unterscheiden sich je nach Firmware; die Namen stammen aus Dahuas
Anleitungen (VTO-Kurzanleitung v4.5, Handbuch VTO2311R-WP).

1. **Netzwerk → SIP-Server:**
   - **„SIP Server“ NICHT aktivieren** – sonst spielt die Station selbst
     Telefonanlage.
   - Servertyp: *Drittanbieter* / *Third Party* / *Asterisk* (je nach
     Firmware).
   - Server-Adresse: IP des TalkOps-Hosts, Port `5060`.
   - SIP-Nummer / Benutzer: `8001-1`, Registrierungspasswort: das Passwort
     aus Schritt 1, Domain: `talkops.local`.
2. **Lokale Einstellungen → Basis:** Die **Rufnummer** („Call No.“, Standard
   `9901`) wählt die Station beim Klingeln. Bei Stationen mit mehreren Tasten
   bekommt jede Taste ihre Nummer.
3. **Türöffnung per Tastenton (optional):** Einen Entriegelungscode festlegen
   und als Verfahren **RFC 2833** wählen – **nur eines** der Verfahren
   (RFC 2833 *oder* SIP INFO), sonst öffnet laut Dahua keines.

Die Station erscheint danach in der **Übersicht** unter *Registrierte Geräte* als `8001-1`.

## 3. Türsprechstelle in TalkOps

**Tür → Neue Türsprechstelle** (Admin):

| Feld | Bedeutung |
|---|---|
| Nebenstelle | die Nebenstelle aus Schritt 1 – jeder Anruf von ihr ist ein Klingeln |
| Klingeln geht an | Nebenstelle, Rufgruppe, Zeitsteuerung, Smart Attendant, Warteschlange oder Voicemail |
| Tasten | für Mehrfamilien-Stationen: gewählte Nummer (z. B. `9902`) → eigenes Ziel; `9902#0` und `9902` gelten als gleich |
| HTTP-Zugang | IP/Port und Web-Login der Station; ohne Host nur Klingeln, kein Türöffner/Bild/Protokoll |
| Schlösser | `2`, wenn ein zweites Schloss über das RS-485-Modul (z. B. DEE1010B) angeschlossen ist |
| Bild bei jedem Klingeln | speichert einen Schnappschuss im Ereignisprotokoll |
| Ereignisprotokoll | TalkOps folgt dem Ereignis-Stream der Station (Tür auf/zu, Entriegelung, Sabotage) |
| Webhook | jedes Ereignis als JSON an diese URL (Home Assistant) |

**Verbindung testen** zeigt das gemeldete Modell. Das Passwort der Station
wird verschlüsselt gespeichert.

## Tür öffnen

| Weg | Wie |
|---|---|
| Während des Gesprächs | Entriegelungscode der Station am Telefon tippen (Tastenton, Schritt 2.3) |
| Kurzwahl | `*85` (erste Türsprechstelle) bzw. `*85<Nebenstelle>`, z. B. `*858001`; `*86…` für das zweite Schloss. Eine Ansage bestätigt. Gut als Kurzwahltaste am Telefon. |
| Weboberfläche | **Tür → Tür öffnen** (alle Benutzer, mit Rückfrage) |
| Automation | `POST /hooks/door/<id>/open` mit Token, siehe unten |

Jede Öffnung steht mit Auslöser im Ereignisprotokoll; Öffnungen über die
Weboberfläche zusätzlich im Audit-Log.

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

- **Station registriert sich nicht:** „SIP Server“ an der Station aus? Domain
  `talkops.local`, Benutzer `8001-1`? Ist der SIP-Port der Anlage erreichbar?
- **Kein Bild am Telefon:** Das Telefon muss Video (H.264) können, z. B.
  Yealink T58W/VP59. Audio funktioniert immer.
- **Tür öffnet per Tastenton nicht:** Nur ein DTMF-Verfahren an der Station
  wählen (RFC 2833).
- **„nicht erreichbar“ in TalkOps:** HTTP-Zugang (IP, Port, Login) prüfen –
  **Verbindung testen**. Manche Firmware erlaubt den Web-Login nur über HTTP
  im lokalen Netz.
