# Dahua VTO und VTH

Diese Seite beschreibt, wie du Dahua-VTO-Außenstationen und
Dahua-VTH-Innenstationen an TalkOps anbindest. Die allgemeinen Funktionen
(Klingelziel, Öffnungscodes, Bilder, Home Assistant) stehen auf der Seite
[Türsprechstellen](tuersprechstelle.md). TalkOps zeigt diese Anleitung auch
unter **Einstellungen → Türsprechstellen → Einrichtungsanleitung**.

> **Status:** Menünamen unterscheiden sich je nach Modell und Firmware.
> Als *nicht bestätigt* markierte Angaben sind nicht mit echten Geräten
> getestet.

| Modell und Firmware | Stand |
|---|---|
| VTO4202F-P-S2, V4.600.0.3 | Registriert sich bei TalkOps (Praxisbericht). |
| VTH5422H, V4.800.0.2 | `403 Forbidden` mit 20-stelligem Passwort. Der Test mit 15-stelligem Passwort steht aus. |
| VTO2202F, VTO2211G, VTO3221E | Mit SIP-Simulator und nachgebildeter HTTP-API getestet, nicht mit echter Hardware. |

## Vor dem Start

- Gib jedem Gerät eine eigene Nebenstelle und einen eigenen
  SIP-Benutzernamen. Nutze ein SIP-Konto nie auf zwei Geräten.
- Halte SIP-Passwort und Geräte-Login getrennt. Siehe
  [SIP-Passwort und Gerätepasswort](#sip-passwort-und-gerätepasswort).
- Stelle sicher, dass jedes Gerät den TalkOps-Server auf UDP-Port `5060`
  erreicht.

Beispielplan:

| Gerät | Nebenstelle | SIP-Benutzername |
|---|---|---|
| VTO | `8000` | `8000vto` |
| VTH 1 | `8001` | `8001vth` |
| VTH 2 | `8002` | `8002vth` |

## SIP-Passwortformat

Dahua-Geräte nehmen eventuell weniger Zeichen an als das
Standard-SIP-Passwort von TalkOps (20 Zeichen). Die Yeastar-Anleitung für
Dahua-VTH nennt eine Länge von 8–15 Zeichen.

- Beim Hinzufügen eines Geräts vom Typ **Türsprechstelle** wählt TalkOps
  das Format **Kompatibel (15 Zeichen)** vor. Du kannst das Format im Dialog
  ändern.
- Für ein vorhandenes Gerät: **Neues Passwort** wählen, dann **Kompatibel
  (15 Zeichen)**, dann **Neues Passwort erzeugen**. Das alte Passwort gilt
  sofort nicht mehr. Trage das neue Passwort im Gerät ein.
- Eine Formatänderung über **Bearbeiten** ändert das aktuelle Passwort
  nicht. Das Format gilt ab dem nächsten neuen Passwort.
- TalkOps ändert die Passwörter funktionierender Geräte nicht.

*Nicht bestätigt:* das 15-Zeichen-Limit für den VTH5422H mit Firmware
V4.800.0.2. Ein erfolgreicher Softphone-Test (zum Beispiel MicroSIP) ersetzt
keinen Test mit dem echten Gerät.

## VTO (Außenstation)

### Einstellungen in TalkOps

1. Öffne **Einstellungen → Nebenstellen** und lege eine Nebenstelle für die
   VTO an, zum Beispiel `8000` „Haustür“.
2. Wähle in der Nebenstelle **Gerät hinzufügen**. Setze den Typ auf
   **Türsprechstelle** und die Endung des SIP-Benutzernamens auf `vto`
   (SIP-Benutzername `8000vto`).
3. Behalte das SIP-Passwortformat **Kompatibel (15 Zeichen)**.
4. Speichere. Kopiere SIP-Benutzernamen und SIP-Passwort.
5. Öffne **Einstellungen → Türsprechstellen → Neue Türsprechstelle**. Setze
   den Hersteller auf **Dahua** und wähle die Nebenstelle der VTO.
6. Wähle das Klingelziel unter **Klingeln geht an**. Für Türöffner, Bilder
   und Ereignisprotokoll trage den HTTP-Zugang der VTO ein (IP-Adresse,
   Port, Web-Login).
7. Wähle **Verbindung testen**. TalkOps zeigt das Modell der VTO.

### Einstellungen in der VTO-Weboberfläche

Melde dich an der Weboberfläche der VTO an. Gehe zu **Network → SIP Server**
(manche Firmware: **Network Settings → SIP Server**). Die Namen folgen
Dahuas Anleitungen (VTO-Kurzanleitung v4.5, Handbuch VTO2311R-WP).

| Feld der VTO | Wert |
|---|---|
| SIP Server (Enable) | **Aus**. TalkOps ist der SIP-Server. |
| Server Type | *ThirdParty* (manche Firmware: *Asterisk*) |
| IP Addr. | IP-Adresse oder Hostname des TalkOps-Servers |
| Port | `5060` |
| Username + **Enable** | **Enable** aktivieren. Den SIP-Benutzernamen aus TalkOps eintragen, zum Beispiel `8000vto`. Ohne *Enable* nutzt die VTO ihre eigene VTO-Nummer. |
| Password | das SIP-Passwort aus TalkOps |
| SIP Domain | `talkops.local` oder die Dahua-Vorgabe `VDP`. TalkOps akzeptiert jede Domain. |
| SIP Server Username | **derselbe** SIP-Benutzername wie bei *Username*, nicht `admin` |
| SIP Server Password | **dasselbe** SIP-Passwort |

TalkOps akzeptiert eine Registrierung nur, wenn der Authentifizierungsname
gleich dem SIP-Benutzernamen ist. *Username* und *SIP Server Username*
müssen daher identisch sein.

- **Local Settings → Basic → Call No.** (Vorgabe `9901`) ist die Nummer, die
  die VTO beim Klingeln wählt. TalkOps leitet jeden Anruf der VTO an das
  Klingelziel. Bei Stationen mit mehreren Tasten in TalkOps unter **Tasten**
  ein Ziel pro Tastennummer festlegen.
- Optional: ein Öffnungscode in der VTO. Wähle **nur eine** DTMF-Methode:
  **RFC 2833**. Laut Dahua funktioniert keine Methode, wenn RFC 2833 und SIP
  INFO zusammen gewählt sind.

### Speichern, neu starten und prüfen

1. Wähle **OK** oder **Save**.
2. Starte die VTO neu, wenn sich der Registrierungsstatus nach einer Minute
   nicht ändert (zum Beispiel **Maintenance → Reboot**).
3. Öffne in TalkOps die **Übersicht**. *Registrierte Geräte* zeigt
   `8000vto`.
4. Drücke die Taste an der VTO. Das Klingelziel klingelt.

## VTH (Innenstation)

Jeder VTH registriert sich als eigenes SIP-Gerät bei TalkOps. Dann können
die VTO und alle Telefone den VTH anrufen.

**Wichtig:** Der VTH sendet seine **Room No.** (Raumnummer) als
SIP-Benutzernamen. Die Room No. muss daher identisch mit dem
SIP-Benutzernamen in TalkOps sein, zum Beispiel `8001vth`. Groß- und
Kleinschreibung gleich übernehmen.

### Einstellungen in TalkOps

1. Öffne **Einstellungen → Nebenstellen** und lege für jeden VTH eine
   Nebenstelle an, zum Beispiel `8001` „Innenstation Flur“.
2. Wähle in der Nebenstelle **Gerät hinzufügen**. Setze den Typ auf
   **Türsprechstelle** und die Endung des SIP-Benutzernamens auf `vth`
   (SIP-Benutzername `8001vth`). TalkOps hat keinen eigenen Typ für
   Innenstationen. Der Typ bestimmt nur den vorgegebenen Benutzernamen und
   das Passwortformat.
3. Behalte das SIP-Passwortformat **Kompatibel (15 Zeichen)**.
4. Speichere. Kopiere SIP-Benutzernamen und SIP-Passwort.

### Einstellungen in ConfigTool

1. Aktiviere den VTH mit ConfigTool und setze seinen Geräte-Login.
2. Öffne **ConfigTool → Device Config**. Wähle die IP-Adresse des VTH und
   **Get Device Info**. Melde dich mit dem Geräte-Login an, nicht mit dem
   SIP-Passwort.
3. Trage diese Werte ein:

   | Feld in ConfigTool | Wert |
   |---|---|
   | Room No. | der SIP-Benutzername aus TalkOps, exakt, zum Beispiel `8001vth` |
   | VTH | **Main VTH**. Jeder VTH, der sich bei TalkOps registriert, ist ein Haupt-VTH. |
   | Sip Server IP | IP-Adresse oder Hostname des TalkOps-Servers |
   | Sip Server Port | `5060` |
   | Sip Register Pwd | das SIP-Passwort aus TalkOps |
   | Domain (falls vorhanden) | `talkops.local` oder `VDP` |
   | Initiative Mode (Yeastar-Anleitung: „Initiale Mode“) | An. Der VTH registriert sich beim SIP-Server. |
   | Login Password / Benutzername des Haupt-VTH | **Nicht** das SIP-Passwort. ConfigTool füllt diese Werte aus. Für TalkOps nicht ändern. |

4. Wähle **OK**. ConfigTool meldet, wenn der VTH die Einstellungen
   übernommen hat.

### Einstellungen am VTH-Bildschirm

Manche Firmware zeigt dieselben Werte am VTH (**Einstellungen →
Projektpasswort → Netzwerk** oder **SIP-Server**).

- Ändere Werte in ConfigTool oder am VTH-Bildschirm, nicht gleichzeitig an
  beiden Stellen.
- Prüfe, dass der VTH-Bildschirm dieselbe Room No., denselben SIP-Server und
  Port wie ConfigTool zeigt.

*Nicht bestätigt:* die Menüs am VTH-Bildschirm für Firmware V4.800.0.2.

### Haupt- und Nebengeräte

- Stelle jeden VTH als **Main VTH** mit eigener Room No. und eigenem
  SIP-Konto ein.
- Um mehrere VTH klingeln zu lassen, nutze eine Rufgruppe (siehe
  [Türsprechstellen](tuersprechstelle.md#klingelziel-und-rufgruppe)).
- Ein Dahua-Neben-VTH (Room No. mit `-1`, `#1` oder ähnlich) verbindet sich
  im Dahua-System mit seinem Haupt-VTH, nicht mit TalkOps.

*Nicht bestätigt:* Neben-VTH, die über einen Haupt-VTH verbunden sind, sind
mit TalkOps nicht getestet.

### Speichern, neu starten und prüfen

1. Wähle **OK** in ConfigTool.
2. Starte den VTH neu (ConfigTool oder Stromversorgung).
3. Öffne in TalkOps die **Übersicht**. *Registrierte Geräte* zeigt den
   SIP-Benutzernamen, zum Beispiel `8001vth`.
4. Rufe die Nebenstelle des VTH von einem Telefon an. Der VTH klingelt.

## SIP-Passwort und Gerätepasswort

| Passwort | Wo eintragen | Zweck |
|---|---|---|
| SIP-Passwort (aus TalkOps) | VTO: *Password* und *SIP Server Password*. VTH: ConfigTool → *Sip Register Pwd* | SIP-Registrierung und Anrufe zu TalkOps |
| Web-Login der VTO | TalkOps: Türsprechstelle → **HTTP-Zugang** | Türöffner, Bilder und Ereignisprotokoll. Nicht für SIP. |
| Geräte-Login des VTH | Anmeldung in ConfigTool, Weboberfläche | Konfiguration des VTH. TalkOps nutzt es nicht. |
| Login Password (Haupt-VTH) | ConfigTool → Local Info | Verbindung zwischen Dahua-Geräten. Nicht das SIP-Passwort. |

- Nutze unterschiedliche Passwörter. Ein neues SIP-Passwort in TalkOps
  ändert den Geräte-Login nicht. Ein neuer Geräte-Login ändert das
  SIP-Passwort nicht.
- Die Anzahl der Maskierungszeichen (`•••`) in ConfigTool zeigt nicht die
  echte Länge eines gespeicherten Passworts.

## Fehlersuche für Dahua

| Was du siehst | Was zu tun ist |
|---|---|
| Keine SIP-Pakete erreichen TalkOps | SIP-Server-Adresse und Port im Gerät prüfen. Netzwerk, VLAN und Firewall prüfen. VTO: *SIP Server (Enable)* muss aus sein. Gerät neu starten. |
| Nur `401 Unauthorized` | Das ist die normale erste Antwort. Das Gerät muss ein zweites REGISTER mit Passwort senden. Fehlt es, SIP-Passwort neu eintragen und Gerät neu starten. |
| `401`, danach `403 Forbidden` | (1) Falsches oder gekürztes Passwort: neues Passwort im Format **Kompatibel (15 Zeichen)** erzeugen und eintragen. (2) Unbekannter SIP-Benutzername: Die Room No. des VTH weicht vom SIP-Benutzernamen ab. (3) VTO: *SIP Server Username* weicht von *Username* ab. (4) TalkOps hat die Adresse gesperrt (siehe [SIP-Anmeldeschutz](tuersprechstelle.md#sip-anmeldeschutz)). |
| `200 OK`, aber eine Funktion fehlt | Siehe [Fehlersuche](tuersprechstelle.md#fehlersuche). |
| Ruf von der VTO: Ton geht, der VTH zeigt kein Bild | TalkOps bietet den gerufenen Geräten nur den Video-Codec der VTO an (H264). FreeSWITCH wandelt Video nicht um. Ein Gerät, das nur VP8 kann, bekommt kein Bild. Prüfe, dass die VTO H264 sendet. |
| Ruf zur VTO endet nach 30 Sekunden | Umlaute im Anrufernamen sind eine vermutete Ursache dafür, dass die VTO V4.600 das ACK ablehnt. Ein Hardwaretest muss das bestätigen. TalkOps sendet bei Rufen von und zu Türstationen die Anrufernamen in ASCII (`ü` → `ue`). Der Anrufverlauf zeigt den vollen Namen. Ältere TalkOps-Versionen: einen Namen ohne Umlaute verwenden. |

Das TalkOps-Log zeigt SIP-Benutzernamen und Adresse jeder fehlgeschlagenen
Anmeldung. Eine Zeile `SIP authentication for unknown or disabled user` mit
dem Benutzer `8001` zeigt: Der VTH sendet `8001` als Room No., TalkOps
erwartet aber einen anderen Benutzernamen.

```sh
docker compose logs -f talkops | grep "SIP authentication"
```

## Quellen

- [Yeastar: Register Dahua VTH indoor monitor](https://help.yeastar.com/en/p-series-appliance-edition/integrations/dahua/register-dahua-vth-indoor-monitor-to-yeastar-p-series-pbx-system.html)
  (Room No. = Nebenstellennummer, *Sip Register Pwd*, 8–15 Zeichen; die
  getesteten Modelle sind nicht genannt)
- Dahua VTO-Kurzanleitung v4.5, Handbuch VTO2311R-WP
