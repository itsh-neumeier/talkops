# Yealink-Telefone einrichten

Geräte-Ausnahmen für HTTP/HTTPS, zusätzliche Tastengruppen, Vorlagen und
Resync-Diagnose beschreibt [Provisionierungsprofile und Vorlagen](provisionierung.md).

TalkOps richtet Yealink-Tischtelefone (T3x, T4x, T5x, VP59, CP9x0),
DECT-Basen (W60B, W70B, W80B) und WLAN-Mobilteile (AX83H, AX86R) automatisch
ein: SIP-Konten, Funktionstasten mit BLF, Telefonbücher, Zeitzone, Sprache,
Admin-Passwort und Firmware.

**AX83H/AX86R:** Die Mobilteile haben kein LAN; die Provisioning-URL daher
entweder per DHCP-Option 66 im WLAN-Netz verteilen oder am Telefon unter
*Einstellungen → Auto Provision* eintragen. Bis zu 4 SIP-Konten,
16 Funktionstasten (Leitung und BLF, Tasten 1–4 im Ruhebildschirm; Kurzwahl
kennen die Mobilteile nicht), eigener Klingelton und Hintergrund.
Funktionstasten brauchen **Firmware 180.87.0.15** oder neuer; AX83H und AX86R
nutzen dieselbe Firmware-Datei (`AX86(AX83,AX86)-….rom`) – unter
*Firmware* für beide Modelle hochladen. Das WLAN selbst (SSID, Kennwort)
richtest du am Mobilteil ein, TalkOps verteilt es nicht.

> **Status:** Die Parameter stammen aus dem offiziellen *Yealink Auto
> Provisioning Guide*. Tests mit echten Geräten stehen noch aus –
> Rückmeldungen (auch „funktioniert mit T46U, Firmware …“) sind willkommen.

## 1. Telefon anlegen

1. **Einstellungen → Telefone (Provisionierung) → Telefon hinzufügen**: Name,
   MAC-Adresse (Aufkleber auf der Rückseite) und Modell eintragen; unter
   *Nebenstelle zuweisen* gleich die Nebenstelle wählen (oder später).
2. Weitere Nebenstellen auf der Seite des Telefons unter *Konten auf diesem
   Telefon → Nebenstelle zuweisen*, ✕ entfernt eine Zuweisung. Alternativ
   von der Nebenstelle aus: *Gerät hinzufügen* und unter *Telefon* das Telefon
   wählen. Das *Konto* (1, 2, …) wird automatisch vergeben; bei DECT-Basen
   entspricht es dem Mobilteil.

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

Nach dem Speichern **Resync** drücken: Das Telefon lädt seine Konfiguration
neu und übernimmt die Änderungen, **ohne neu zu starten**. **Neustarten**
startet es zusätzlich neu (während eines Gesprächs startet ein Yealink nicht
neu). Telefone, die noch eine Konfiguration von TalkOps 1.7.3 oder älter
haben, starten beim ersten Resync ein letztes Mal neu. Technisch: SIP NOTIFY
`check-sync;reboot=false` bzw. `reboot=true` mit
`sip.notify_reboot_enable = 0` (Yealink-Doku *Phone Reboot* und *Trigger the
Phone to Perform Auto Provisioning*). Nicht registrierte Telefone holen die Konfiguration beim
nächsten Start bzw. nachts zwischen 2 und 4 Uhr.

### Grafische Funktionstasten und Kurzwahl

Tischtelefone zeigen auf ihrer Telefonseite eine Gerätegrafik und ein
anklickbares Display. Taste oder Beschriftung auswählen, um den Tastendialog
zu öffnen. **Liste** zeigt alle Tasten als Tabelle. Beide Ansichten bearbeiten
dieselben Belegungen.

| Unterstützte Modelle | DSS-Tasten je Telefon |
| --- | --- |
| T31G | 2 |
| T33G | 12 |
| T41P, T41S, T42U | 15 |
| T43U, T53, T53W | 21 |
| T46G, T46S, T46U, T54W, T58W, VP59 | 27 |
| T48G, T48S, T48U, T57W | 29 |
| T73W | 40 |
| T85W | 60 |
| T87W | 84 |

Die Grafiken sind schematisch. Bei älteren Telefonen dient die Taste unten
rechts zum Seitenwechsel. TalkOps hält diese Taste als Seitensteuerung frei,
damit das Telefon zur grafischen Belegung passt. T73W und T85W haben eine
separate Seitensteuerung. Touchscreen-Modelle zeigen eine scrollbare
DSS-Übersicht, keine Kopie jeder Firmware-Startansicht.

1. Taste auswählen.
2. **Kurzwahl**, **BLF**, **Leitung** oder **Unbelegt** wählen. Die angebotenen
   Funktionen richten sich nach Modell und Tastenposition.
3. Für eine Kurzwahl **Telefonbuch**, **Nebenstelle** oder **Freie Nummer** wählen.
4. Im Telefonbuch globale Kontakte oder ein diesem Telefon zugewiesenes Buch
   wählen. Kontakt suchen und geschäftliche, mobile oder weitere Nummer wählen.
5. Beschriftung und verwendetes Konto prüfen.
6. **Übernehmen** ändert den Entwurf auf der Telefonseite.
7. **Speichern** oder **Speichern & Resync** auf der Telefonseite wählen.

Die Auswahl kopiert Nummer und Kontaktname. Die Beschriftung bleibt
bearbeitbar. Spätere Kontaktänderungen ändern gespeicherte Tasten nicht
automatisch. Interne Kurzwahl nutzt `**<Nebenstelle>`. BLF nutzt die
Nebenstellennummer zur Überwachung und `*8*<Nebenstelle>` zum Heranholen.

**Standard** entfernt die eigene Belegung und stellt die automatische
Belegung wieder her. **Unbelegt** leert die Taste ausdrücklich. Für eine
aktive Taste muss ein Konto zugewiesen sein. Speichern & Resync speichert
zuerst Telefon und Kontobeschriftungen. Erst danach wird Resync angefordert.
Ein fehlgeschlagener Resync macht das Speichern nicht rückgängig. Ein
Offline-Telefon übernimmt die Konfiguration beim nächsten Provisionierungsabruf.

Die dokumentierten Grenzen korrigieren T33G von 4 auf 12, T46U von 40 auf 27
und VP59 von 29 auf 27 Tasten. Nicht unterstützte Altbelegungen bleiben
erhalten und werden in einem Korrekturbereich angezeigt. Andere Einstellungen
lassen sich mit unveränderten Altbelegungen weiter speichern. Werte bei
Bedarf notieren und Belegungen ausdrücklich entfernen oder unterstützten
Tasten zuweisen. TalkOps löscht sie nicht still.

Der Editor benötigt keine Datenbankmigration und keine neuen Pakete. Neue
Modelle nutzen die vorhandenen Provisionierungszugangsdaten. Quellen stehen
im [Entwurfsdokument](../deskphone-key-editor-proposal.md). Hardwaretests stehen
aus; automatisierte Tests bestätigen keine bestimmte Telefon-Firmware.

### Label und Anzeigename

Unter *Konten auf diesem Telefon* hat jedes Konto zwei Felder (auch im
Geräte-Dialog der Nebenstelle, sobald ein Telefon gewählt ist):

- **Label am Telefon** – steht an der Leitungstaste und im Ruhebildschirm
  (leer = Nebenstellennummer), höchstens 32 Zeichen.
- **Anzeigename** – der Name, den das Telefon als Anrufer sendet (leer =
  Name der Nebenstelle), höchstens 64 Zeichen.

Als Gerätetyp gibt es neben *Tischtelefon* und *DECT-Mobilteil* auch
**WLAN-Mobilteil** (AX83H/AX86R).

### Klingelton und Hintergrundbild

Unter **Einstellungen → Telefone → Klingeltöne und Hintergründe** hochladen:

- **Klingelton:** jede Audiodatei, die der Browser abspielt (MP3, WAV, …). Sie
  wird in das Yealink-Format umgewandelt (WAV, 8 kHz, mono, 16 Bit) und auf
  6, 15 oder 30 Sekunden gekürzt. T42U, T43U und T53W nehmen Klingeltöne nur
  bis 100 KB (gut 6 Sekunden), T46U/T48U/T54W/T57W bis 8 MB.
- **Hintergrundbild:** jedes Bild (JPEG, PNG, …); es wird auf höchstens
  1280 × 800 verkleinert und als JPEG gespeichert (Yealink: höchstens 5 MB).
  Unterstützt von T43U, T46U, T48U, T53W, T54W und T57W.

Auf der Seite des Telefons unter *Klingelton, Hintergrund und Telefonbuch*
auswählen, speichern, **Resync**. Das Telefon lädt die Datei herunter
(`ringtone.url`/`wallpaper_upload.url`) und stellt sie ein
(`phone_setting.ring_type`/`phone_setting.backgrounds`). *Einstellung des
Telefons behalten* lässt beides unangetastet. Modelle ohne dokumentierte
Unterstützung (T31G, T33G, T58W, VP59, Konferenztelefone, DECT) zeigen die
Auswahl nicht.

Quellen: Yealink *SIP-T5 Series Administrator's Guide*, Abschnitte *Ring
Tones* und *Wallpaper Customization*; für AX83H/AX86R die Yealink-Doku
*Ring Tones*, *Wallpaper Settings* und *Line Key* (support.yealink.com).

## 4. Kurzwahlen am Telefon

| Code | Funktion |
|---|---|
| `*78` / `*79` | Nicht stören an / aus |
| `*72<Nummer>` / `*73` | alle Anrufe umleiten (Nebenstelle oder externe Nummer) / aus |
| `**<Nebenstelle>` | interne Nebenstelle anrufen |
| `*8*<Nebenstelle>` | klingelnden Anruf heranholen |
| `*51` … `*59` | Anruf parken / geparkten Anruf holen ([Anrufsteuerung](anrufsteuerung.md)) |
| `*30<Nummer>` | Zeitsteuerung „geschlossen“ ein/aus |
| `*97` / `*98` | eigene Voicemail / beliebige Voicemail mit PIN ([Voicemail](voicemail.md)) |
| `*85` / `*85<Nebenstelle>` | Tür öffnen (erste Türsprechstelle / die mit dieser Nebenstelle), `*86…` für das zweite Schloss ([Türsprechstelle](tuersprechstelle.md)) |

Die DND-Taste des Telefons schaltet *Nicht stören* der Nebenstelle auf
Konto 1. Beides lässt sich auch in der Weboberfläche bei der Nebenstelle bzw.
unter *Einstellungen → Meine Telefone* einstellen.

### Sofortwahl (Wählplan)

Ohne Wählplan wartet ein Yealink nach der letzten Ziffer einige Sekunden,
bevor es wählt (oder bis du `#` bzw. *Senden* drückst). TalkOps schickt den
Telefonen deshalb Sofortwahl-Regeln (`dialplan.dialnow.rule.1`–`20`); diese
Nummern gehen nach einer Sekunde raus:

- Notrufnummern (110, 112 bzw. die unter *Einstellungen → Telefonie*
  eingetragenen),
- die festen Codes `*51`–`*59`, `*73`, `*78`, `*79`, `*97`, `*98`,
- ausdrücklich interne Nummern wie `**101` (Nebenstellen, Gruppen, Menüs, Warteschlangen).

Ausgenommen ist jede Nummer, mit der eine andere wählbare Nummer beginnt –
gibt es z. B. `**300` und `**3000`, wartet das Telefon bei `**300` weiter.
`**101` und `**510` kollidieren nicht mit Systemcodes mit einem Stern.
Interne Nummern ohne Präfix ab `11` (Servicenummern wie 11833) und – wenn eine
Ortsvorwahl eingetragen ist – alle internen Nummern bleiben ebenfalls ohne
Sofortwahl, weil dann Ortsnummern ohne Vorwahl gewählt werden können. Neue
Nebenstellen kommen beim nächsten Resync aufs Telefon (spätestens nachts).

Quellen: Yealink-Doku *Dial Plan* (support.yealink.com) und *Using Dial
Plan Feature on Yealink SIP-T3XG Phones*.

### Komfort-Einstellungen

Unter **Telefone → Komfort-Einstellungen** legst du für alle passenden
Telefone fest, was TalkOps einstellt; auf der Seite eines Telefons lässt
sich jeder Wert abweichend setzen. *Einstellung des Telefons behalten*
(Standard) schreibt nichts – das Telefon behält, was am Gerät eingestellt
ist; dahinter steht der Werkswert, sofern Yealink ihn dokumentiert.
Änderungen kommen mit dem nächsten **Resync** aufs Telefon.

Derzeit für die WLAN-Mobilteile AX83H/AX86R:

| Bereich | Einstellung | Parameter |
|---|---|---|
| Töne | Tastenton, Ton beim Einlegen, Vibration | `features.send_key_tone`, `features.charging_tone.enable`, `phone_setting.vibrate.enable` |
| Ladeschale | Abnehmen nimmt an, Einlegen beendet | `phone_setting.off_cradle_auto_answer.enable`, `phone_setting.end_call_on_hook.enable` |
| Anrufe | Anklopfen, Anklopfton | `call_waiting.enable`, `call_waiting.tone` |
| Display | Beleuchtungsdauer, Helligkeit, 12/24 h | `phone_setting.backlight_time`, `phone_setting.active_backlight_level`, `local_time.time_format` |
| Hinweise | Entgangene Anrufe, neue Voicemail | `features.missed_call_popup.enable`, `features.voice_mail_popup.enable` |
| Audio | Rauschfilter, Akustik-Schild | `features.noise_filtering_rev.enable`, `features.acoustic_shield.mode` |

Die Liste steht in `presets/phones/yealink.yaml` (`settings:`) und lässt sich
dort erweitern. Quellen: Yealink-Doku zum AX86R (Firmware 180.87) auf
support.yealink.com; die Vibration steht nur in Yealinks
Provisioning-Vorlage AX8X 180.87.0.5.

### Audio-Codecs

TalkOps schreibt Codec-Einstellungen nur für Geräte mit dem Codec-Profil HD
oder Robust. WLAN-Handsets nutzen standardmäßig HD. Siehe
[Codec-Profile](codec-profile.md).

## 5. Telefonbuch

Jedes Telefon zeigt **Intern** (alle aktiven Nebenstellen) und **Kontakte**
(das globale Telefonbuch: alle Kontakte ohne Bereich). Im internen Telefonbuch
steht `**101` für einen Anruf der Nebenstelle `101`. TalkOps ergänzt dabei keine
Landes- oder Ortsvorwahl. Die Nebenstellen- und SIP-Kennungen in der Datenbank
bleiben unverändert. Externe Kontakte werden national angezeigt und können
direkt gewählt werden.

Trage interne Kontakte als `**101` in ein Rufnummernfeld oder eine CSV-Datei ein.
TalkOps speichert das Präfix und gibt es in globalen und Bereichstelefonbüchern
unverändert aus. Eine externe Nummer wie `+499543101` wird nicht automatisch
umgeschrieben. Korrigiere einen falsch gespeicherten Kontakt selbst auf `**101`.

**Bereiche** sind zusätzliche Telefonbücher, die nur auf ausgewählten
Telefonen erscheinen – z. B. *Familie* nur im Wohnzimmer, *Lieferanten* nur
im Büro:

1. *Telefonbuch → Bereiche*: Bereich anlegen.
2. Beim Kontakt unter *Bereich* den Bereich statt *Global* wählen.
3. Auf der Seite des Telefons unter *Telefonbuch-Bereiche* bis zu drei
   Bereiche ankreuzen, speichern, **Resync**.

Ein gelöschter Bereich verschwindet mit seinen Kontakten auch von den
Telefonen.

### Kontakte suchen, sortieren und filtern

Die Seite *Telefonbuch* lädt alle Kontakte. Der Browser sortiert und filtert
sie, daher wirkt jede Änderung sofort.

- **Sortieren:** auf den Spaltentitel **Name**, **Firma** oder **Bereich**
  klicken. Ein zweiter Klick kehrt die Reihenfolge um (aufsteigend,
  absteigend). Der Pfeil zeigt die aktuelle Reihenfolge. Kontakte ohne Firma
  stehen bei der Sortierung nach Firma am Ende. Bei der Sortierung nach
  Bereich steht das globale Telefonbuch zuerst.
- **Filtern:** eine Spalte mit Filter hat eine Trichter-Schaltfläche. Ein
  farbiger Punkt zeigt einen aktiven Filter.

  | Spalte | Filter |
  | --- | --- |
  | Geschäftlich, Mobil, Sonstige | Alle, mit Nummer oder ohne Nummer. |
  | Bereich | Ein oder mehrere Telefonbücher: *Global* und die Bereiche. |

- **Suchen:** das Suchfeld findet Text im Namen, in der Firma, im
  Bereichsnamen und in den Nummern. Eine Suche nur mit Ziffern, z. B.
  `0951123`, findet auch `+49 951 123…`.
- **Alle Filter zurücksetzen** löscht die Filter und die Suche. Die
  Sortierung bleibt.

Die Seitenadresse enthält Filter und Sortierung. Ein Neuladen oder ein
kopierter Link zeigt daher dieselbe Liste. Ein Link aus der Anrufliste oder
den laufenden Gesprächen (`/phonebook?contact=<id>`) hebt den Eintrag hervor.
Wenn die aktuellen Filter den Eintrag verbergen, ersetzt die Seite sie durch
den Bereich des Eintrags.

Die Spalte *Bereich* und ihr Filter erscheinen nur, wenn Bereiche existieren.
Auf schmalen Bildschirmen ist die Spalte *Sonstige* ausgeblendet.

### Umstellung und Anrufübernahme

`**101` ruft jetzt die Nebenstelle `101` an und übernimmt keinen klingelnden
Anruf mehr. Verwende dafür `*8*101`. Führe nach dem Update einen Resync aller
provisionierten Telefone durch und lade das entfernte Telefonbuch neu.
TalkOps aktualisiert dabei die BLF-Übernahmecodes. Passe auch manuell eingerichtete
BLF-Tasten, Übernahmecodes und gespeicherte Kurzwahlen an. Interne Anrufe ohne
Präfix und der bisherige Alias mit einem Stern bleiben möglich. Unbekannte
explizite interne Ziele liefern `404` und werden nicht über einen Trunk gewählt.

### Kontakte importieren (CSV)

*Telefonbuch → Importieren*: CSV-Datei wählen, TalkOps zeigt sofort eine
Vorschau (neu / unverändert / fehlerhafte Zeilen mit Zeilennummer), erst
**Importieren** speichert. Die **Beispieldatei** gibt es im Dialog zum
Herunterladen (`telefonbuch-beispiel.csv`):

```csv
Name;Firma;Geschäftlich;Mobil;Privat;Bereich
Erika Mustermann;Musterfirma GmbH;030 23125 101;0171 3920001;;
Max Mustermann;;;0171 3920002;030 23125 102;Familie
```

- **Trennzeichen** Semikolon, Komma oder Tab, Kodierung UTF-8 oder Windows
  (Excel „CSV (Trennzeichen-getrennt)“) – beides wird erkannt.
- **Spalten** werden am Namen in der Kopfzeile erkannt, die Reihenfolge ist
  egal: *Name* (oder *Vorname*/*Nachname*, sonst *Firma*), *Firma*,
  *Geschäftlich*, *Mobil*, *Privat*/*Sonstige*, *Bereich*. Exporte aus
  Outlook (deutsch/englisch) und Google Kontakte passen ohne Umbau.
- **Nummern** national oder international, Leerzeichen, `-`, `/`, `.` und
  `+49 (0)89 …` sind erlaubt. Excel zeigt lange Nummern gern als `4,9171E+11`
  – Spalte vorher als *Text* formatieren.
- **Bereich:** leer = der im Dialog gewählte Bereich (Standard *Global*);
  unbekannte Bereiche werden angelegt.
- **Doppelte:** Ein Kontakt mit gleichem Namen im gleichen Bereich wird nicht
  doppelt angelegt; mit *Vorhandene Kontakte aktualisieren* werden seine
  Nummern durch die aus der Datei ersetzt.

Bis zu 5000 Zeilen pro Datei; Zeilen ohne Name oder Nummer werden übersprungen.

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
