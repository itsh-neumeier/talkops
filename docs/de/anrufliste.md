# Anrufliste

[Handbuch](index.md) · [English](../en/call-log.md)

## Voraussetzungen

Du benötigst: Ein angemeldetes Konto; calls.view_all für fremde Anrufe.

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

## Karte

Wähle über der Liste den Reiter **Karte**. Die Karte zeigt, woher externe
Anrufe kommen und wohin sie gehen. Die Seitenadresse enthält Ansicht,
Zeitraum und Kategorien (zum Beispiel `/calls?view=map&range=1w`). Ein
kopierter Link öffnet dieselbe Karte.

- **Zeitraum**: 1 Stunde, 1 Tag, 1 Woche oder 1 Monat (30 Tage), jeweils
  bis jetzt. Bei 1 Stunde aktualisiert sich die Karte alle 30 Sekunden,
  solange der Browser-Tab sichtbar ist.
- **Kategorien**: eingehend angenommen (grün), eingehend verpasst (rot; auch
  besetzt oder abgelehnt), ausgehend (blau; das Ziel bestimmt den Ort) und
  blockiert / Spam (violett). Ein Klick blendet eine Kategorie aus oder ein.
  Die Zahl daneben ist die Anzahl im Zeitraum. Interne Anrufe erscheinen
  nicht auf der Karte.
- **Blasen**: eine Blase je Ortsnetz oder Ort. Die Fläche wächst mit der
  Zahl der Anrufe. Die Segmente zeigen die Kategorien. Mobilnummern,
  Servicenummern (zum Beispiel 0800, 0180) und Nummern ohne bekanntes
  Ortsnetz bilden je Land eine Blase „Mobil / ohne Ort“ (gestrichelter
  Rand).
- **Details**: Klicke auf eine Blase oder wähle sie mit der Tab-Taste und
  drücke Enter. Das Feld zeigt Ortsnetz, Vorwahl (zum Beispiel
  `Bamberg · +49 951`), Land, Summen je Kategorie, Gesamtdauer und die 20
  neuesten Anrufe. **Alle in der Anrufliste anzeigen** öffnet die Liste mit
  demselben Zeitraum und nur den Anrufen dieses Ortsnetzes. Esc schließt das
  Feld.
- **Ohne Standort**: Die Liste unter der Karte zeigt die Blasen ohne Ort je
  Land mit der Zahl der Mobil-, Service- und sonstigen Nummern. Anrufe ohne
  Rufnummer (anonym) werden nur gezählt.

Die Karte nutzt dieselben Rechte wie die Liste: Benutzer sehen die Anrufe
ihrer Nebenstellen, Benutzer mit `calls.view_all` alle Anrufe. Die Karte
zählt höchstens 50.000 Anrufe. Bei mehr Anrufen zeigt ein Hinweis, dass nur
die neuesten gezählt sind.

### Zoom und Orientierung

Vergrößere mit der Schaltfläche **+**, dem Mausrad oder der Taste **+**. Die
Offline-Karte zeigt je Stufe mehr Details:

| Zoomstufe | Inhalt |
| --- | --- |
| bis 4 | Länder |
| 5 | Grenzen der Bundesländer, Kantone und Provinzen, große Flüsse und Seen, Städte ab 500.000 Einwohnern |
| 7 | Alle Flüsse und Seen der Kartendaten, Orte ab 50.000 Einwohnern |
| 8 | Ortsnetzbereiche in Deutschland (dünne Linien), Orte ab 10.000 Einwohnern |
| 10 bis 11 | Orte ab 1.000 Einwohnern in Deutschland, Österreich und der Schweiz |

Überlappende Ortsnamen werden ausgeblendet; größere Orte haben Vorrang. Eine
gewählte deutsche Blase zeigt die Fläche ihres Ortsnetzbereichs. Die
Offline-Karte hat keine Straßen. Für Straßendetails kann ein Administrator
einen Tile-Server eintragen
([Systemeinstellungen](systemeinstellungen.md#tile-server-der-anrufkarte));
Ortsnetzbereiche und Blasen liegen dann über den Kacheln.

### Dashboard

Die Dashboard-Karte **Herkunft der Anrufe** zeigt eine kleine Karte des
letzten Tages (oder der letzten Stunde) und die fünf Ortsnetze mit den
meisten Anrufen. Sie nutzt dieselben Rechte. Die Karte zoomt auf die Blasen
und zeigt dieselben Details wie die Kartenansicht (Bundesländer, Gewässer,
Orte, Ortsnetzbereiche). Zoome mit den Schaltflächen **+** und **−**; das
Mausrad scrollt die Seite. Ein Klick auf eine Blase, ein Ortsnetz oder
**Karte öffnen** öffnet die Kartenansicht der Anrufliste mit demselben
Zeitraum.

### Genauigkeit

Die Karte zeigt das Ortsnetz der Rufnummer, nicht den Standort des
Anrufenden:

- Deutschland, Österreich und Schweiz: das Ortsnetz der Vorwahl.
- Andere Länder: der Ort aus den libphonenumber-Daten, wenn er bekannt ist,
  sonst das Land. Für manche Länder (zum Beispiel Frankreich) gibt es keine
  Ortsdaten.
- Eine portierte Nummer behält ihre Vorwahl. Eine VoIP-Nummer mit
  Ortsvorwahl kann an einem anderen Ort genutzt werden. Eine Mobilnummer
  hat keinen Ort.

### Datenquellen und Datenschutz

Die Karte funktioniert offline. Basiskarte (Länder, Bundesländer, Gewässer,
Orte, Ortsnetzbereiche) und alle Ortsnetzdaten sind in TalkOps enthalten. Die Seite sendet keine Anfrage an
fremde Server. Die Karte nennt diese Quellen:

- © Bundesnetzagentur (Ortsnetzbereiche Deutschland)
- Quelle: RTR-GmbH (Ortsnetze Österreich)
- GeoNames, CC BY 4.0 (Koordinaten und Namen der Orte)
- libphonenumber, Apache License 2.0 (Rufnummernpräfixe)
- Natural Earth, gemeinfrei (Länder, Bundesländer, Flüsse, Seen)

Die Daten lassen sich mit `tools/geo/` aktualisieren (siehe ADR 0044).
Ein Administrator kann zusätzlich einen Tile-Server für eine detaillierte
Hintergrundkarte eintragen
([Systemeinstellungen](systemeinstellungen.md#tile-server-der-anrufkarte)).
Der Browser lädt die Kartenkacheln dann von diesem Server.

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
Mit `area` (die Kennung einer Kartenblase, zum Beispiel `DE-951`, `DE-none`
oder `none`) enthält die Liste nur die Anrufe dieses Kartenbereichs.

`GET /api/v1/calls/map?range=1h|1d|1w|1m&categories=answered,missed,outbound,blocked`
liefert die Kartendaten: je Bereich Position, Name, Vorwahl, Zähler je
Kategorie, Gesamtdauer und die 20 neuesten Anrufe, dazu die anonymen Zähler
und die Datenquellen.

## Ergebnis prüfen

Vergleiche einen Testanruf mit Zeitpunkt, Richtung, Ergebnis und Filtern.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.
