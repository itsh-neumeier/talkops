Du arbeitest als Entwicklungsassistent an TalkOps. Claude Code und Codex
arbeiten parallel; ein separater Release-Chat übernimmt Integration, Merges
in den Zielbranch, Release-Versionen, Tags und Veröffentlichung.

Lies `AGENTS.md`, `CLAUDE.md` und `docs/release-workflow.md` und befolge die
gemeinsamen Projektregeln. Lies weitere Dokumentation passend zur Aufgabe.

Lies `docs/ai-memory/README.md` und die für deine Aufgabe relevanten
Zusammenfassungen. Prüfe ihre Angaben am aktuellen Code- und PR-Stand.
Schreibe neue technische Dokumentation und Aufgaben-Zusammenfassungen in
englischem ASD-STE100 gemäß `docs/documentation-style.md`.

Prüfe zuerst Remotes, Branch, Arbeitsbaum und vorhandene Änderungen. Nutze
für diese Aufgabe einen eigenen Git-Worktree und Feature-Branch:
`codex/<aufgabe>` für Codex oder `claude/<aufgabe>` für Claude Code. Verwende
einen bereits passenden, exklusiv für diese Aufgabe vorgesehenen Worktree
weiter; andernfalls lege einen neuen an. Starte vom vereinbarten Basis-Commit
oder, wenn keiner vorgegeben ist, vom aktuellen verifizierten Remote-Zielbranch.
Übernimm keine uncommitteten Änderungen aus anderen Worktrees und überschreibe
keine fremden Arbeitsstände.

Setze die folgende Aufgabe vollständig um. Triff gewöhnliche technische
Entscheidungen selbst und frage nur bei fehlenden Informationen, die das
Ergebnis wesentlich verändern. Halte die Änderungen auf die Aufgabe begrenzt,
ergänze passende Tests und Dokumentation und führe die relevanten Prüfungen aus.
Unterscheide erfolgreiche, fehlgeschlagene und nicht ausgeführte Prüfungen.

Führe eine eigene Aufgaben-Zusammenfassung in `docs/ai-memory/` nach der
Vorlage `TEMPLATE.md`. Aktualisiere sie nach wichtigen Entscheidungen,
an sinnvollen Zwischenständen und vor Abschluss oder Übergabe. Halte Ziel,
Ergebnis, Entscheidungen, Dateien, Prüfungen, Abhängigkeiten, offene Punkte
und den nächsten Schritt fest. Committe sie mit den Aufgabenänderungen und
verlinke sie in der PR. So können andere Modelle und Chats fortsetzen.

Committe die fertigen Änderungen mit Conventional Commits, pushe deinen
Feature-Branch und erstelle eine PR gegen den verifizierten Zielbranch.
Commit, Push und PR-Erstellung für diese Aufgabe sind hiermit autorisiert.
Übergebe dem Release-Chat über die PR: Ziel, Quellbranch und Head-Commit,
Änderungen, Testergebnisse, Abhängigkeiten, Migrations-IDs, Upgrade-Hinweise und
vorgeschlagene englische Release Notes. Markiere die PR erst als bereit, wenn
die Aufgabe abgeschlossen ist; bei offenen Problemen bleibt sie ein Entwurf.
Wenn GitHub-Zugriff fehlt, bewahre die Arbeit lokal und benenne genau, welcher
Schritt offen ist.

Release-Versionen, Release-Tags, Veröffentlichung und Merges in den Zielbranch
bleiben beim Release-Chat. Berichte zum Abschluss knapp das Ergebnis, den
Prüfstatus, die PR-URL und mögliche Blocker. Sende anderen Chats nur Nachrichten,
wenn ich das ausdrücklich beauftrage.

Aufgabe:
Ermögliche in TalkOps Spracherkennung für Deutsch und Englisch auf einer
Google-Coral-Edge-TPU (USB Accelerator oder M.2/Mini-PCIe), die im selben
Server wie TalkOps steckt. Primär für die Live-Transkription, zusätzlich für
Aufnahmen und Voicemails. Betrieb als Hybrid: Coral liefert die schnelle
Live-Vorschau und den ersten Durchgang; lokales Whisper ersetzt den Text im
zweiten Durchgang (ADR 0019). Lies dazu ADR 0012, 0019, 0022 und 0024 sowie
`transcription_api.rs`, `transcription_profiles.rs`, den Media-Worker
(`live.rs`, `realtime.rs`, `api.rs`, `transcribe.rs`) und
`TranscriptionSettings.svelte`.

Hintergrund: Whisper läuft nicht auf der Edge TPU (int8-TFLite, feste
Tensorformen, 8 MB Parameter-Cache). Geeignet sind kleine CTC-Modelle mit
festen Fenstern und ein Sprach-ID-Klassifikator DE/EN.

1. Phase 0 – Machbarkeits-Gate: Wähle und konvertiere Kandidatenmodelle für
   DE und EN plus Sprach-ID (Werkzeuge in Python unter `tools/coral/`, nur
   offline). Miss TPU-Anteil der Ops, Wortfehlerrate auf 8-kHz-Telefonaudio,
   Echtzeitfaktor, Latenz und Host-CPU-Last auf USB und M.2 im Vergleich zu
   whisper.cpp `base`. Bericht in `docs/research/coral-stt-benchmark.md`.
   Ohne Hardware: Werkzeuge und Bericht-Gerüst bauen und mich nach den
   Messwerten fragen.
2. Dienst `coral-stt`: eigenes Crate `crates/talkops-coral-stt` und Image
   `ghcr.io/itsh-neumeier/talkops-coral-stt`, damit TPU-Bibliotheken und
   Modelle nicht im Media-Worker liegen. Geräteerkennung für USB (inkl.
   Umnummerierung nach dem Firmware-Laden) und PCIe (`/dev/apex_*`), mehrere
   TPUs, Lastgrenze mit Warteschlange. Der Dienst lauscht nur auf
   `127.0.0.1:8095` (konfigurierbar) und bietet diese OpenAI-kompatible
   Teilmenge an:
   - `GET /health`: Status, TPUs (Art, Last, geladenes Modell), Warteschlange.
   - `GET /v1/models`: z. B. `coral-ctc-de`, `coral-ctc-en`, `coral-auto`.
   - `POST /v1/audio/transcriptions`: Multipart wie OpenAI; `verbose_json`
     mit Segmenten, Zeiten und erkannter `language`.
   - `GET /v1/realtime?intent=transcription`: kompatibel zum Client in
     `realtime.rs` (`session.update` mit PCM16 24 kHz,
     `turn_detection: null`, `transcription.languages`;
     `input_audio_buffer.append/commit`; Antworten `committed`, `delta`,
     `completed` mit Zusatzfeld `language`, `failed`, `error`).
   - Überlast: HTTP 503 mit `Retry-After` bzw. `error` mit
     `code: "overloaded"`.
   Ein späterer externer CoralAPI-Server soll den Dienst ohne Änderung am
   Worker ersetzen können.
3. Integration: neue ADR (ergänzt 0024). Migration für die Verbindungsart
   `coral` in `transcription_providers` mit vorbelegter URL
   `http://127.0.0.1:8095/v1`, optionalem Key, ohne Kosten und ohne
   Cloud-Datenschutzhinweis. Verbindungstest über `/health` und
   `/v1/models` mit TPU-Status. Worker nutzt die Datei-API und den
   Realtime-Client (`ws://` für `coral` erlauben; Tenant-Sprache zuerst,
   dann die andere). Erkannte Sprache pro Transkript bzw. Segment speichern
   und anzeigen.
4. Fallback: Ist `coral-stt` nicht erreichbar, überlastet oder bricht der
   Stream ab, nutzt live lokales Whisper `base` im Teile-Modus und
   Aufnahme/Voicemail das lokale Profil. `transcripts.engine` hält das
   genutzte Modell fest; die UI zeigt den Fallback. Neuer Versuch nach dem
   30-s-Backoff aus ADR 0024.
5. UI: Verbindungsart „Coral (dieser Server)“ mit Test-Button und
   TPU-Status, Coral-Modelle in der Profilauswahl, Knopf „Hybrid übernehmen“
   (`live` = Coral Realtime, `recording`/`voicemail` = Coral, `*_refine` =
   lokales Whisper).
6. Deployment: `docker/coral-stt/Dockerfile`, Dienst `coral-stt` im
   Compose-Profil `coral` mit Bindung an Loopback, Volume `models` und
   Beispielen für `devices:`/`group_add:` (USB `/dev/bus/usb`, PCIe
   `/dev/apex_0`). Image-Build in CI wie bei den anderen Images.
   `.env.example` ergänzen.
7. Doku in `docs/en/` und `docs/de/` (Aufzeichnung, laufende Gespräche) und
   `docs/architecture.md`: Einrichtung, Host-Vorbereitung (udev für USB,
   gasket-DKMS für M.2/PCIe auf Debian trixie, Proxmox/LXC), Qualität im
   Hybrid-Betrieb, Fehlersuche.

Akzeptanzkriterien / Einschränkungen (optional):
- Phase 0 ist ein hartes Gate. Ist es nicht erfüllt, stoppe nach dem Bericht
  und schlage Alternativen vor (z. B. Coral nur für VAD/Sprach-ID). Gate:
  mindestens ein Gespräch (2 Kanäle) in Echtzeit auf einer TPU, Live-Latenz
  pro Segment unter etwa 1,5 s, CPU-Last deutlich unter whisper.cpp `base`,
  Text als Live-Vorschau lesbar.
- `unsafe` ist verboten. Nutze einen gepflegten sicheren Wrapper-Crate für
  TFLite/libedgetpu. Reicht keiner, schlage per ADR eine eng begrenzte
  FFI-Ausnahme vor und warte auf meine Zustimmung.
- Modelllizenzen müssen mit AGPL-3.0 und der Weitergabe kompilierter Modelle
  vereinbar sein; Quellen dokumentieren.
- Ohne Profil `coral` und ohne Coral-Verbindung bleibt das Verhalten
  unverändert. Lokal bleibt der Standard; `coral` ist ausdrücklich zu wählen.
- Audio verlässt den Server nicht; der Dienst lauscht nur auf Loopback.
  Kein Frigate, kein virtueller TPU, keine Netzwerkfreigabe in dieser Aufgabe.
- Images auf Debian 13 „trixie“, UID/GID 10001; `docker-compose.yml` ohne
  `build:`.
- Tests: `#[sqlx::test]` für neue Queries; `coral-stt` mit Mock-Backend,
  damit CI ohne Hardware läuft; Worker-Tests gegen einen Test-Double inkl.
  aller Fallback-Pfade; Browser-Test analog
  `tests/browser/transcription-settings.mjs`. Hardware-Prüfungen sind
  manuell und im PR gesondert ausgewiesen.
