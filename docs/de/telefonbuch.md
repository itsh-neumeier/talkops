# Telefonbuch

[Handbuch](index.md) · [English](../en/phonebook.md)

Das Telefonbuch enthält gemeinsame Kontakte. Das interne Verzeichnis stammt
aus den Nebenstellen. Ein Telefonbuchbereich wählt Kontakte für bestimmte
provisionierte Telefone aus. Er ist weder ein privates Benutzeradressbuch noch
eine Zugriffsgrenze.

## Voraussetzungen

Melde dich an, um das gemeinsame Telefonbuch zu lesen. Bearbeiten, Löschen,
Bereichsverwaltung und CSV-Import benötigen `phonebook.edit`.
Telefonzuweisungen benötigen `phones.manage`.
Siehe [Benutzer und Rollen](benutzer-und-rollen.md).

## Kontakt anlegen und finden

1. Öffne **Telefonbuch → Neuer Kontakt**.
2. Trage Name, optional Firma und geschäftliche, mobile oder andere Nummer ein.
3. Wähle einen Bereich oder lasse **Global (alle Telefone)** stehen.
4. Speichere und suche nach Name oder Nummer.

Nutze möglichst internationale Nummern wie `+49891234567`. Ein interner Kontakt
kann mit `**101` ausdrücklich intern gewählt werden. Die Suche erfasst auch
Firma und Bereich. Spaltenfilter wählen Kontakte mit oder ohne den jeweiligen
Nummerntyp. Sortiere nach Name, Firma oder Bereich. Setze Filter zurück, wenn
ein gespeicherter Eintrag verborgen bleibt. Beim Löschen eines Bereichs werden
auch seine Kontakte gelöscht; prüfe die Bestätigung.

## CSV-Datei importieren

1. Öffne **Telefonbuch → Importieren** und lade die Beispieldatei herunter.
2. Exportiere Kontakte mit Kopfzeile und Komma, Semikolon oder Tab als Trennzeichen.
3. Wähle die Datei und prüfe die Vorschau.
4. Wähle den Bereich für Zeilen ohne Bereich.
5. Aktiviere **Bestehende Kontakte aktualisieren** nur bei beabsichtigten Änderungen.
6. Prüfe neue, aktualisierte, übersprungene und fehlerhafte Zeilen und wähle **Importieren**.

Die Vorschau speichert noch keine Kontakte. Erkannt werden Name oder Vor-/Nachname,
Firma, Geschäftlich, Mobil, Andere/Privat und Bereich, einschließlich üblicher
Outlook- und Google-Exporte. Nutze UTF-8; der Browser akzeptiert auch Windows-1252.
Gleicher Name im gleichen Bereich erkennt einen bestehenden Kontakt. Neue
Bereichsnamen in der Datei legen Bereiche an. Fehlerhafte Zeilen werden mit
Zeilennummer gemeldet und übersprungen. Korrigiere sie vor einem neuen Import.

Erwartetes Ergebnis: Die importierten Namen erscheinen im gewählten Bereich.
Prüfe bei Aktualisierung zunächst eine kleine Beispieldatei.

## Geräteverzeichnisse, Anrufernamen und Favoriten

Globale Kontakte und internes Verzeichnis stehen unterstützten provisionierten
Telefonen zur Verfügung. Zusätzliche Bereiche weist du auf der Telefon-Detailseite
zu. Die Oberfläche unterstützt bei Yealink bis zu drei zusätzliche Bereiche.
Formate und Unterstützung hängen vom Modell ab; siehe
[Yealink-Telefonbücher](yealink.md#5-telefonbuch) und
[Herstellereinstellungen](hersteller-provisionierung.md).

Anrufansichten können Nummern zu Kontakten auflösen und den passenden Eintrag
verlinken. Das [Softphone](softphone.md) bietet Kontakte und Favoriten zur Wahl.
Favoriten sind kein eigener Telefonbuchbereich.

Prüfe einen eindeutigen Kontakt zuerst im Browser und aktualisiere dann das
Verzeichnis auf einem unterstützten konfigurierten Telefon. Fehlt er, prüfe
Bereichszuweisung, Verzeichnisadresse, Provisionierungszugang und Modellunterstützung.
Die Bereichswahl macht gemeinsame Kontakte in der Weboberfläche nicht vertraulich.
