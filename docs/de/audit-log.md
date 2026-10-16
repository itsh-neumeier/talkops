# Änderungsprotokoll (Audit-Log)

TalkOps schreibt für jede administrative Änderung einen Audit-Eintrag mit
Zeit, Benutzer, IP-Adresse, Aktion, Objekt und Details. Die Details enthalten
nie Passwörter oder Schlüssel.

**Einstellungen → Änderungsprotokoll** öffnen. Dafür ist die Berechtigung
`audit.view` nötig.

## Filtern und sortieren

- **Von** und **Bis**: Tage in lokaler Zeit. **Bis** schließt den ganzen Tag
  ein.
- **Benutzer**, **Aktion** und **Objekt**: Listen der Werte im Protokoll.
- **Suche**: Text in der Objekt-ID oder in den Details, ohne Unterschied
  zwischen Groß- und Kleinschreibung.

Ein Klick auf eine Spaltenüberschrift sortiert nach Zeit, Benutzer, Aktion
oder Objekt; ein zweiter Klick dreht die Richtung um. Die Liste zeigt 50
Einträge je Seite und die Anzahl aller passenden Einträge.

## Herunterladen

**CSV** und **JSON** laden alle Einträge, die zum Filter passen, in der
gewählten Sortierung und ohne Seiten herunter. Ein Download enthält höchstens
100.000 Einträge; passen mehr, den Filter enger fassen. Jeder Download
schreibt einen Audit-Eintrag mit Format, Anzahl und Filter.

- CSV: RFC 4180 mit Kopfzeile und UTF-8. Die Spalte `details` enthält JSON.
  Eine Zelle, die mit `=`, `+`, `-` oder `@` beginnt, erhält ein führendes
  `'`, damit eine Tabellenkalkulation sie nicht als Formel ausführt.
- JSON: ein Array der Einträge.

## Aufbewahrung und automatischer Export

Der Reiter **Aufbewahrung und Export** braucht die Berechtigung
`audit.manage`. Zum Öffnen der Seite ist zusätzlich `audit.view` nötig.

**Aufbewahrung in der Datenbank**: TalkOps löscht stündlich Einträge, die
älter als die eingestellte Anzahl Tage sind. `0` behält alle Einträge
(Standard). Bei aktivem automatischem Export löscht TalkOps nur Einträge, die
bereits in einer Exportdatei stehen.

Der **automatische Export** schreibt neue Einträge auf ein
[Speicherziel](datensicherung.md#sicherungsziele) (SMB, S3, WebDAV, SFTP, FTP
oder lokaler Ordner):

1. Unter **Einstellungen → Speicher** ein Speicherziel anlegen.
2. Im Reiter **Aufbewahrung und Export** Speicherziel, Unterordner (Standard
   `audit`) und Format wählen.
3. Zeitplan und Anzahl der aufzubewahrenden Exportdateien festlegen und
   **Speichern** klicken.

Jeder Lauf schreibt alle Einträge nach dem zuletzt exportierten Eintrag in
eine neue Datei:

- `talkops-audit-JJJJMMTT-HHMMSS-ERSTE-LETZTE.jsonl` (JSON Lines: ein Eintrag
  je Zeile) oder `.csv`. `ERSTE` und `LETZTE` sind die Eintrags-IDs der Datei.
- daneben `<datei>.sha256` im Format von `sha256sum`. TalkOps zeigt die
  Prüfsumme auch in der Liste der Exportdateien.

Ein Lauf ohne neue Einträge schreibt keine Datei. Eine Datei enthält höchstens
50.000 Einträge; der nächste Lauf setzt fort. **Jetzt exportieren** startet
sofort einen Lauf. Der Status zeigt letzten Lauf, letzten Fehler und nächsten
Lauf.

Datei auf dem Speicherziel prüfen:

```sh
sha256sum -c talkops-audit-20261016-021500-1-120.jsonl.sha256
```

Die Aufbewahrung der Dateien folgt den Regeln der Sicherungsaufträge (siehe
[Intelligente Aufbewahrung](datensicherung.md#intelligente-aufbewahrung)). Sie
löscht nur TalkOps-Exportdateien und ihre Prüfsummendateien im Ordner.

Ein vom Export genutztes Speicherziel lässt sich nicht löschen. Entfernte
Speicherziele brauchen für den Audit-Export kein Backup-Passwort. TalkOps
verschlüsselt die Dateien nicht; ein Speicherziel mit verschlüsselter
Übertragung verwenden (SFTP, FTPS, HTTPS, S3 mit TLS oder SMB 3).

Die API steht in der [englischen Anleitung](../en/audit-log.md#api).
