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

Enterprise-Firmware wird durch diese Integration **nicht unterstützt**.
Sie benötigt Cisco-Konfigurations- und Vertrauensdienste, üblicherweise über
TFTP oder CUCM-HTTP-Dienste. TalkOps bietet weder CUCM noch CTL-/ITL-Signierung
oder einen TFTP-Server. Ein bestehendes 8800-Template belegt keine Kompatibilität
mit der 7800-Enterprise-Firmware. Eine authentifizierte TalkOps-Datei-URL ersetzt
diese Dienste nicht. Es gibt kein auswählbares Enterprise-Profil.
Konvertierungen und Firmwareupdates sind nicht Bestandteil der Einrichtung.

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
nicht verwenden. Beende den MPP-Test bei der Firmwareprüfung.
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
