# Laufende Gespräche

Die erste Zeile zeigt Richtung und lokale Nebenstelle, die zweite den
Gesprächspartner. Namen werden mit allen Telefonbuchbereichen und den drei
Rufnummernfeldern abgeglichen. Nationale und internationale Schreibweisen
erkennen denselben Kontakt. Rufnummern stehen in eckigen Klammern:
`Familie Vollmeier (Maria) [+4995458589]`. Mehrdeutige Treffer erhalten einen
Hinweis statt eines willkürlich gewählten Namens.

Status und Dauer bleiben rechts. Die Gesprächsdauer beginnt bei Annahme;
vorher wird die Wartezeit angezeigt. Lange Namen umbrechen auf kleinen Geräten.

## Live-Transkript

**Live-Transkript** öffnet die Vorschau bei angenommenen, aufgezeichneten
Gesprächen mit aktivierter Transkription. Administratoren können alle Gespräche
ihres Mandanten lesen, Operatoren nur Gespräche eigener Nebenstellen.
Das Popup startet keine Aufnahme.

Der Worker nutzt das lokale Modell `base` mit höchstens zwei Threads und lädt
es beim ersten Bedarf herunter, falls es fehlt. Die Vorschau beginnt mit den
letzten Sekunden beim Öffnen. Neue Beiträge erscheinen mit Sprecher und
Uhrzeit der Sprache. Eine kurze Tippanimation zeigt neuen Text schrittweise;
bei reduzierten Animationen erscheint er sofort vollständig.

Die Erkennung ist vorläufig und lastabhängig zeitversetzt. Pro Schritt werden
höchstens acht Sekunden Audio gelesen. Pro Mandant sind vier Vorschauen
möglich. Das Popup behält die letzten 200 Textabschnitte. Das endgültige
Transkript bleibt für vollständige Texte und Suche zuständig.

Schließen beendet Anfragen und löscht temporären Text. Verlassene Vorschauen
stoppen neue Arbeit nach 30 Sekunden ohne Anfragen. Ihre Texte werden innerhalb
einer Stunde gelöscht. Endgültige Transkripte behalten die normale Löschfrist.

Falls kein Text erscheint, Aufnahme, Transkription, Worker, Modellzugriff und
freie Kapazität prüfen. Stille kann ohne Text bleiben. Server und Worker
gemeinsam aktualisieren und zuvor die Migrationen ausführen.

Siehe [Aufzeichnung](aufzeichnung.md) und
[Implementierung](../adr/0022-active-call-identities-and-live-previews.md).
