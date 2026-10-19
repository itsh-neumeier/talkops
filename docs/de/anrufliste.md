# Anrufliste

## Blockierte Anrufe und Whitelist

Anrufliste und letzte Anrufe zeigen **Blockiert** mit der Quelle:
**Sperrliste**, **PhoneBlock** oder **Anonym**. Unter **Blockierungsdetails**
stehen die passende Regel oder die Zahl der Meldungen und der technische
Auflegegrund. Als Ziel erscheint die gewählte Rufnummer. Ein blockierter
Anruf hat kein Telefon zum Klingeln gebracht.

Mit **Whitelist verwalten** (`blocking.allowlist`) kann ein Benutzer bei
einer bekannten internationalen Anrufernummer **Zur Whitelist hinzufügen**
wählen. Der Dialog zeigt die exakte Nummer und ein optionales Notizfeld.
**Nummer freigeben** speichert die Ausnahme für den gesamten Mandanten.
Anonyme Anrufer können nicht auf die Whitelist gesetzt werden.

Die Freigabe gilt sofort für künftige Anrufe und übersteuert eigene
Sperrlisten einschließlich Vorwahlen sowie PhoneBlock. Andere Routingregeln
gelten weiterhin. Frühere Anrufe behalten den ursprünglichen
Blockierungsgrund; **Auf Whitelist** zeigt die aktuelle Freigabe separat.
Die Sichtbarkeit fremder Anrufe wird dadurch nicht erweitert.

Der Ergebnisfilter **Blockiert** enthält bekannte blockierte Anrufe.
Diese zählen nicht zusätzlich als verpasst oder besetzt. Auch die
Verpasst-Zahl und Annahmequote im Dashboard schließen bekannte blockierte
Anrufe aus; Gesamtzahl und eingehende Anrufe enthalten sie weiterhin.

Ältere Datensätze ohne gespeicherten Grund behalten ihr bisheriges Ergebnis.
TalkOps leitet den Grund nicht aus einer allgemeinen Ablehnung oder dem
heutigen Stand der Sperrlisten ab.

Die **Anrufliste** zeigt die beendeten Anrufe. Admins und Operatoren sehen
alle Anrufe, Benutzer die Anrufe ihrer eigenen Nebenstellen.

## Spalten

| Spalte | Inhalt |
| --- | --- |
| Zeit | Beginn des Anrufs. |
| Richtung | Eingehend, ausgehend oder intern. |
| Von | Der Anrufer: eine Nebenstelle mit Namen oder eine externe Rufnummer. |
| An | Der Angerufene. Bei angenommenen eingehenden Anrufen: die annehmende Nebenstelle, die gewählte eigene Rufnummer und der Name der Nebenstelle, z. B. `Nst. 101 über +4995431241` und `Familie Zweig`. |
| Dauer | Gesprächsdauer eines angenommenen Anrufs. |
| Ergebnis | `OK`, `besetzt/abgelehnt` oder `nicht angenommen`. |
| Aufnahme | Wiedergabe-Schaltfläche bei aufgezeichneten Anrufen. |

## Telefonbuchnamen

Die Checkbox **Rufnummern über Telefonbuch anzeigen** ist standardmäßig an.

- An: Eine externe Rufnummer mit Telefonbucheintrag zeigt zuerst den Namen
  des Eintrags und darunter die Nummer. Ein Klick auf den Namen öffnet den
  Eintrag. Haben mehrere Einträge die Nummer, liefert das Telefonbuch der
  eigenen Nebenstelle den Namen (dieselbe Regel wie bei den
  [laufenden Gesprächen](laufende-gespraeche.md)).
- Aus: Die Liste zeigt zuerst die Rufnummer und darunter den Anrufernamen
  aus dem Gesprächsdatensatz.

Der Browser merkt sich die Einstellung.

## Länderflaggen und Laden

Bei eingehenden Anrufen steht die Flagge neben der externen Quelle, bei
ausgehenden Anrufen neben dem externen Ziel. Der Ländername erscheint als
Tooltip und ist für Screenreader verfügbar. Die Flagge bezeichnet den
Rufnummernbereich, nicht den Aufenthaltsort. Nebenstellen, anonyme Anrufer,
Servicenummern und nicht eindeutig zugeordnete Länder erhalten keine Flagge.
Die Flagge bleibt auch bei ausgeschalteten Telefonbuchnamen sichtbar.

Beim ersten Laden erscheinen sofort Platzhalter für mindestens 300 ms.
Danach wird der Inhalt über 180 ms weich eingeblendet. Beim Filtern bleiben
vorhandene Zeilen mit einem Ladehinweis sichtbar. **Weitere laden** zeigt
zusätzliche Platzhalterzeilen bis zur Antwort. Bei reduzierter Bewegung
entfallen Mindestanzeige und Animationen.

## Sortieren

Ein Klick auf die Spaltenüberschrift **Zeit** oder **Dauer** sortiert nach
dieser Spalte. Ein zweiter Klick kehrt die Reihenfolge um (absteigend,
aufsteigend). Der Pfeil zeigt die aktuelle Reihenfolge. Der Server sortiert;
die Reihenfolge gilt also für alle Anrufe, nicht nur für die geladenen Zeilen.

## Filtern

Filterbare Spalten haben eine Trichter-Schaltfläche. Ein Klick öffnet das
Filterfenster der Spalte. Ein farbiger Punkt zeigt einen aktiven Filter.

| Spalte | Filter |
| --- | --- |
| Zeit | Heute, 7 Tage, 30 Tage oder ein Datumsbereich (beide Tage eingeschlossen). |
| Richtung | Eine oder mehrere Richtungen. |
| Von | Text in Nummer, Name, Nebenstelle oder Telefonbuchname. |
| An | Text in Nummer, annehmender Nebenstelle, gewählter eigener Rufnummer oder Telefonbuchname. |
| Ergebnis | Angenommen, nicht angenommen, besetzt/abgelehnt. |
| Aufnahme | Alle, mit Aufnahme, mit Transkript, ohne Aufnahme. |

Textfilter starten nach einer kurzen Tipppause. **Zurücksetzen** löscht den
Filter einer Spalte. **Alle Filter zurücksetzen** löscht alle Filter und die
Suche; die Sortierung bleibt.

Die Seitenadresse enthält Filter und Sortierung. Neu laden oder ein kopierter
Link zeigt also dieselbe Liste.

## Weitere laden

Die Liste lädt jeweils 50 Anrufe. Die Zeile unter der Tabelle zeigt die Zahl
der geladenen und aller passenden Anrufe. **Weitere laden** lädt die
nächsten Anrufe.

## API

`GET /api/v1/calls` versteht diese Parameter: `since`, `until` (Zeitpunkte),
`direction`, `result` (kommagetrennte Listen), `from`, `to`, `recording`
(`with`, `transcript`, `without`), `search`, `sort` (`time`, `duration`),
`order` (`asc`, `desc`), `offset` und `limit` (höchstens 500). Der Header
`X-Total-Count` enthält die Zahl aller passenden Anrufe. Jeder Anruf enthält
zusätzlich die aufgelösten Teilnehmer `caller` und `called` und bei
eingehenden Anrufen `via_number`.
