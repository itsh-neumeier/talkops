# 0012 – Gesprächsaufzeichnung mit record_session, lokale Transkription mit whisper.cpp

- Status: Angenommen, ergänzt durch [0019](0019-transkription-in-zwei-durchgaengen-und-ki-api.md)
- Datum: 2026-10-06

## Kontext

Phase 5 bringt Gesprächsaufzeichnung (mit Hinweisansage), Transkription von
Aufnahmen und Sprachnachrichten, eine Volltextsuche und automatische
Löschfristen. Aufnahmen sind besonders schützenswerte Inhalte (in
Deutschland § 201 StGB, DSGVO); Audio soll das System nicht verlassen.
Zielhardware sind Homelab-Server und kleine Büro-Server, meist ohne GPU.

## Optionen

Aufzeichnung:

1. **`record_session` im Dialplan** vor dem `bridge` – FreeSWITCH schreibt
   die Datei selbst, `RECORD_STEREO` trennt die Gesprächsrichtungen,
   `RECORD_ANSWER_REQ` startet erst bei Annahme.
2. **`uuid_record` per ESL** nach dem Annehmen – mehr Steuerung, aber eine
   zusätzliche Fehlerquelle (Event verpasst → keine Aufnahme).

Transkription:

1. **whisper.cpp** (`whisper-cli`) im Media-Worker – C/C++, keine
   Python-Laufzeit, läuft gut auf CPUs, MIT-Lizenz.
2. **faster-whisper / OpenAI-Whisper** (Python) – größeres Image, mehr
   Abhängigkeiten.
3. **Cloud-Dienste** – widersprechen „Audio bleibt lokal“.

Suche: PostgreSQL-Volltext (`tsvector`) oder eine separate Suchmaschine.

## Entscheidung

- **Aufzeichnung per `record_session`** in Stereo (links Anrufer, rechts
  Gegenseite), nur angenommene Gespräche, `stop_record_session` vor
  Ausweichzielen (Voicemail ist kein Teil des Gesprächs). Die Richtlinie
  (Mandanten-Vorgabe je Richtung, je Nebenstelle `always`/`never`, `never`
  gewinnt) wertet TalkOps beim Routing aus. Die Hinweisansage spielt
  `bridge_pre_execute_{a,b}leg_app` beiden Seiten nach Annahme vor; vor
  Warteschlangen hört sie der Anrufer vor dem Warten.
- Die Datei wird über die CDR-Variable `talkops_recording` mit dem
  Gesprächsdatensatz verknüpft (`recordings`-Tabelle), Pfad
  `<mandant>/<jjjj-mm>/<call-uuid>.wav` im Volume `recordings`.
- **whisper.cpp v1.9.4** im Media-Worker, gebaut mit allen x86-64-CPU-
  Varianten als dynamisch geladene Backends (die beste wird zur Laufzeit
  gewählt; läuft vom Atom bis zum aktuellen Server). Kanäle werden getrennt
  transkribiert und nach Zeit zusammengeführt – das ergibt Sprecher ohne
  Diarisierung. Transkription läuft in einer eigenen Job-Spur, damit Ansagen
  (TTS) nie warten.
- Modelle (`base`, `small`) werden beim ersten Bedarf geladen und per SHA-256
  geprüft; andere Modelle legt der Admin selbst ins Volume `models`.
- **Suche mit PostgreSQL** (`to_tsvector('simple', …)`, GIN-Index,
  `websearch_to_tsquery`); `simple` statt `german`, weil Gespräche gemischt
  deutsch/englisch sind und Eigennamen/Nummern nicht gestemmt werden sollen.
- Zugriff: Benutzer der beteiligten Nebenstellen und Admins; Zugriffe durch
  Admins auf fremde Gespräche, Löschungen und Abrufe stehen im Audit-Log.
  Löschfrist je Mandant (Standard 90 Tage), stündlicher Aufräumlauf.
- Bei aktivierter Transkription wartet die Voicemail-Mail auf das
  Transkript (oder dessen endgültiges Scheitern) und enthält den Text.

## Konsequenzen

- Keine neue Infrastruktur (kein Suchserver, keine GPU nötig); Audio und
  Text bleiben lokal.
- `base` transkribiert auf aktuellen CPUs schneller als Echtzeit; lange
  Gespräche belegen den Worker trotzdem minutenlang (daher die eigene Spur).
- Warteschlangen: Die Aufnahme beginnt beim Eintritt (inkl. Wartezeit).
  `record-template` von mod_callcenter würde erst ab Agentenannahme
  aufnehmen, gilt aber statisch je Queue und passt nicht zur Richtlinie je
  Anruf.
- Eine CUDA-Variante des Worker-Images ist möglich (`GGML_CUDA`), aber
  nicht Teil dieser Phase.
- Ohne „Alle Beteiligten wurden informiert“ ist Aufzeichnen in vielen
  Ländern unzulässig: Die Ansage ist standardmäßig an, die Doku weist darauf
  hin.
