# Yealink-Telefone einrichten

TalkOps richtet Yealink-Tischtelefone (T3x, T4x, T5x, VP59, CP9x0),
DECT-Basen (W60B, W70B, W80B) und WLAN-Mobilteile (AX83H, AX86R) automatisch
ein: SIP-Konten, Funktionstasten mit BLF, Telefonbücher, Zeitzone, Sprache,
Admin-Passwort und Firmware.

**AX83H/AX86R:** Die Mobilteile haben kein LAN; die Provisioning-URL daher
entweder per DHCP-Option 66 im WLAN-Netz verteilen oder am Telefon unter
*Einstellungen → Auto Provision* eintragen. Bis zu 4 SIP-Konten, keine
Funktionstasten. Ab Firmware 180.86.

> **Status:** Die Parameter stammen aus dem offiziellen *Yealink Auto
> Provisioning Guide*. Tests mit echten Geräten stehen noch aus –
> Rückmeldungen (auch „funktioniert mit T46U, Firmware …“) sind willkommen.

## 1. Telefon anlegen

1. **Telefone → Telefon hinzufügen**: Name, MAC-Adresse (Aufkleber auf der
   Rückseite) und Modell eintragen.
2. **Nebenstellen → Nebenstelle → Gerät hinzufügen**: unter *Telefon* das eben
   angelegte Telefon wählen. Das *Konto* (1, 2, …) wird automatisch vergeben;
   bei DECT-Basen entspricht es dem Mobilteil.

Ein Telefon kann mehrere Nebenstellen tragen (z. B. Konto 1 = eigene
Nebenstelle, Konto 2 = Zentrale).

## 2. Telefon auf TalkOps zeigen lassen

Unter **Telefone → Auto-Provisioning → Provisioning-URL anzeigen** steht die
URL mit Zugangsdaten, z. B.

```
http://provision:Ab3dE…@192.168.1.10:8080/provisioning
```

**Variante A – DHCP-Option 66 (empfohlen):** Trage die URL im DHCP-Server als
Option 66 (Typ *Text/String*) ein. Neue oder zurückgesetzte Telefone holen
sich dann alles selbst.

| DHCP-Server | Einstellung |
|---|---|
| UniFi | *Settings → Networks → Netzwerk → DHCP → Custom DHCP Option*: Code 66, Typ Text |
| OPNsense / pfSense | *Services → DHCPv4 → LAN → TFTP server / Additional options*: Nummer 66, Typ Text |
| dnsmasq / Pi-hole | `dhcp-option=66,"http://provision:…@192.168.1.10:8080/provisioning"` |
| FRITZ!Box | kann keine Option 66 – Variante B nutzen |

**Variante B – von Hand:** In der Weboberfläche des Telefons (Standard-Login
`admin`/`admin`) unter *Einstellungen → Auto Provision*: *Server URL* =
`http://192.168.1.10:8080/provisioning`, *Benutzername* `provision`,
*Passwort* aus TalkOps, dann **Autoprovision Now**.

Das Telefon startet neu und registriert sich. Danach gilt das in TalkOps
angezeigte **Admin-Passwort der Telefone** für die Weboberfläche des
Telefons. In der Telefonliste erscheinen *letzter Kontakt*, IP-Adresse und
Firmware-Version.

## 3. Funktionstasten und BLF

Auf der Seite des Telefons lassen sich die Tasten belegen:

- **Leitung** – Taste für ein Konto (Standard für die ersten Tasten).
- **BLF (Nebenstelle)** – zeigt, ob eine Nebenstelle frei ist, klingelt oder
  telefoniert. Ein Druck auf eine blinkende BLF-Taste übernimmt den Anruf.
- **Kurzwahl** – wählt eine Nummer.

Nach dem Speichern **Resync** drücken: Das Telefon startet neu und übernimmt
die Änderungen. Nicht registrierte Telefone holen die Konfiguration beim
nächsten Start bzw. nachts zwischen 2 und 4 Uhr.

## 4. Kurzwahlen am Telefon

| Code | Funktion |
|---|---|
| `*78` / `*79` | Nicht stören an / aus |
| `*72<Nummer>` / `*73` | alle Anrufe umleiten (Nebenstelle oder externe Nummer) / aus |
| `**<Nebenstelle>` | klingelnden Anruf heranholen |
| `*51` … `*59` | Anruf parken / geparkten Anruf holen ([Anrufsteuerung](anrufsteuerung.md)) |
| `*30<Nummer>` | Zeitsteuerung „geschlossen“ ein/aus |
| `*97` / `*98` | eigene Voicemail / beliebige Voicemail mit PIN ([Voicemail](voicemail.md)) |
| `*85` / `*85<Nebenstelle>` | Tür öffnen (erste Türsprechstelle / die mit dieser Nebenstelle), `*86…` für das zweite Schloss ([Türsprechstelle](tuersprechstelle.md)) |

Die DND-Taste des Telefons schaltet *Nicht stören* der Nebenstelle auf
Konto 1. Beides lässt sich auch in der Weboberfläche bei der Nebenstelle bzw.
unter *Meine Telefone* einstellen.

## 5. Telefonbuch

Die Telefone zeigen zwei Telefonbücher: **Intern** (alle aktiven
Nebenstellen) und **Kontakte** (gemeinsames Telefonbuch unter
*Telefonbuch* in der Weboberfläche). Nummern werden national angezeigt und
können direkt gewählt werden.

## 6. Firmware

Firmware von der Yealink-Supportseite herunterladen (`.rom` bzw. `.bin`) und
unter **Telefone → Firmware** für das passende Modell hochladen und
**aktivieren**. Die Telefone aktualisieren sich beim nächsten Resync. Pro
Modell ist höchstens eine Firmware aktiv.

## Fehlersuche

- **Telefon holt nichts:** Ist Port 8080 vom Telefonnetz erreichbar? Im
  Container-Log von `talkops` steht `provisioning request from unknown phone`,
  wenn die MAC nicht angelegt ist.
- **401 im Telefon-Log:** Zugangsdaten falsch oder neu erzeugt – DHCP-Option 66
  bzw. Auto-Provision-Einstellungen anpassen. Nach 10 Fehlversuchen sperrt
  TalkOps die IP für 5 Minuten.
- **BLF-Taste bleibt dunkel:** Die überwachte Nebenstelle braucht mindestens
  ein aktives Gerät; nach Tastenänderungen Resync ausführen.
- **Konfiguration prüfen:** *Konfiguration anzeigen* auf der Telefonseite zeigt
  die erzeugte Datei (enthält SIP-Passwörter, der Abruf wird protokolliert).

## Sicherheit

Provisioning läuft über HTTP im LAN. Die Dateien enthalten SIP-Passwörter;
betreibe TalkOps daher nur in einem vertrauenswürdigen Netz oder hinter dem
optionalen Caddy mit HTTPS (dann `https://…/provisioning` verwenden). Mit
**Neue Passwörter erzeugen** werden Provisioning- und Telefon-Admin-Passwort
ausgetauscht.
