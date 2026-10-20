# Trunks und Rufnummern

[Handbuch](index.md) · [English](../en/trunks-and-numbers.md)

Ein Trunk verbindet TalkOps mit einem SIP-Provider. Ein Konto enthält die
Registrierungsdaten. Eine Rufnummer gehört zu einem Konto und wählt ein
eingehendes Ziel. Diese Objekte sind von Benutzern und internen Nebenstellen
getrennt.

## Voraussetzungen

Nutze einen Administrator oder eine Rolle mit `trunks.view` und `trunks.manage`.
Telefonievorgaben benötigen zusätzlich `settings.manage`. Du brauchst SIP-Server,
Benutzername, gegebenenfalls abweichenden Authentifizierungsnamen, Passwort und
zugewiesene Rufnummern. Die SIP-Zugangsdaten können vom Provider-Portal abweichen.
Für diesen Anbieter gibt es die Anleitung [LEONET](leonet.md).

## Provider und Konto hinzufügen

1. Öffne **Einstellungen → Trunks → Neuer Trunk**.
2. Wähle die Provider-Vorlage und lies Hinweise und Prüfstatus.
3. Wähle bei unbekanntem Provider die generische SIP-Vorlage und trage den Registrar ein.
4. Speichere den Trunk und öffne seine Detailseite.
5. Wähle **Zugang hinzufügen** und trage die Provider-Zugangsdaten ein.
6. Speichere und prüfe den Registrierungsstatus.

Erwartetes Ergebnis: Ein Konto mit Registrierung zeigt **registriert**.
Ein Provider mit IP-Authentifizierung kann anders arbeiten; beachte die
Vorlagenhinweise. Prüfe bei Fehlern Registrar, Transport und
Authentifizierungsname, bevor du erweiterte Einstellungen änderst.
Veröffentliche keine Zugangsdaten in Supportanfragen.

## Rufnummern und eingehende Ziele hinzufügen

1. Ergänze beim richtigen Konto eine Rufnummer im internationalen Format, etwa `+49891234567`.
2. Öffne **Einstellungen → Rufnummern** oder die Rufnummer auf der Trunk-Detailseite.
3. Wähle das eingehende Ziel und speichere.
4. Rufe die Nummer von einem separaten externen Telefon an.

Der Zielwähler bietet je nach Einrichtung Nebenstelle, Gruppe, Zeitsteuerung,
Sprachmenü, Warteschlange, Voicemail oder konfiguriertes Faxziel.
Bei einer Nebenstelle ergänzt **Weitere Nebenstellen klingeln lassen** parallele
Ziele. Die Hauptnebenstelle bestimmt die Behandlung unbeantworteter Anrufe.
Siehe [Anrufsteuerung](anrufsteuerung.md). Ein Faxziel ersetzt die Sprachroute
dieser Nummer.

Erwartetes Ergebnis: Das gewählte Ziel erhält den Anruf. Nach Gesprächsende
erscheint er in der berechtigten [Anrufliste](anrufliste.md).
Eine Registrierung allein prüft weder Rufnummernzuordnung noch beidseitigen Ton.

## Ausgehende Anrufe und Notrufrouting

1. Öffne **Einstellungen → Telefonie** und setze Landes- und Ortsvorwahl.
2. Wähle eine Standardrufnummer und speichere.
3. Weise einer Nebenstelle bei Bedarf eine andere ausgehende Nummer zu.
4. Prüfe mit einem normalen externen Anruf Rufnummernanzeige und beidseitigen Ton.

Notrufe verwenden die Standardrufnummer und unterdrücken die Rufnummer nicht.
Kläre Notrufunterstützung und Adresszuordnung mit dem Provider.
Rufe keine Notrufnummer als gewöhnlichen Installationstest an.
Wahlformate und Rufnummernunterdrückung stehen unter [Erste Schritte](erste-schritte.md).

## SIP-Konten ohne Rufnummer

Ein Konto kann nur eine SIP-Adresse haben. Wähle **Eingehende Anrufe an dieses
Konto**, wenn Anrufe ohne Rufnummernzuordnung ein Ziel erreichen sollen.
Die Wahl mit `name@domain` erklären die [ersten Schritte](erste-schritte.md).

## Fehlerbehebung

- Nicht registriert: Zugangsdaten, Registrar, Transport und Erreichbarkeit prüfen.
- Registriert, aber kein Klingeln: Konto, Rufnummer und Ziel prüfen.
- Abgewiesener Anruf: gespeicherten Sperrgrund und Routingregeln prüfen.
- Kein externer Anruf: Standard-/Ausgangsnummer und Provider-Freigaben prüfen.
- Einseitiger Ton: Netzwerkprüfungen unter [Fehlerbehebung](fehlerbehebung.md) nutzen.

Die [Provider-Vorlagenanleitung](../trunk-presets.md) erklärt Quellen und die
Einreichung einer geprüften Vorlage.
