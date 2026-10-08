# 0019 – Transkription in zwei Durchgängen, optional über eine KI-API

- Status: Angenommen
- Datum: 2026-10-08
- Ergänzt [0012](0012-aufzeichnung-und-transkription.md): „Audio bleibt
  lokal“ gilt weiter als Standard, ist aber nicht mehr ausnahmslos.

## Kontext

Genaue lokale Modelle (large-v3) brauchen auf CPUs ein Mehrfaches der
Gesprächsdauer. Wer Genauigkeit will, wartet lange auf den Text – auch in
der Voicemail-Mail. Zugleich wünschen sich Betreiber die Möglichkeit, eine
KI-API (OpenAI, Groq, Mistral) oder einen eigenen GPU-Server im LAN zu
nutzen, der schneller und genauer ist als die CPU des TalkOps-Hosts.

## Optionen

1. Nur ein Durchgang, Wahl zwischen schnell und genau (Stand 1.4).
2. **Zwei Durchgänge**: zuerst schnell, danach genau; der zweite ersetzt
   den ersten.
3. Externe Dienste über je eigene SDKs (OpenAI, Deepgram, AssemblyAI, …).
4. **Eine OpenAI-kompatible Schnittstelle** (`POST /audio/transcriptions`),
   die OpenAI, Groq, Mistral und Server wie Speaches oder LocalAI anbieten.

## Entscheidung

- **Optionen 2 und 4.** Erster Durchgang (Standard: lokal *Schnell*) direkt
  nach dem Gespräch; sein Text wird angezeigt und geht mit der
  Voicemail-Mail raus. Ein optionaler zweiter Durchgang läuft als eigener
  Job (`pass: 2`) mit niedrigerer Priorität in derselben Transkriptions-Spur
  und ersetzt das Transkript. Bis dahin ist es als vorläufig markiert
  (`transcripts.final = false`); scheitert der zweite Durchgang, bleibt das
  erste Transkript und gilt als endgültig. `transcripts.engine` hält fest,
  welches Modell den Text erzeugt hat.
- Jeder Durchgang kann ein lokales Modell oder die **KI-API** nutzen. Der
  Media-Worker schickt jede Gesprächsseite getrennt als 16-kHz-WAV (Sprecher
  wie bei Whisper), lange Gespräche in Stücken bis 10 Minuten. Liefert das
  Modell keine Zeitstempel, schickt er jeden Sprachabschnitt einzeln; kennt
  der Server die OpenAI-Optionen nicht, nur Datei, Modell und Sprache.
- Aufrufe laufen über `curl` wie die Modell-Downloads (keine neue
  HTTP-Abhängigkeit im Worker); der Schlüssel geht über stdin, nicht über
  die Kommandozeile. Er wird wie das SMTP-Passwort mit
  `TALKOPS_SECRET_KEY` verschlüsselt gespeichert (ADR 0008); der Worker
  bekommt dafür diesen Schlüssel.
- Die KI-API ist **ausdrücklich einzuschalten**; die Oberfläche weist auf
  Datenschutz und Auftragsverarbeitung hin. Lokal bleibt der Standard.

## Konsequenzen

- Schneller erster Text, später ein genauerer – ohne dass die Mail wartet.
- Lokal-genau im zweiten Durchgang belegt die Spur weiter lange; neue erste
  Durchgänge warten höchstens auf einen laufenden zweiten.
- Mit KI-API verlässt Audio das System: Verantwortung des Betreibers
  (Information der Gesprächspartner, AV-Vertrag). Ein eigener Server im LAN
  vermeidet das.
- Anbieter ohne OpenAI-kompatible Schnittstelle (Deepgram, AssemblyAI)
  bleiben außen vor oder lassen sich über einen kompatiblen Proxy anbinden.
- Der Worker braucht `TALKOPS_SECRET_KEY`; die mitgelieferte
  `docker-compose.yml` setzt ihn.
