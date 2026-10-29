# Herstellereinstellungen und Gerätekopien

[Handbuch](index.md) · [English](../en/manufacturer-provisioning.md)

## Voraussetzungen

Du benötigst: phones.manage sowie die Anleitung des genauen Gerätemodells und seiner Firmware.

Wähle beim Hinzufügen das genaue Modell. Die Quellbibliothek enthält den
vollständigen festgeschriebenen FusionPBX-Provisionierungsbaum. Für Spectralink
und Grandstream Wave fehlen dort nutzbare Vorlagen. Sie bleiben als gesperrte
Einträge sichtbar.

## Herstellereinstellungen bearbeiten

Öffne ein Telefon und den Bereich **Modelleinstellungen**. Die Felder sind nach
Funktion gegliedert: Sprache und Zeit, Netzwerk, SIP, Anrufe, Audio, Display,
Tasten, Kontakte, Provisionierung, Wartung und Sicherheit. Die Suche erfasst
Parameternamen, Kommentare und Quelldateien. Öffne eine Kategorie und bei Bedarf
**Weitere Felder anzeigen**. **Nur Geräteüberschreibungen** filtert gesetzte Werte.

Jedes Feld zeigt seinen Quellparameter und seine Quelldatei. Kommentare erklären
verfügbare Optionen. Eine leere Eingabe setzt einen ausdrücklich leeren Wert.
**Vorlagenvorgabe verwenden** entfernt die Überschreibung. Ein gespeichertes
Geheimnis bleibt erhalten, wenn das Feld unverändert bleibt. Der Zurücksetzen-
Button entfernt es. Listen erwarten JSON-Textwerte, zum Beispiel `["112", "110"]`.

SIP-Zugangsdaten und Geräteidentität verwaltet TalkOps über die zugehörigen
Bedienelemente. Diese Felder sind im Herstellerformular schreibgeschützt.
Die Quellvorlage ersetzt kein vollständiges Firmwarehandbuch. Prüfe zulässige
Werte und zusätzliche Firmwarefunktionen im Gerätehandbuch. Quellumwandlung
und Prüfung am Gerät werden getrennt angezeigt.

Gemeinsame Sprache, IANA-Zeitzone und NTP-Server stehen unter
**Provisionierungstransport**. Jedes Telefon kann diese Werte überschreiben.
Herstellerfelder bieten die gerätespezifischen Formate und haben Vorrang.
Manche Telefone benötigen ein Sprachpaket. Nutze Herstellerfeld und Handbuch,
wenn das Profil keine Zuordnung aus dem gemeinsamen Format anbietet.

Die Kontenzahl folgt den Vorlagengrenzen. Fehlt eine Quellgrenze, beginnt das
Profil mit einem Konto. Trage die für dein Gerät dokumentierte Zahl ein.
Entferne höhere Kontozuordnungen, bevor du diese Zahl verringerst.

## Dateien bereitstellen

Native Profile zeigen ihr Provisionierungsverzeichnis und alle Dateien.
Verwende gemäß Gerätehandbuch die Verzeichnisadresse oder die Hauptdatei.
Die Dateien benötigen die Provisionierungszugangsdaten. Konfigurationsansicht
und QR-Export benötigen Telefonverwaltungsrechte und erzeugen Audit-Einträge.

Apps erhalten bei leerer Geräte-ID eine neue Kennung. Weise vor dem Export ein
SIP-Konto zu. Cloud Softphone benötigt zusätzlich seinen Cloud-Code. QR-Codes
enthalten Zugangsdaten und müssen geheim bleiben.

Profile ohne Fernsteuerungsadapter verwenden geplante Downloads oder das
Gerätemenü. Ihre Oberfläche bietet keinen entfernten Resync oder Neustart an.
TalkOps liefert HTTP(S). Für zwingendes TFTP ist ein separater Dienst nötig.
Die Swissvoice-Vorlagen enthalten keine Telefonbuchdatei. Die URL-Felder
bleiben zunächst leer und können auf ein kompatibles externes Telefonbuch
verweisen. Firmwaredateien werden nicht mitgeliefert.

## Telefon kopieren

1. Wähle **Einstellungen von einem Telefon übernehmen**.
2. Wähle die Quelle und prüfe die Kopie. Das Modell bleibt festgelegt.
3. Trage neuen Namen und MAC-Adresse ein. Apps können eine neue ID erzeugen.
4. Wähle bei Bedarf neue Nebenstellenzuordnungen.
5. Lege das Telefon an.

Die Kopie erhält eigene SIP-Zugangsdaten. Ausgeschlossen bleiben Quellpasswörter,
geheime Parameter, Laufzeitdaten, Quellkontobeschriftungen und gerätespezifische
Werte. Tasten ohne Konto bleiben inaktiv. Spätere Quelländerungen übertragen
sich nicht. Es wird kein Resync ausgelöst. Lade eine veraltete Vorschau neu.

Quellrevision und Lizenz stehen in der Quellbibliothek. Native Vorlagenkopien
behalten die Dateiliste ihres Modells. Die Aktivierung prüft Dateistruktur,
Zugangsdatenzuordnung sowie leere, einzelne und vollständig belegte Konten.

Weitere Angaben: [Provisionierungsprofile](provisionierung.md).

## Ergebnis prüfen

Prüfe abgerufene Dateien und Geräteverhalten; eine gerenderte Vorlage allein genügt nicht.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.

## Cisco-7800-Telefone mit MPP

Wähle **7811 MPP / 3PCC**, **7821 MPP / 3PCC**, **7841 MPP / 3PCC** oder
**7861 MPP / 3PCC**. Die Profile verwenden das Cisco-MPP-XML-Format auf Basis
der Dokumentation für MPP 11.3(1). Alle Profile sind **am Gerät ungetestet**.
Die stabilen Modell-IDs enden auf `-mpp`.

| Modell | Unabhängige SIP-Konten | Konten-Leitungszuordnung |
| --- | --- | --- |
| 7811 | 1 | Keine programmierbare Leitungstaste |
| 7821 | 2 | Tasten 1 bis 2 |
| 7841 | 4 | Tasten 1 bis 4 |
| 7861 | 16 | Tasten 1 bis 16 |

Ein SIP-Konto ist eine Registrierung mit eigenen Zugangsdaten. Eine
Leitungstaste kann dieses Konto darstellen. Mehrere Gespräche auf einem Konto
ergeben keine zusätzlichen Registrierungen. TalkOps ordnet Konten den Tasten
mit derselben Nummer zu. Nicht zugewiesene Slots werden deaktiviert und ihre
Zugangsdaten geleert. Eigene Tastenfunktionen, BLF und Kontakte sind nicht verfügbar.

### Firmware zuerst prüfen

MPP- und Enterprise-/CUCM-Firmware nutzen unterschiedliche Formate.
Eine PID mit `3PCC` bezeichnet das MPP-Produkt. Prüfe zusätzlich den installierten
Load. Eine Enterprise-PID allein beweist nicht die installierte Firmware.
Wähle bei Enterprise-Firmware kein MPP-Profil.

Für Enterprise-Firmware gibt es die unten beschriebenen separaten experimentellen
Profile. Verwende keine MPP-XML-Datei für ein Enterprise-Telefon. Bestehende
Cisco-8800-Profile belegen keine Kompatibilität mit der Enterprise-7800-Reihe.

### MPP-Telefon einrichten

Voraussetzungen: entsperrtes MPP-Telefon, Administratorzugang, DHCP, DNS und
Zugriff auf TalkOps-Provisionierung sowie SIP. Bei HTTPS muss das Telefon dem
Serverzertifikat vertrauen. TalkOps deaktiviert diese Prüfung nicht.
Ein manuell gewählter HTTP-Endpunkt ist nur in einem vertrauenswürdigen Netz geeignet.

1. Lege das Telefon mit dem genauen MPP-Modell und der MAC-Adresse an.
2. Weise Nebenstellen den gewünschten Kontennummern zu.
3. Öffne die Konfigurationsdateien des Telefons in TalkOps.
4. Kopiere die vollständige URL der Hauptdatei ohne eingebettete Zugangsdaten.
5. Öffne die Telefonwebseite als Administrator im erweiterten Modus.
6. Öffne **Voice > Provisioning**.
7. Trage eine Regel mit den von TalkOps angezeigten Provisionierungszugangsdaten ein:

   ```text
   [--uid PROVISIONING_USER --pwd PROVISIONING_PASSWORD] https://pbx.example/provisioning/devices/805ec0010203/CP-7861-3PCC805ec0010203.cfg
   ```

8. Ersetze Host, Zugangsdaten, Modellnummer und MAC durch die tatsächlichen Werte.
9. Speichere die Änderungen und starte am Telefon einen Resync.
10. Prüfe den erfolgreichen Dateiabruf in der TalkOps-Provisionierungsanfrageliste.

Verwende die **Provisionierungszugangsdaten**, nicht Web-Login, SIP-Konto oder
Telefon-Administratorpasswort. Die explizite URL bestimmt den Dateinamen.
`$MA` kann die kleingeschriebene MAC im Dateinamen ersetzen; im Gerätepfad bleibt
die tatsächliche MAC stehen. Trage nicht nur das Verzeichnis ein. Nutze für
diesen Bootstrap keine DHCP-TFTP-Standardwerte. Lasse Profile Rule B/C/D auf
einem ausschließlich von TalkOps verwalteten Gerät leer.
Die erzeugte Datei behält die ursprüngliche Profile Rule und enthält keine
Downloadreferenzen. Sie setzt das TalkOps-Telefonadministratorpasswort.
Halte dieses Passwort vor dem Resync bereit. Weitere Abrufe verwenden die
gespeicherte Profile Rule samt Authentifizierung.

SIP verwendet zugewiesenen Benutzernamen, unabhängiges Passwort, Anzeigenamen
und konfigurierten Registrar samt Port. Der aktuelle native TalkOps-Kontenkontext
verwendet UDP. Der Proxy enthält den Zielport. `SIP_Port` ist der lokale
Telefonport, nicht der Registrarport; das Profil lässt ihn unverändert.

NTP und Voicemail verwenden die gemeinsamen TalkOps-Einstellungen. Unterstützte
Zeitzonen sind UTC, Europe/London und die mitteleuropäischen Zonen Berlin,
Wien, Zürich, Paris, Rom, Madrid und Amsterdam einschließlich Sommerzeitregel.
Andere Zonen erzeugen einen ausdrücklichen Renderingfehler. Herstellerfelder
können die ausgegebenen Zeitwerte überschreiben.

Englisch ist integriert. Deutsch setzt `German` und `de-DE`. Die deutsche
Anzeige benötigt zusätzlich ein zur Firmware passendes Cisco-Wörterbuch.
Stelle das lizenzierte Wörterbuch separat bereit und setze
**Dictionary_Server_Script** in den Modelleinstellungen, zum Beispiel
`serv=https://locales.example/;d1=German;x1=ACTUAL_FILE.xml`.
TalkOps liefert weder Wörterbücher noch Firmware mit. Der externe Server hat
eigene Zugriffsregeln; die TalkOps-Geräteauthentifizierung gilt dort nicht.

### Gerätetest

Erfasse zuerst vollständige PID, Hardwareversion und Active Load.
Das vorhandene Gerät ist `CP-7861-K9= V09` mit Active Load
`sip78xx.14-2-1-0001-14` (Enterprise 14.2(1)). Diese Firmware kann das MPP-Profil
nicht verwenden. Nutze stattdessen den Enterprise-Test unten.
Ein Werksreset belegt keine MPP-Kompatibilität. Ändere für diesen Test keine Firmware.

Für ein Telefon mit bestätigter MPP-Firmware:

1. Erfasse PID, Firmware, Modell-ID und Testdatum.
2. Prüfe HTTP 401 für einen Abruf ohne Authentifizierung.
3. Starte einen Resync und prüfe HTTP 200 für die deklarierte Hauptdatei.
4. Prüfe HTTP 404 für einen fremden MAC-Dateinamen im Geräteverzeichnis.
5. Prüfe zwei unterschiedliche Kontenregistrierungen, sofern das Modell dies erlaubt.
6. Telefoniere über jedes zugewiesene Konto ein- und ausgehend.
7. Prüfe Sprache in beide Richtungen, Halten, Vermitteln und DTMF.
8. Prüfe Kontenbeschriftungen und deaktivierte freie Slots.
9. Prüfe Voicemail über die Nachrichtentaste und protokolliere das MWI-Ergebnis.
10. Prüfe NTP-Zeit, Sommerzeit und Anzeigesprache.
11. Starte das Telefon neu und prüfe authentifizierten Abruf sowie Registrierung erneut.
12. Speichere die Ergebnisse ohne Zugangsdaten und kennzeichne fehlgeschlagene Schritte.

BLF, eigene Tastenfunktionen, Kontakte und Fernbefehle gehören nicht zu diesem
Profiltest. Erfolgreiches XML-Rendering ist kein Hardwaretest.

Siehe [Cisco-Quellen und Implementierungsgrenzen](../research/cisco-7800.md).

## Cisco-7800-Telefone mit Enterprise-SIP (experimentell)

Die separaten Profile `cisco-7811-enterprise`, `cisco-7821-enterprise`,
`cisco-7841-enterprise` und `cisco-7861-enterprise` richten nicht abgesichertes
Enterprise-SIP ein. Erste Zielfirmware ist 14.2(1), einschließlich
`sip78xx.14-2-1-0001-14`. Ein `CP-7861-K9` mit diesem Load hat am 28.10.2026
die SEP-Datei und `dialplan.xml` geladen und sich per SIP-TCP registriert.
Anrufe, Audio, DTMF und Voicemail sind **noch nicht getestet**. 7811, 7821 und
7841 bleiben **am Gerät ungetestet**.
Dies sind TalkOps-Integrationsprofile, keine Cisco-Zertifizierung für FreeSWITCH.

Jedes Enterprise-Profil erlaubt **ein unabhängiges SIP-Konto auf Platz 1**.
Mehr physische Leitungstasten belegen keine Unterstützung unabhängiger
Digest-Zugangsdaten pro Leitung. TalkOps vereinheitlicht oder schwächt keine
Kontenpasswörter, um zusätzliche Leitungen einzurichten. Die MPP-Kontengrenzen
bleiben erhalten. Eigene Tastenbelegung, BLF, Kontakte, Firmwareupdates und
Sprachpaket-Downloads sind nicht enthalten. Die installierte Telefonsprache
bleibt erhalten. NTP und die unterstützten Zeitzonen UTC, Mitteleuropa und
London verwenden die gemeinsamen Einstellungen.

### Optionale TFTP-Bereitstellung aktivieren

Voraussetzungen sind ein Linux-Docker-Host und ein Server-Image aus diesem
Entwicklungsstand. Ältere veröffentlichte Images enthalten diese Funktion nicht.
TFTP ist unverschlüsselt und hat keine Authentifizierung. Aktiviere es nur in
einem vertrauenswürdigen Telefon-LAN. Die Quell-IP-Prüfung verhindert kein
Mitlesen oder IP-Spoofing durch Angreifer in diesem LAN. Die normale
authentifizierte HTTP-Provisionierung bleibt unverändert.

1. Reserviere eine feste IPv4-DHCP-Adresse für das Telefon.
2. Lege das genaue Modell **Enterprise SIP (experimental)** in TalkOps an.
3. Trage die Telefon-MAC ein und weise Nebenstelle und Konto 1 zu.
4. Setze den SIP-Host auf die vom Telefon erreichbare PBX-Adresse.
5. Öffne Provisionierung unter **Modelleinstellungen** und setze
   **TFTP-Client-IPv4-Adresse** auf die reservierte Telefonadresse.
   Jedes exportierte Telefon benötigt eine eindeutige Adresse.
6. Setze `TALKOPS_TFTP_BIND_IP` in der Deployment-`.env` auf die vertrauenswürdige
   LAN-Adresse des PBX-Hosts. Das ist die Serveradresse, nicht die Telefonadresse.
7. Starte die optionale Bereitstellung:

   ```sh
   docker compose -f docker-compose.yml -f docker-compose.cisco-tftp.yml up -d
   ```

8. Begrenze UDP 69 und UDP 30000–30031 über die Host-Firewall auf das Telefon-LAN.
9. Erlaube SIP-TCP auf dem konfigurierten Telefon-SIP-Port (Standard 5060) und
   den vorhandenen FreeSWITCH-Medienportbereich.

Der Dienst bildet Host-UDP 69 auf Container-UDP 1069 ab. Übertragungen verwenden
UDP 30000–30031. tftpd-hpa muss als root starten, um für jede Anfrage die
Rechte abzugeben. Der Container behält nur die Capabilities `SETUID`, `SETGID`
und `DAC_READ_SEARCH` und nutzt `no-new-privileges` sowie ein schreibgeschütztes
Dateisystem. Jede Übertragung läuft als UID 10001. Docker muss die echte LAN-Client-IP erhalten.
Ein Proxy oder NAT, das alle Clientadressen ersetzt, ist damit nicht vereinbar.

TalkOps exportiert ausschließlich die deklarierte SEP-Konfiguration und
`dialplan.xml` in ein privates flüchtiges tmpfs-Volume. Der TFTP-Dienst bindet es
schreibgeschützt ein. Dateien erhalten Modus 0600, Verzeichnisse Modus 0700.
Die Exporte werden alle 30 Sekunden erneuert. Doppelte Client-IPs, fehlende
SIP-Hosts und ungültige Profile werden ausgelassen. Entfernte Kontenzuordnungen
und Telefone verlieren ihren alten Export beim nächsten Lauf. Datenbankfehler
sperren den bisherigen Export. Ohne frischen Export stoppt der TFTP-Dienst
nach Ablauf des 90-Sekunden-Leases, auch nach einem TalkOps-Absturz. Die Prüfung
erfolgt jede Sekunde. Nach fünf Sekunden ohne Beendigung wird die gesamte
TFTP-Prozessgruppe zwangsweise beendet.
Das Volume liegt getrennt von persistenten Provisionierungs- und Backupdaten.
Gerätekopien übernehmen die Client-IP nicht.

### DHCP gemeinsam für Yealink und Cisco verwenden

Beide Gerätetypen können denselben DHCP-Server im Telefonnetz verwenden.
Stelle zwei unterschiedliche Optionen bereit:

| Client | DHCP-Option | Wert |
| --- | --- | --- |
| Vorhandene Yealink-Telefone | 66 | Bestehende HTTP(S)-URL mit `/provisioning` unverändert lassen |
| Cisco-Enterprise-Telefone | 150 | IPv4-Adresse des optionalen TFTP-Servers ohne URL-Pfad |

Cisco empfiehlt Option 150 für die 7800-Reihe. UniFi erlaubt zusätzliche
benutzerdefinierte DHCP-Optionen per Optionscode. Ergänze Option 150 in den
DHCP-Einstellungen des Telefonnetzes. Verwende den Typ IPv4-Adressliste, keine
ASCII-URL und keinen Hostnamen. Die genauen Feldnamen hängen von der
UniFi-Version ab. Behalte die DHCP-Suchreihenfolge der Yealinks bei; stelle
diese Telefone nicht auf Option 150 um.

Sobald der optionale TFTP-Dienst läuft, bleibt **Alternativer TFTP** am Cisco
deaktiviert, damit DHCP die Serveradresse liefert. Erneuere den Lease oder
starte das Cisco neu. Prüfe die angezeigte Adresse von TFTP-Server 1.
Prüfe zusätzlich einen Yealink-Resync auf unveränderte Funktion. Kann der
DHCP-Server keine getrennten Optionen liefern, nutze den manuellen Cisco-Eintrag
unten. Ersetze die gemeinsame Option-66-URL nicht durch eine reine TFTP-Adresse.

Die feste Telefon-IP bleibt für die Dateifreigabe erforderlich. DHCP-Servererkennung
und Freigabe der Dateien pro Telefon sind getrennte Prüfungen. Liegen Telefon
und PBX in unterschiedlichen VLANs, müssen Routing und Firewall die Verbindung
zulassen. TalkOps verändert weder DHCP-Server noch Gateway-Konfiguration.

### Bootstrap-Adresse am Telefon einstellen

1. Öffne **Anwendungen > Admin-Einstellungen > Netzwerkeinrichtung > IPv4-Einrichtung**.
2. Setze **Alternativer TFTP** auf **Ja**.
3. Setze **TFTP-Server 1** auf die LAN-IP aus `TALKOPS_TFTP_BIND_IP`.
4. Übernimm die Einstellungen und starte das Telefon manuell neu.

Die Menünamen hängen von der Firmware ab. Alternativ kann DHCP-Option 150 dieselbe
Server-IP liefern. Trage dort weder eine HTTP-URL noch TalkOps-Zugangsdaten ein.

Ein Telefon mit Beispiel-MAC `80:5E:C0:01:02:03` lädt
`SEP805EC0010203.cnf.xml` und danach `dialplan.xml`. Es verwendet SIP-TCP und die
eigenen TalkOps-Digest-Zugangsdaten. HTTP-Exporte und maskierte Vorschauen stehen
weiter über die normalen verwalteten Gerätepfade zur Verfügung. TFTP-Abrufe
erscheinen nicht in der HTTP-Provisionierungsanfrageliste. Prüfe den tatsächlichen
Gerätestand über Telefon-Statusmeldungen und SIP-Registrierung. Exportprotokolle
belegen ausschließlich die Dateierzeugung.

Der TFTP-Dienst verweigert Uploads, Pfadwechsel, fremde Telefondateien, generische
Fallback-Profile, Firmware sowie Signatur- und Vertrauensdateien. Fehlende
CTL-/ITL-Anfragen werden nicht mit erfundenen Dateien beantwortet. Vorhandene
CUCM-Vertrauenslisten können unsignierte Konfigurationen blockieren. Stoppe in
diesem Fall und verwende das vom Administrator freigegebene Cisco-Migrationsverfahren.
TalkOps löscht keine Vertrauenslisten und ersetzt keine Zertifikate. Abgesicherter
CUCM-Betrieb, CAPF, TLS-Zertifikate und Firmwarekonvertierung sind nicht implementiert.

### Vorhandenes Enterprise-Telefon testen

Vorhandenes Testmodell: `CP-7861-K9= V09` mit Enterprise 14.2(1).
Die Schritte 1 bis 5 waren am 28.10.2026 erfolgreich. Die übrigen Schritte sind offen.

Die Statusmeldung „Error Verifying Config Info" bedeutet, dass das Telefon den
Inhalt der SEP-Datei ablehnt. Releases bis 1.8.15 erzeugen eine solche Datei.
Verwende ein Release mit der korrigierten Enterprise-Vorlage. Die Vorlage schaltet
die schreibgeschützten Statusseiten des Telefons unter `http://<Telefon-IP>` ein.
Unter **Device Logs** steht dort die Ursache von Provisionierungs- oder
Registrierungsproblemen. SSH bleibt ausgeschaltet.

1. Erfasse PID, Hardwareversion und Active Load, ohne die MAC zu veröffentlichen.
2. Wähle `cisco-7861-enterprise` und richte die festen Adressen wie oben ein.
3. Prüfe die Vorschau auf maskierte Passwörter.
4. Prüfe den erfolgreichen TFTP-Abruf der genauen SEP-Datei und `dialplan.xml`.
5. Prüfe in TalkOps die SIP-TCP-Registrierung des zugewiesenen Kontos.
6. Telefoniere intern mit `**` ausgehend und nimm einen eingehenden Anruf an.
7. Prüfe Audio in beide Richtungen, DTMF, Halten und Vermitteln.
8. Prüfe Voicemailzugang, Uhrzeit und Leitungsbeschriftung.
9. Starte das Telefon neu und prüfe Abruf sowie Registrierung erneut.
10. Entferne die TFTP-Client-IP, warte einen Exportlauf ab und prüfe den verweigerten Abruf.
11. Protokolliere Erfolge und Fehler vor einer Änderung des Geräteprüfstatus.

Ändere bei diesem Test weder Firmware noch Vertrauenslisten. Registrierung,
Telefonie, MWI und Zusatzfunktionen bleiben bis zu diesem Gerätetest offen.
