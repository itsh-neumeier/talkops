# 0010 – Voicemail in Rust, Sprachausgabe mit Piper, Mail aus dem TalkOps-Dienst

- Status: Angenommen
- Datum: 2026-10-06

## Kontext

Phase 3 bringt Voicemail mit Ansagen in mehreren Sprachen, eigener oder
vorgelesener Begrüßung und Mail-Benachrichtigung. FreeSWITCH bringt dafür
`mod_voicemail` und Sound-Pakete mit; die Daten sollen aber wie alles andere
in PostgreSQL liegen und im Web-UI verwaltet werden. Die Container laufen ohne
Internetzugriff zur Laufzeit zuverlässig weiter.

## Optionen

1. **`mod_voicemail`** – ausgereift, aber eigene SQLite/ODBC-Datenbank,
   Konfiguration per XML, Menüs nur über Phrase-Macros anpassbar, Mailversand
   über `mod_voicemail`-Templates mit `sendmail`.
2. **Eigene Abläufe in Rust über ESL outbound** (wie in [ADR 0006](0006-steuerung-xml-curl-und-esl.md)
   vorgesehen) mit Standard-Applikationen (`playback`, `record`,
   `play_and_get_digits`).
3. Sprachausgabe: FreeSWITCH-Soundpakete (Callie, nur Englisch in guter
   Qualität), Cloud-TTS, oder **Piper** (lokal, ONNX, CPU-tauglich).

## Entscheidung

- **Voicemail-Logik in Rust** (`talkops_api::voicemail`): FreeSWITCH übergibt
  den Anruf per `socket 127.0.0.1:8084 async full`; Abläufe laufen gegen ein
  `Call`-Trait und sind mit einem skriptbaren Fake testbar. Aufnahmen landen
  als WAV im gemeinsamen Volume `voicemail`, Metadaten in PostgreSQL. MWI per
  `MESSAGE_WAITING`-Event, auch direkt nach jeder neuen Registrierung.
- **Systemansagen** sind ein Katalog in `talkops_core::prompts` (Deutsch,
  Englisch, Zahlen 0–99). Der Media-Worker rendert fehlende Dateien beim Start
  mit **Piper** (Binary 2023.11.14-2, Stimmen `de_DE-thorsten-medium` (CC0)
  und `en_US-ljspeech-medium` (Public Domain), per SHA-256 im Image fixiert).
  Der Dateiname enthält einen Hash aus Stimme und Text, Änderungen erzeugen
  automatisch neue Dateien. Fehlende Ansagen werden übersprungen – die Mailbox
  funktioniert auch ohne TTS.
- **Begrüßungen** per Text werden als Job `tts.greeting` vom Media-Worker
  erzeugt; aufgenommene Begrüßungen entstehen am Telefon (`*97`, Taste 5).
- **Mailversand** (Job `mail.voicemail`) läuft im TalkOps-Dienst mit `lettre`
  (SMTP, STARTTLS/TLS über rustls), weil nur er den Schlüssel für das
  SMTP-Passwort hat (ADR 0008). Der Media-Worker bleibt ohne Secrets.

## Konsequenzen

- Keine zweite Datenhaltung, Menüs und Texte sind normaler, getesteter Code.
- Das Media-Worker-Image wird um ca. 150 MB größer (Piper + zwei Stimmen);
  weitere Sprachen brauchen eine Stimme und Übersetzungen im Katalog.
- Ansagen klingen synthetisch, sind aber in beiden Sprachen konsistent und
  ohne Lizenzfragen verteilbar.
- `voicemail` und `sounds` müssen in `freeswitch`, `talkops` und
  `media-worker` unter denselben Pfaden eingebunden sein (Compose erledigt das).
