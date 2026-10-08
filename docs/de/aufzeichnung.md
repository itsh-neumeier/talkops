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
- Beim ersten Mal lädt der Worker das Sprachmodell (ca. 550 MB) und das
  Modell der Spracherkennung (Silero VAD, ca. 1 MB) aus den offiziellen
  whisper.cpp-Repositories und prüft die Prüfsummen; danach arbeitet er
  ohne Internet.
- Die Spracherkennung (VAD) schneidet Stille und Wartemusik heraus, bevor
  Whisper transkribiert. Das verhindert erfundene Sätze in Pausen und
  macht die Erkennung schneller. Abschalten mit `TALKOPS_WHISPER_VAD=false`.

**Genauigkeit** (Einstellungen → Anrufbehandlung): *Schnell* nutzt das Modell
aus `TALKOPS_WHISPER_MODEL`. *Genau* (`large-v3-q5_0`, 1,1 GB, ca. 2 GB RAM)
und *Beste* (`large-v3`, 3,1 GB, ca. 4 GB RAM) nutzen das volle large-v3 mit
breiterer Suche: deutlich weniger falsche Wörter, aber zwei- bis viermal
langsamer. **Namen und Begriffe** (durch Komma getrennt) helfen bei Namen,
Firmen und Fachwörtern. Zum Vergleichen im Container:
`talkops-media-worker transcribe --quality accurate --vocabulary "Name, Firma" datei.wav`.

Modell wählen (`.env`):

| `TALKOPS_WHISPER_MODEL` | Größe | Hinweis |
|---|---|---|
| `large-v3-turbo-q5_0` (Standard) | ca. 550 MB | sehr genau, auch bei Telefonqualität; ca. 2 GB RAM |
| `large-v3-turbo-q8_0` | ca. 870 MB | minimal genauer |
| `large-v3-turbo` | ca. 1,6 GB | volle Genauigkeit, braucht viel RAM |
| `medium-q5_0` / `medium` | ca. 540 MB / 1,5 GB | Alternative zu turbo |
| `small` | ca. 490 MB | für schwache Hardware (z. B. Raspberry Pi) |
| `base` | ca. 150 MB | schnell, aber deutlich ungenauer (Standard bis 1.2) |

Auf einem aktuellen x86-Server mit 4 Kernen braucht `large-v3-turbo-q5_0`
etwa ein Drittel bis die Hälfte der Gesprächsdauer. Wer `base` explizit
gesetzt hat, sollte die Zeile in der `.env` entfernen oder ändern.
Andere Modelle als `ggml-<name>.bin` selbst in das Volume `models` legen und
den Namen eintragen. Weitere Optionen: `TALKOPS_WHISPER_THREADS`
(CPU-Threads je Transkription, Standard bis 4).

### Zwei Durchgänge: erst schnell, dann genau

Unter *Einstellungen → Anrufbehandlung* lassen sich zwei Durchgänge wählen:

- **Erste Transkription (sofort)** läuft direkt nach dem Gespräch. Ihr Text
  erscheint zuerst und geht mit der Voicemail-Mail raus – am besten
  *Schnell*.
- **Zweite, genauere Transkription (danach)** läuft im Anschluss mit
  niedrigerer Priorität (neue erste Durchgänge gehen vor) und ersetzt das
  erste Transkript, sobald sie fertig ist, z. B. *Beste* (large-v3) oder die
  KI-API. Bis dahin steht am Transkript *Vorläufig – genauere Fassung
  folgt*; die Ansicht aktualisiert sich von selbst. Schlägt der zweite
  Durchgang fehl (ein Wiederholungsversuch), bleibt das erste Transkript.

Am Transkript steht, welches Modell es erstellt hat (z. B.
`whisper:large-v3-turbo-q5_0` oder `api:whisper-1`).

### KI-API (OpenAI, Groq, Mistral, eigener Server)

Statt des lokalen Whisper kann jeder Durchgang eine OpenAI-kompatible
Transkriptions-API nutzen (`POST …/audio/transcriptions`). Auswahl
*KI-API*, dann Anbieter, URL, Modell und Schlüssel eintragen und *Verbindung
testen* (listet die Sprachmodelle des Servers, ohne Audio zu senden):

| Anbieter | URL | Modelle (Beispiele) |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `whisper-1`, `gpt-4o-transcribe`, `gpt-4o-mini-transcribe` |
| Groq | `https://api.groq.com/openai/v1` | `whisper-large-v3-turbo`, `whisper-large-v3` |
| Mistral | `https://api.mistral.ai/v1` | `voxtral-mini-latest` |
| eigener Server (Speaches, LocalAI, …) | z. B. `http://192.168.1.10:8000/v1` | je nach Server |

- **Datenschutz:** Mit der KI-API gehen Aufnahmen und Sprachnachrichten an
  den Anbieter. Gesprächspartner informieren und eine
  Auftragsverarbeitung (DSGVO) abschließen; ein eigener Server im LAN hält
  die Daten im Haus.
- Jede Gesprächsseite wird getrennt geschickt (16 kHz mono WAV), lange
  Gespräche in Stücken bis 10 Minuten. Modelle mit Zeitstempeln
  (`whisper-1`, Groq) liefern die Sätze mit Zeit; bei Modellen ohne (z. B.
  `gpt-4o-transcribe`) schickt TalkOps jeden Gesprächsabschnitt einzeln,
  damit die Reihenfolge stimmt. Server, die die OpenAI-Optionen nicht
  kennen, bekommen nur Datei, Modell und Sprache.
- Der Schlüssel wird verschlüsselt gespeichert (wie das SMTP-Passwort) und
  nie angezeigt. Der Media-Worker braucht dafür `TALKOPS_SECRET_KEY` (in der
  `docker-compose.yml` ab 1.5 gesetzt).
- Getestet mit einem nachgebauten OpenAI-kompatiblen Server; die Abläufe der
  einzelnen Anbieter sind nach deren Dokumentation umgesetzt.

## Suche

**Suche** findet Wörter in allen Transkripten, auf die Sie Zugriff haben –
Gespräche und Sprachnachrichten. Schreibweise wie bei Suchmaschinen:
`"genaue Phrase"`, `-ausschließen`, `rechnung or mahnung`. Treffer lassen
sich direkt abspielen.

## Speicherplatz

Eine Minute Gespräch belegt etwa 1,9 MB (8 kHz, 16 Bit, Stereo; mit
HD-Codecs mehr). Aufnahmen liegen im Volume `recordings`
(`<mandant>/<jahr-monat>/<anruf>.wav`) – denken Sie an die Datensicherung.
