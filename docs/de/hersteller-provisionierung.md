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
