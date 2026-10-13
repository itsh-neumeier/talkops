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

Je Nebenstelle (**Einstellungen → Nebenstellen → Bearbeiten → Gesprächsaufzeichnung**):

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

Die Übersicht bietet eine optionale [Live-Vorschau](laufende-gespraeche.md)
für bereits aufgezeichnete Gespräche. Endgültige Transkripte bleiben separat.

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

**Lokale Modelle** (Einstellungen → Anrufbehandlung →
*Transkriptions-Engines*): *Schnell* nutzt das Modell aus
`TALKOPS_WHISPER_MODEL`. *Genau* (`large-v3-q5_0`, 1,1 GB, ca. 2 GB RAM) und
*Beste* (`large-v3`, 3,1 GB, ca. 4 GB RAM) nutzen das volle large-v3 mit
breiterer Suche: deutlich weniger falsche Wörter, aber zwei- bis viermal
langsamer. *Base* (ca. 150 MB) gibt es nur für die Live-Vorschau. **Namen und
Begriffe** (durch Komma getrennt) helfen bei Namen, Firmen und Fachwörtern.
Zum Vergleichen im Container:
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

### Engines je Zweck

*Einstellungen → Anrufbehandlung → Transkriptions-Engines* legt je Zweck eine
Engine fest ([ADR 0024](../adr/0024-transcription-engines-per-purpose.md)):

| Zweck | Wann | Standard |
|---|---|---|
| Live-Vorschau | Während eines laufenden, aufgezeichneten Gesprächs ([laufende Gespräche](laufende-gespraeche.md)) | Lokal *Base* |
| Aufnahme – schnell | Direkt nach dem Gespräch; sein Text erscheint zuerst. | Lokal *Schnell* |
| Aufnahme – langsam, genau | Optional; danach mit niedrigerer Priorität. | Aus |
| Voicemail – schnell | Direkt nach der Nachricht; die Mail enthält seinen Text. | Lokal *Schnell* |
| Voicemail – langsam, genau | Optional; danach mit niedrigerer Priorität. | Aus |

Jeder Zweck nutzt **Lokal** (ein Whisper-Modell auf diesem Server) oder
**Cloud** (ein Modell einer Cloud-Verbindung). Ein zweiter Durchgang ersetzt
das erste Transkript, sobald er fertig ist. Bis dahin steht am Transkript
*Vorläufig – genauere Fassung folgt*; die Ansicht aktualisiert sich von
selbst. Schlägt der zweite Durchgang fehl (ein Wiederholungsversuch), bleibt
das erste Transkript. Ein zweiter Durchgang mit derselben Engine wie der
erste läuft nicht.

Am Transkript steht, welches Modell es erstellt hat (z. B.
`whisper:large-v3-turbo-q5_0` oder `api:OpenAI/gpt-transcribe`).

Ein Update von 1.7 oder älter übernimmt die bisherigen Einstellungen: Die
bisherige KI-API wird zur Verbindung *OpenAI* (bzw. *API* bei anderer URL),
erster und zweiter Durchgang gelten für Aufnahmen und Voicemails.

### Cloud-Verbindungen (OpenAI, Groq, Mistral, eigener Server)

Unter *Cloud-Verbindungen* lassen sich eine oder mehrere Verbindungen mit
OpenAI-Transkriptionsschnittstelle anlegen (`POST …/audio/transcriptions`):
**Verbindung hinzufügen**, Typ wählen, Schlüssel eintragen, bei eigenem
Server auch die URL. **Verbindung testen** listet die Sprachmodelle des
Servers, ohne Audio zu senden, und nennt Modelle deiner Engines, die der
Server nicht anbietet.

| Typ | URL | Modelle (Beispiele) |
|---|---|---|
| OpenAI | fest: `https://api.openai.com/v1` | `gpt-transcribe`, `gpt-4o-mini-transcribe`, `gpt-4o-transcribe`, `whisper-1`; live: `gpt-live-transcribe` |
| Groq | `https://api.groq.com/openai/v1` | `whisper-large-v3-turbo`, `whisper-large-v3` |
| Mistral | `https://api.mistral.ai/v1` | `voxtral-mini-latest` |
| eigener Server (Speaches, LocalAI, …) | z. B. `http://192.168.1.10:8000/v1` | je nach Server |

- **Datenschutz:** Mit einem Cloud-Modell gehen Aufnahmen und
  Sprachnachrichten an den Anbieter, bei der Live-Vorschau schon während des
  Gesprächs. Gesprächspartner informieren und eine Auftragsverarbeitung
  (DSGVO) abschließen; ein eigener Server im LAN hält die Daten im Haus.
- Jede Gesprächsseite wird getrennt geschickt (16 kHz mono WAV), lange
  Gespräche in Stücken bis 10 Minuten. Modelle mit Zeitstempeln
  (`whisper-1`, Groq) liefern die Sätze mit Zeit; bei Modellen ohne (z. B.
  `gpt-transcribe`) schickt TalkOps jeden Gesprächsabschnitt einzeln, damit
  die Reihenfolge stimmt. Server, die die OpenAI-Optionen nicht kennen,
  bekommen nur Datei, Modell und Sprache.
- Der Schlüssel wird verschlüsselt gespeichert (wie das SMTP-Passwort) und
  nie angezeigt. Der Media-Worker braucht dafür `TALKOPS_SECRET_KEY` (in der
  `docker-compose.yml` ab 1.5 gesetzt).
- Eine Verbindung, die eine Engine nutzt, lässt sich nicht löschen. Diese
  Engines vorher auf eine andere Verbindung oder auf lokal stellen.

### Live-Vorschau in der Cloud

Für die Live-Vorschau **Cloud** wählen. Es gibt zwei Arten:

- **Ohne Streaming:** TalkOps schickt 8-Sekunden-Stücke der Aufnahme. Der
  Text kommt mit ca. 10 Sekunden Verzögerung. Das können alle Verbindungen.
- **Live streamen (Realtime-API):** Der Media-Worker schickt die Sprache jeder
  Gesprächsseite per WebSocket (`…/realtime?intent=transcription`). Text
  erscheint, während gesprochen wird, mit ca. einer Sekunde Verzögerung. Bei
  OpenAI `gpt-live-transcribe` verwenden. Der Worker schickt nur Sprache und
  beendet den Stream, wenn das Popup schließt, das Gespräch endet oder nach
  vier Stunden. Die WebSocket-Verbindung nutzt kein `HTTPS_PROXY`.

### Cloud-Nutzung und Kosten

*Cloud-Nutzung* zeigt die an jedes Cloud-Modell gesendeten Audiominuten in
diesem und im letzten Monat mit geschätzten Kosten. Die Schätzung nutzt den
Preis pro Minute der Engine, sonst den OpenAI-Listenpreis (Stand 2026-10-13,
z. B. `gpt-transcribe` 0,0045 USD, `gpt-live-transcribe` 0,017 USD pro
Minute). Ein Sternchen markiert Audio ohne bekannten Preis, etwa von einem
eigenen Server. Maßgeblich ist die Rechnung des Anbieters.

## Suche

**Suche** findet Wörter in allen Transkripten, auf die Sie Zugriff haben –
Gespräche und Sprachnachrichten. Schreibweise wie bei Suchmaschinen:
`"genaue Phrase"`, `-ausschließen`, `rechnung or mahnung`. Treffer lassen
sich direkt abspielen.

## Speicherplatz

Eine Minute Gespräch belegt etwa 1,9 MB (8 kHz, 16 Bit, Stereo; mit
HD-Codecs mehr). Aufnahmen liegen im Volume `recordings`
(`<mandant>/<jahr-monat>/<anruf>.wav`) – denken Sie an die Datensicherung.
