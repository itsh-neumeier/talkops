# Laufende Gespräche

[Handbuch](index.md) · [English](../en/active-calls.md)

## Voraussetzungen

Du benötigst: calls.active für die Übersicht; Live-Transkript benötigt eigene Rechte.

Die erste Zeile zeigt Richtung und lokale Nebenstelle, die zweite den
Gesprächspartner. Namen werden mit allen Telefonbuchbereichen und den drei
Rufnummernfeldern abgeglichen. Nationale und internationale Schreibweisen
erkennen denselben Kontakt. Rufnummern stehen in eckigen Klammern:
`Familie Vollmeier (Maria) [+4995458589]`. Ein Klick auf einen Namen aus dem
Telefonbuch öffnet das Telefonbuch. Es zeigt den Bereich des Eintrags und
hebt den Eintrag hervor.

### Rufnummern in mehreren Telefonbucheinträgen

Eine Rufnummer kann in mehreren Telefonbuchbereichen stehen. Die Zeile zeigt
dann einen Namen und die Schaltfläche **+N weitere**. TalkOps wählt den Namen
in dieser Reihenfolge:

1. Ein Eintrag aus einem Telefonbuchbereich der beteiligten eigenen
   Nebenstelle. Das sind die Bereiche, die ihre provisionierten Telefone
   zeigen, und ein Bereich mit demselben Namen wie die Nebenstelle.
2. Der alphabetisch erste Eintrag.

Mit dem Mauszeiger auf dem Gesprächspartner oder mit **+N weitere** öffnet
sich eine Übersicht aller Einträge. Sie hat drei Spalten: **Telefonbuch**,
**Eintrag (Name)** und **Gehe zu Eintrag**. Die Markierung **angezeigt**
kennzeichnet den Eintrag aus der Zeile. Esc schließt die Übersicht.

Status und Dauer bleiben rechts. Die Gesprächsdauer beginnt bei Annahme;
vorher wird die Wartezeit angezeigt. Lange Namen umbrechen auf kleinen Geräten.

## Länderflaggen und Laden

Laufende und letzte Anrufe zeigen eine Flagge neben der eingehenden Quelle
oder dem ausgehenden Ziel. Sie kennzeichnet den Rufnummernbereich, nicht
den aktuellen Aufenthaltsort. Interne, anonyme und nicht zuordenbare Nummern
bekommen keine Flagge. Der Ländername erscheint als Tooltip und ist für
Screenreader verfügbar. Die letzten Anrufe im Softphone zeigen Flaggen bei
Rufnummern mit internationalem `+`- oder `00`-Präfix.

Die beiden Anrufbereiche der Übersicht laden unabhängig. Beim ersten Laden
erscheinen sofort Platzhalter für mindestens 300 ms. Danach wird der Inhalt
über 180 ms weich eingeblendet. Hintergrundabfragen lassen vorhandene Inhalte
sichtbar. Ein Leerzustand erscheint erst nach einer erfolgreichen leeren
Antwort. Bei Fehlern bleiben vorhandene Inhalte mit einem Fehlerhinweis
sichtbar. Bei reduzierter Bewegung entfallen Mindestanzeige und Animationen.

## Letzte Anrufe

Die Karte **Letzte Anrufe** im Dashboard zeigt beendete Anrufe im selben
Format wie die laufenden Gespräche. Namen, eckige Klammern und **+N weitere**
funktionieren gleich. Die erste Zeile zeigt außerdem die eigene Rufnummer:

- Eingehend: die Rufnummer, die der Anrufer gewählt hat, zum Beispiel
  `Eingehender Anruf an Nst: Familie Zweig [101] über +4995431241`.
- Ausgehend: die Rufnummer, die der Angerufene als Absender sieht.
- Intern: keine eigene Rufnummer.

Ältere Einträge ohne gespeicherte Rufnummer zeigen kein **über**. Rechts stehen
Zeitpunkt und Dauer. Verpasste eingehende Anrufe sind rot und zeigen
**Verpasst**. **Alle Anrufe** öffnet die [Anrufliste](anrufliste.md).

## Kurze Listen im Dashboard

Die Karten **Laufende Gespräche** und **Letzte Anrufe** zeigen drei Einträge.
Unter der Liste zeigt **N weitere anzeigen** alle Einträge. **Weniger
anzeigen** klappt die Liste wieder ein. **Letzte Anrufe** zeigt bis zu acht
Anrufe; **Alle Anrufe** öffnet die Anrufliste. Ein klingelnder Anruf bleibt
immer sichtbar, auch in einer eingeklappten Liste. Der Browser merkt sich für
jede Karte, ob sie aufgeklappt ist. In einem privaten Fenster starten die
Listen eingeklappt.

## Live-Transkript

**Live-Transkript** öffnet die Vorschau bei angenommenen, aufgezeichneten
Gesprächen mit aktivierter Transkription. Administratoren können alle Gespräche
ihres Mandanten lesen, Operatoren nur Gespräche eigener Nebenstellen.
Das Popup startet keine Aufnahme.

Standardmäßig nutzt der Worker das lokale Modell `base` mit höchstens zwei
Threads und lädt es beim ersten Bedarf herunter, falls es fehlt. Die Engine
der *Live-Vorschau* in den
[Transkriptions-Einstellungen](aufzeichnung.md#engines-je-zweck) kann das
lokale Modell *Schnell* oder ein Cloud-Modell wählen; ein Cloud-Modell kann
auch über die Realtime-API streamen, mit ca. einer Sekunde Verzögerung
([Live-Vorschau in der Cloud](aufzeichnung.md#live-vorschau-in-der-cloud)). Die Vorschau beginnt mit den
letzten Sekunden beim Öffnen. Neue Beiträge erscheinen mit Sprecher und
Uhrzeit der Sprache. Eine kurze Tippanimation zeigt neuen Text schrittweise;
bei reduzierten Animationen erscheint er sofort vollständig.

Die Erkennung ist vorläufig und lastabhängig zeitversetzt. Ohne Streaming
liest jeder Schritt höchstens acht Sekunden Audio. Pro Mandant sind vier Vorschauen
möglich. Das Popup behält die letzten 200 Textabschnitte. Das endgültige
Transkript bleibt für vollständige Texte und Suche zuständig.

### Live-Vorschau mit Coral

Mit einer Coral-Edge-TPU im Server kann die *Live-Vorschau*
**Coral (dieser Server)** mit Streaming nutzen
([Coral Edge TPU](aufzeichnung.md#coral-edge-tpu-dieser-server)). Der Text
erscheint während des Sprechens, Audio verlässt den Server nicht. Jede Zeile
zeigt die erkannte Sprache (`DE` oder `EN`). Ist Coral nicht verfügbar, zeigt
das Popup *Coral ist gerade nicht verfügbar; lokales Whisper erstellt die
Vorschau*, und das lokale Modell `base` arbeitet in 8-Sekunden-Stücken
weiter. Nach 30 Sekunden versucht TalkOps Coral erneut. Das ist kein Fehler.

Administratoren können die experimentelle Vorschau mit vollständigen Wörtern
über `TALKOPS_CORAL_STABLE_PREVIEW=true` in der Stack-Umgebung aktivieren.
Danach den Dienst `coral-stt` neu erstellen. Der Standardwert ist `false`.
Dieser Modus hält ein angefangenes Wort zurück, bis weiteres Audio es
abschließt oder eine Sprechpause den Abschnitt beendet. Modellkontext und
Pausenerkennung bleiben erhalten. Endgültige Transkripte von Aufnahmen und
Voicemails ändern sich dadurch nicht.

Beim getesteten deutschen Modell sind ungefähr 4–5 Sekunden Verzögerung
einzuplanen. Der erste Text kann länger dauern. Das ist eine modellabhängige
Schätzung und keine garantierte Obergrenze. Die Erkennungsqualität braucht
noch Verbesserungen. Widerspricht ein späteres Ergebnis einem bereits
angezeigten Wort, meldet der Stream einen Fehler und TalkOps nutzt den
vorhandenen Rückfall auf lokales Whisper. Zum Zurückstellen die Option auf
`false` setzen und den Dienst neu erstellen.

Bei einem Transfer zu einem anderen Gesprächspartner das Popup neu öffnen.
Die bisherige Vorschau stoppt, damit ihre Sprechernamen nicht dem neuen
Gesprächspartner zugeordnet werden.

Schließen beendet Anfragen und löscht temporären Text. Verlassene Vorschauen
stoppen neue Arbeit nach 30 Sekunden ohne Anfragen. Ihre Texte werden innerhalb
einer Stunde gelöscht. Endgültige Transkripte behalten die normale Löschfrist.

Falls kein Text erscheint, Aufnahme, Transkription, Worker, Modellzugriff und
freie Kapazität prüfen. Stille kann ohne Text bleiben. Server und Worker
gemeinsam aktualisieren und zuvor die Migrationen ausführen.

Siehe [Aufzeichnung](aufzeichnung.md) und
Implementierung ([ADR 0022](../adr/0022-active-call-identities-and-live-previews.md),
[ADR 0024](../adr/0024-transcription-engines-per-purpose.md),
[ADR 0046](../adr/0046-coral-edge-tpu-speech-recognition.md)).

## Ergebnis prüfen

Führe einen Testanruf und prüfe Live-Ansicht sowie abgeschlossenen Anrufdatensatz.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.
