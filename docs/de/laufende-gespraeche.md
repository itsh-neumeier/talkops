# Laufende Gespräche

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
[ADR 0024](../adr/0024-transcription-engines-per-purpose.md)).
