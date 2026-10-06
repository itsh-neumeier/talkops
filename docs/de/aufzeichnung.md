# Gesprächsaufzeichnung und Transkription

TalkOps kann Gespräche aufzeichnen, Aufnahmen und Sprachnachrichten lokal in
Text umwandeln (Whisper) und diesen Text durchsuchen. Kein Audio verlässt den
Server.

> **Rechtliches:** In Deutschland ist das Aufzeichnen eines Gesprächs ohne
> Einwilligung aller Beteiligten strafbar (§ 201 StGB). Lassen Sie die
> Hinweisansage eingeschaltet, informieren Sie Ihre Mitarbeitenden und klären
> Sie den Einsatz im Unternehmen (Betriebsrat, Datenschutz) vorab.

## Einrichten

**Einstellungen → Gesprächsaufzeichnung und Transkription** (Admin):

- **Gespräche aufzeichnen** – getrennt für eingehende, ausgehende und interne
  Gespräche. Das ist die Vorgabe für alle Nebenstellen.
- **Aufzeichnung ansagen** – beide Gesprächspartner hören beim Annehmen
  „Dieses Gespräch wird aufgezeichnet.“ (Standard: an). Bei Warteschlangen
  hört der Anrufer die Ansage vor dem Warten.
- **Aufnahmen aufbewahren (Tage)** – ältere Aufnahmen werden samt Transkript
  automatisch gelöscht (stündlich geprüft). `0` = unbegrenzt. Standard: 90.
- **Aufnahmen und Sprachnachrichten transkribieren** – schaltet Whisper ein.

Je Nebenstelle (**Nebenstellen → Bearbeiten → Gesprächsaufzeichnung**):

| Einstellung | Wirkung |
|---|---|
| Vorgabe (Einstellungen) | es gilt die Einstellung je Richtung |
| Immer aufzeichnen | Gespräche dieser Nebenstelle werden immer aufgezeichnet |
| Nie aufzeichnen | nie – auch wenn die Gegenseite „immer“ hat (z. B. Betriebsrat, Arzt) |

Aufgezeichnet wird nur, was angenommen wurde. Klingelt ein Anruf ins Leere
und landet auf der Voicemail, entsteht keine Gesprächsaufnahme (die
Sprachnachricht wird wie gewohnt gespeichert). Bei Warteschlangen beginnt
die Aufnahme beim Eintritt in die Warteschlange, enthält also auch die
Wartezeit.

## Aufnahmen anhören

In der **Anrufliste** haben aufgezeichnete Gespräche die Schaltfläche
**Aufnahme**: Abspielen, Herunterladen (WAV, Stereo: links Anrufer, rechts
Angerufener) und – falls vorhanden – das Transkript mit Sprecher und Zeit.
Löschen können nur Admins.

Wer darf was?

- Benutzer: Aufnahmen und Transkripte von Gesprächen ihrer eigenen
  Nebenstellen.
- Admins: alle; das Abhören fremder Gespräche, das Lesen fremder Transkripte
  und das Löschen stehen im Audit-Log.
- Operatoren sehen zwar alle Anrufe in der Anrufliste, Aufnahmen aber nur wie
  Benutzer.

## Transkription

Bei eingeschalteter Transkription wandelt der Media-Worker jede neue
Aufnahme und jede neue Sprachnachricht in Text um:

- Zwei Gesprächsseiten werden getrennt erkannt, daher steht im Transkript,
  wer was gesagt hat (*Anrufer* / *Angerufener*).
- Sprache: die Sprache der Mailbox bzw. die Standardsprache aus den
  Einstellungen.
- Die Voicemail-Mail wartet auf das Transkript und enthält den Text
  (bei einem Fehler kommt sie ohne Text).
- Beim ersten Mal lädt der Worker das Sprachmodell (ca. 150 MB) aus dem
  offiziellen whisper.cpp-Repository und prüft seine Prüfsumme; danach
  arbeitet er ohne Internet.

Modell wählen (`.env`):

| `TALKOPS_WHISPER_MODEL` | Größe | Hinweis |
|---|---|---|
| `base` (Standard) | ca. 150 MB | schnell, gut für klare Sprache |
| `small` | ca. 490 MB | genauer, etwa dreimal langsamer |

Andere Modelle (z. B. `medium`) als `ggml-<name>.bin` selbst in das Volume
`models` legen und den Namen eintragen. Weitere Optionen:
`TALKOPS_WHISPER_THREADS` (CPU-Threads je Transkription, Standard bis 4).

## Suche

**Suche** findet Wörter in allen Transkripten, auf die Sie Zugriff haben –
Gespräche und Sprachnachrichten. Schreibweise wie bei Suchmaschinen:
`"genaue Phrase"`, `-ausschließen`, `rechnung or mahnung`. Treffer lassen
sich direkt abspielen.

## Speicherplatz

Eine Minute Gespräch belegt etwa 1,9 MB (8 kHz, 16 Bit, Stereo; mit
HD-Codecs mehr). Aufnahmen liegen im Volume `recordings`
(`<mandant>/<jahr-monat>/<anruf>.wav`) – denken Sie an die Datensicherung.
