# Coral Edge TPU speech recognition and virtual TPU: development prompts

Status: Interview completed with the user on 2026-10-24. No code changes yet.
Review base: `8f32ec7` on the task branch (release 1.8.10).

This file records the interview result and two development prompts. The
prompts use the standard frame of
[development-chat-prompt.md](../development-chat-prompt.md). The prompt text is
German, because it is a chat prompt for the user.

## Feasibility summary

- Google Coral Edge TPU (USB, M.2, Mini-PCIe) runs only fully int8-quantized
  TFLite models with static tensor shapes. The on-chip parameter cache is
  8 MB. Whisper does not fit, because its autoregressive decoder needs
  dynamic shapes.
- Small CTC models (for example quantized Conformer-CTC or wav2vec2-CTC) can
  run partly on the TPU. German and English are possible. The accuracy is
  lower than Whisper `base` or `small`, mainly on 8 kHz telephone audio.
- The TPU runs audio classifiers (voice activity, language DE/EN) well.
- The PCIe/M.2 driver (gasket) is not in the kernel since 5.13. It needs a
  DKMS module on the host. The USB Accelerator needs only libusb.
- Result: a hybrid. Coral gives fast live text and a fast first pass. Local
  Whisper on the CPU does the second pass and replaces the text (ADR 0019).

## Interview decisions

| Topic | Decision |
|---|---|
| Hardware | USB Accelerator and M.2/Mini-PCIe |
| Goals | Less CPU load, lower live latency, use existing hardware |
| Quality | Hybrid: Coral first pass and live, Whisper second pass |
| Architecture | Separate project `itsh-neumeier/coralapi-server` (MIT, Rust). Runs as an optional TalkOps sidecar or as an external LAN server |
| Language | Automatic DE/EN detection per channel, tenant language as fallback |
| Live interface | Subset of the OpenAI Realtime transcription WebSocket (ADR 0024) |
| TalkOps connection | New connection kind `coral` |
| Failure | Fall back to local Whisper and show the fallback in the UI |
| Scale | Homelab (1–2 calls), office (up to 5), larger (10+) through several TPUs |
| Gate | Phase 0 benchmark first. Continue only if it passes |
| Virtual TPU | Frigate ZMQ detector, generic remote TPU API, later a libedgetpu shim, USB/IP documentation |
| TPU sharing | Configurable pools with roles `stt`, `vision`, `shared` |
| Security | API key per client, optional TLS, CurveZMQ or LAN binding for ZMQ |

## Order of work

Update 2026-10-24: the user selected Prompt C as the first step. Prompt C
puts Coral speech recognition into TalkOps on the same server. Prompts A and
B stay for later. Prompt C uses the same API contract, so the CoralAPI-Server
can replace the TalkOps service later without a change in the media worker.

1. Prompt A (CoralAPI-Server), phase 0. Stop if the gate fails.
2. Prompt A, phases 1–3 (STT API). Prompt B can start in parallel against a
   test double of the API contract below.
3. Prompt A, phases 4–6 (virtual TPU). This work does not block TalkOps.

## Shared API contract (STT)

Both prompts use this contract. A change needs agreement in both projects.

- `GET /health`: `{status, version, tpus:[{id, kind:"usb"|"pci", role, busy, model}], load, queue}`
- `GET /v1/models`: OpenAI list format. IDs such as `coral-ctc-de`,
  `coral-ctc-en`, `coral-auto` (DE/EN detection).
- `POST /v1/audio/transcriptions`: OpenAI multipart format (`file`, `model`,
  `language`, `response_format=json|verbose_json`). `verbose_json` returns
  segments with times and a detected `language`.
- `GET /v1/realtime?intent=transcription` (WebSocket): events as in ADR 0024.
  Client to server: `session.update` (PCM16, 24 kHz, `turn_detection: null`,
  `transcription.model`, `transcription.languages: ["de","en"]`),
  `input_audio_buffer.append`, `input_audio_buffer.commit`. Server to client:
  `input_audio_buffer.committed`,
  `conversation.item.input_audio_transcription.delta`, `.completed` (with the
  extra field `language`), `.failed`, `error`.
- Overload: HTTP 503 with `Retry-After`, or the realtime `error` event with
  `code: "overloaded"`. TalkOps then falls back.
- Auth: `Authorization: Bearer <key>`, optional if the server has no keys.

---

## Prompt A: CoralAPI-Server (new project)

```text
Du arbeitest als Entwicklungsassistent am neuen Projekt CoralAPI-Server
(Repository itsh-neumeier/coralapi-server, Lizenz MIT). Das Projekt ist
eigenständig, wird aber zuerst von TalkOps (itsh-neumeier/talkops) genutzt.
Claude Code und Codex können parallel arbeiten.

Das Repository ist neu. Lege zuerst die gemeinsamen Projektregeln an und
übernimm dabei die Konventionen von TalkOps, soweit sie passen: `CLAUDE.md`
als gemeinsame Quelle, `AGENTS.md` verweist darauf, `docs/ai-memory/` mit
`README.md` und `TEMPLATE.md`, `docs/adr/`, `docs/documentation-style.md`
(englisches ASD-STE100), Conventional Commits, GitHub-Actions-CI. Lies als
Vorlage in TalkOps: `CLAUDE.md`, `docs/ai-memory/README.md`,
`docs/documentation-style.md`, `docs/adr/0019-*.md`, `docs/adr/0024-*.md`,
`crates/talkops-media-worker/src/realtime.rs` und
`docs/proposals/coral-transcription-dev-prompts.md` (Interview-Ergebnis und
verbindlicher API-Vertrag).

Prüfe zuerst Remotes, Branch, Arbeitsbaum und vorhandene Änderungen. Nutze
für diese Aufgabe einen eigenen Feature-Branch: `codex/<aufgabe>` für Codex
oder `claude/<aufgabe>` für Claude Code. Starte vom aktuellen verifizierten
Remote-Zielbranch. Übernimm keine uncommitteten Änderungen aus anderen
Worktrees und überschreibe keine fremden Arbeitsstände.

Setze die folgende Aufgabe phasenweise um. Triff gewöhnliche technische
Entscheidungen selbst und frage nur bei fehlenden Informationen, die das
Ergebnis wesentlich verändern. Halte die Änderungen auf die Aufgabe begrenzt,
ergänze passende Tests und Dokumentation und führe die relevanten Prüfungen aus.
Unterscheide erfolgreiche, fehlgeschlagene und nicht ausgeführte Prüfungen.
Prüfungen, die echte Coral-Hardware brauchen, kennzeichnest du gesondert.

Führe eine eigene Aufgaben-Zusammenfassung in `docs/ai-memory/` nach der
Vorlage `TEMPLATE.md`. Aktualisiere sie nach wichtigen Entscheidungen,
an sinnvollen Zwischenständen und vor Abschluss oder Übergabe. Halte Ziel,
Ergebnis, Entscheidungen, Dateien, Prüfungen, Abhängigkeiten, offene Punkte
und den nächsten Schritt fest. Committe sie mit den Aufgabenänderungen und
verlinke sie in der PR. So können andere Modelle und Chats fortsetzen.

Committe die fertigen Änderungen mit Conventional Commits, pushe deinen
Feature-Branch und erstelle je Phase eine PR gegen den Zielbranch.
Commit, Push und PR-Erstellung für diese Aufgabe sind hiermit autorisiert.
Beschreibe in der PR Ziel, Quellbranch und Head-Commit, Änderungen,
Testergebnisse (mit/ohne Hardware), Abhängigkeiten und vorgeschlagene
englische Release Notes. Markiere die PR erst als bereit, wenn die Phase
abgeschlossen ist; bei offenen Problemen bleibt sie ein Entwurf.
Wenn GitHub-Zugriff fehlt, bewahre die Arbeit lokal und benenne genau, welcher
Schritt offen ist. Berichte zum Abschluss knapp das Ergebnis, den Prüfstatus,
die PR-URL und mögliche Blocker. Sende anderen Chats nur Nachrichten, wenn
ich das ausdrücklich beauftrage.

Aufgabe:
Baue CoralAPI-Server: einen Rust-Server, der Google-Coral-Edge-TPUs (USB
Accelerator und M.2/Mini-PCIe, auch mehrere und Dual-TPU) im Netzwerk
bereitstellt. Er hat zwei Rollen:
(1) Spracherkennung Deutsch/Englisch mit einer OpenAI-kompatiblen API
    (Datei und Realtime), primär für die Live-Transkription von TalkOps.
(2) Ein „virtueller TPU over IP“ (vergleichbar mit vGPU over IP), den andere
    Anwendungen wie Frigate nutzen können.

Phase 0 – Machbarkeits-Gate (zuerst, eigene PR, Ergebnis als Bericht in
`docs/research/phase0-benchmark.md`):
- Wähle Kandidatenmodelle für DE und EN, die sich vollständig int8 als TFLite
  quantisieren und mit `edgetpu_compiler` kompilieren lassen: CTC-Modelle
  mit festen Eingabefenstern (z. B. Conformer-CTC, QuartzNet/Citrinet,
  wav2vec2-CTC klein). Dazu ein Sprach-ID-Klassifikator DE/EN und optional
  ein VAD-Modell. Prüfe Lizenzen (kommerzielle Nutzung, Weitergabe der
  kompilierten Modelle) und dokumentiere die Quellen.
- Konvertierungs- und Benchmark-Werkzeuge dürfen Python sein (`tools/`), da
  das TFLite/Edge-TPU-Ökosystem dort liegt. Die Laufzeit ist Rust.
- Miss je Modell: Anteil der Ops auf der TPU, Wortfehlerrate auf
  Telefonaudio (8 kHz, auf 16 kHz hochgerechnet; z. B. Common Voice DE/EN
  telefoniegefiltert), Echtzeitfaktor und Latenz pro Fenster auf USB und
  M.2, CPU-Last des Hosts, Vergleich mit whisper.cpp `base` und `small` auf
  derselben CPU.
- Gate: Weiter nur, wenn ein Modell je Sprache mindestens 2 Kanäle in
  Echtzeit auf einer TPU schafft, die Live-Latenz pro Segment unter etwa
  1,5 s liegt und die Host-CPU-Last deutlich unter whisper.cpp `base` liegt.
  Die Wortfehlerrate darf schlechter als Whisper sein (Hybrid: Whisper
  ersetzt den Text im zweiten Durchgang), muss aber als Live-Vorschau
  lesbar sein; nenne die Werte. Ist das Gate nicht erfüllt, stoppe nach
  dem Bericht und schlage Alternativen vor (z. B. Coral nur für VAD/Sprach-ID).

Phase 1 – Grundgerüst und TPU-Laufzeit:
- Cargo-Workspace, edition 2024, axum/tokio, `thiserror` in Bibliotheken,
  `anyhow` in Binaries, clippy `-D warnings`. `unsafe` ist nur im
  FFI-Crate (`coral-sys` o. ä.) für die TFLite-C-API und libedgetpu erlaubt,
  mit `#![deny(unsafe_op_in_unsafe_fn)]` und SAFETY-Kommentaren; alle
  anderen Crates `#![forbid(unsafe_code)]`.
- Geräteerkennung (USB über libusb inkl. der Umnummerierung nach dem
  Firmware-Laden, PCIe über `/dev/apex_*`), Hot-Plug, Gesundheitsstatus.
- TPU-Pools: Jede TPU bekommt per Konfiguration eine Rolle `stt`, `vision`
  oder `shared`. Auf `shared` gilt die Priorität Live-STT > Vision > Datei-STT.
  Minimiere Modellwechsel (der Parameter-Cache hält nur ein Modell);
  nutze Co-Kompilierung, wo möglich. Lastmeldung und Warteschlange mit
  Grenzen; bei Überlast 503 mit `Retry-After`.
- Konfiguration per TOML-Datei und Umgebungsvariablen; Prometheus-Metriken
  (`/metrics`) je TPU (Auslastung, Modellwechsel, Latenz, Warteschlange).
- Sicherheit: API-Keys pro Client (gehasht gespeichert, nie geloggt),
  optional TLS mit eigenem Zertifikat; Bindung an konfigurierbare Adressen.

Phase 2 – STT-Datei-API:
- `GET /health`, `GET /v1/models`, `POST /v1/audio/transcriptions` genau nach
  dem API-Vertrag in `docs/proposals/coral-transcription-dev-prompts.md`
  (TalkOps-Repo). Modelle `coral-ctc-de`, `coral-ctc-en`, `coral-auto`.
- Automatische Spracherkennung DE/EN pro Datei/Kanal in den ersten
  Sekunden; `language` als Vorgabe oder Liste möglicher Sprachen.
- Segmente mit Zeitstempeln (aus VAD und CTC-Ausrichtung), `verbose_json`.

Phase 3 – Realtime-Transkription:
- WebSocket `/v1/realtime?intent=transcription` als Teilmenge der OpenAI
  Realtime API, kompatibel zum Client in TalkOps `realtime.rs`
  (session.update, input_audio_buffer.append/commit, delta/completed/failed,
  error). PCM16 24 kHz, client-seitige Turn-Erkennung (`turn_detection: null`).
- Streaming über feste, überlappende Fenster; Deltas so früh wie möglich,
  `completed` mit erkannter Sprache (Zusatzfeld `language`).
- Last: Homelab (1–2 Gespräche), Büro (bis 5), größer (10+) über mehrere
  TPUs; jeweils mit Benchmark-Wert in der Doku.

Phase 4 – Virtueller TPU: Frigate-ZMQ-Detektor:
- ZMQ-REP-Endpunkt, kompatibel zu Frigates Detektor `type: zmq`
  (Protokoll aus dem Frigate-Quelltext übernehmen und mit Quelle/Version
  dokumentieren), inkl. Modellübertragung. Läuft auf Pool `vision`/`shared`.
- Absicherung über CurveZMQ-Schlüssel oder Bindung an LAN-Adressen.
- Doku mit Frigate-Beispielkonfiguration und Integrationstest mit einem
  ZMQ-Testclient (optional Frigate im Container).

Phase 5 – Generische Remote-TPU-API:
- gRPC (tonic) oder WebSocket: Modell hochladen (Cache per SHA-256),
  Invoke mit Eingabetensoren, Ausgabetensoren zurück, Batch/Streaming,
  Prioritätsklasse pro Client. Rust-Client-Crate `coralapi-client`.
- Dokumentiere Latenz-Overhead gegenüber lokaler TPU.

Phase 6 – libedgetpu-Shim (experimentell) und USB/IP-Doku:
- Konzept und Prototyp einer Ersatzbibliothek bzw. eines TFLite-Delegates,
  die Edge-TPU-Aufrufe transparent an den Server weiterleitet.
  Als experimentell kennzeichnen.
- Doku, wie man einen Coral-USB-Stick roh per USB/IP durchreicht, mit
  Warnungen (kein Zugriffsschutz, keine Verschlüsselung, Umnummerierung
  beim Firmware-Laden).

Auslieferung:
- Image `ghcr.io/itsh-neumeier/coralapi-server` auf Debian 13 „trixie“,
  läuft als UID/GID 10001, ohne zusätzliche Capabilities. Gerätezugriff über
  `devices:` und `group_add:` (USB: `/dev/bus/usb`, PCIe: `/dev/apex_0`).
  Beispiel-Compose ohne `build:`.
- Doku für Host-Vorbereitung: gasket-DKMS für M.2/PCIe auf Debian trixie
  (Kernel 6.12), udev-Regeln für USB, Proxmox-/LXC-Durchreichung.
- Englische Doku (ASD-STE100), README mit Kurzstart; deutsche Übersetzung
  der Benutzerdoku.

Akzeptanzkriterien / Einschränkungen:
- Phase 0 ist ein hartes Gate; ohne bestandenes Gate keine Phasen 1–6.
- Der API-Vertrag im TalkOps-Repo ist verbindlich; Änderungen nur nach
  Abstimmung mit dem TalkOps-Chat (über mich).
- CI läuft ohne Hardware (TPU-Abstraktion mit CPU-/Mock-Backend);
  Hardware-Tests sind markiert und manuell ausführbar.
- Keine Secrets in Logs; Modelle mit Quelle und Lizenz dokumentiert.
- Jede Phase endet lauffähig, getestet, dokumentiert und mit grüner CI.
```

---

## Prompt B: TalkOps integration (connection kind `coral`)

```text
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
Binde den CoralAPI-Server (itsh-neumeier/coralapi-server) als lokale
Spracherkennung für Deutsch und Englisch in TalkOps ein, primär für die
Live-Transkription, aber auch für Aufnahmen und Voicemails. Lies vorher
`docs/proposals/coral-transcription-dev-prompts.md` (Interview-Ergebnis und
verbindlicher API-Vertrag), ADR 0019, 0022 und 0024 sowie
`crates/talkops-core/src/transcription_api.rs`,
`crates/talkops-core/src/transcription_profiles.rs`,
`crates/talkops-media-worker/src/{live.rs,realtime.rs,api.rs,transcribe.rs}`
und `web/src/lib/components/TranscriptionSettings.svelte`.

Umfang:
1. Neue ADR (supplements 0024): Verbindungsart `coral`, Hybrid-Betrieb,
   Fallback, Sidecar vs. externer Server.
2. Datenmodell: neue Migration, die `transcription_providers.kind` um
   `coral` erweitert (vorhandene Migrationen nicht ändern). `coral` hat eine
   frei wählbare URL (http/https, LAN), optionalen API-Key (verschlüsselt
   nach ADR 0008), keine Kosten (kein Eintrag in Preislisten, Nutzung in
   `transcription_usage` mit Preis 0 oder gar nicht – entscheide und
   begründe) und keinen Cloud-Datenschutzhinweis, stattdessen einen
   LAN-Hinweis, wenn die URL nicht privat ist.
3. API (`routes/transcription.rs`): Verbindung anlegen/testen; der Test ruft
   `/health` und `/v1/models` und liefert TPU-Anzahl, Rollen, Last und
   Modelle zurück. utoipa-Annotationen, Rollenprüfung, Audit-Eintrag ohne
   Secrets.
4. Media-Worker:
   - Datei-Transkription über `/v1/audio/transcriptions` mit `verbose_json`
     (Segmente, Zeiten, erkannte Sprache).
   - Live über den vorhandenen Realtime-Client (`realtime.rs`). Für `coral`
     `session.update` mit `transcription.languages: ["de","en"]` (Auto-
     Erkennung, Tenant-Sprache an erster Stelle) senden und das Zusatzfeld
     `language` aus `completed` übernehmen.
   - Der Realtime-Client muss für `coral` auch `ws://` (aus `http://`) und
     selbst signierte bzw. eigene CA-Zertifikate unterstützen (konfigurier-
     bar pro Verbindung, z. B. hinterlegtes CA-PEM), ohne die OpenAI-
     Verbindungen zu schwächen.
   - Fallback: Ist Coral nicht erreichbar, meldet 503/`overloaded` oder
     bricht der Stream ab, fällt live auf lokales Whisper `base` im
     Teile-Modus und Aufnahmen/Voicemails auf das lokale Profil zurück.
     `transcripts.engine` hält das tatsächlich genutzte Modell fest. Die
     Live-Vorschau und die Transkript-Ansicht zeigen den Fallback an.
     Nach Fallback periodischer neuer Versuch (wie der 30-s-Backoff aus
     ADR 0024).
   - Hybrid: Empfohlene Standardvorgabe in der UI, wenn eine Coral-
     Verbindung existiert: `live` = Coral Realtime, `recording`/`voicemail`
     = Coral, `*_refine` = lokales Whisper (ersetzt den Text nach ADR 0019).
5. Speichere die erkannte Sprache pro Transkript bzw. Segment, falls das
   Datenmodell dafür noch kein Feld hat (prüfen; ggf. Migration), und zeige
   sie in der UI an.
6. UI (`TranscriptionSettings.svelte`): Verbindungsart „Coral (lokal/LAN)“
   mit URL, optionalem Key, optionalem CA-Zertifikat, Test-Button mit
   TPU-Status; Profilauswahl der Coral-Modelle; Hinweis auf Hybrid-Betrieb.
   Alle Texte über `t()` in `en.ts` und `de.ts`; `docs/ui-guidelines.md`.
7. Deployment: optionaler Dienst `coral` im Compose-Profil `coral` in
   `docker-compose.yml` (Image `ghcr.io/itsh-neumeier/coralapi-server`, kein
   `build:`, UID/GID 10001, `devices:`/`group_add:` für USB und PCIe,
   auskommentierte Beispiele, `network_mode: host` wie die anderen Dienste
   oder nur an 127.0.0.1 gebunden). Für den externen Server genügt die URL.
   `.env.example` ergänzen.
8. Tests: `#[sqlx::test]` für alle neuen Queries; Worker-Tests gegen einen
   Test-Double des CoralAPI-Servers (HTTP und WebSocket nach dem API-
   Vertrag), inkl. Fallback-Pfade (Verbindungsfehler, 503, `overloaded`,
   Stream-Abbruch); Browser-Test der Einstellungen analog
   `tests/browser/transcription-settings.mjs`.
9. Doku: `docs/architecture.md`, `docs/en/recording.md`,
   `docs/en/active-calls.md` und die deutschen Gegenstücke
   (`docs/de/aufzeichnung.md`, `docs/de/laufende-gespraeche.md`) mit
   Einrichtung (Sidecar und externer Server), Hardware-Hinweisen (USB vs.
   M.2, gasket-DKMS) und Erwartung an die Qualität (Hybrid).

Akzeptanzkriterien / Einschränkungen:
- Abhängigkeit: Der API-Vertrag ist verbindlich. Die Umsetzung darf gegen
  den Test-Double starten, bevor CoralAPI-Server veröffentlicht ist; der
  Compose-Dienst bleibt hinter dem Profil `coral` und ist standardmäßig aus.
- Ohne Coral-Verbindung ändert sich am bisherigen Verhalten nichts.
- Lokal bleibt der Standard; `coral` ist ausdrücklich auszuwählen.
- Keine Secrets in Logs oder Audit-Einträgen.
- Alle CI-Prüfungen aus `.github/workflows/ci.yml` grün; Prüfungen mit
  echter Coral-Hardware sind im PR als nicht ausgeführt bzw. manuell
  gekennzeichnet.
```

---

## Prompt C: TalkOps with Coral on the same server (first step)

```text
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
Live-Vorschau und den ersten Durchgang, lokales Whisper ersetzt den Text im
zweiten Durchgang (ADR 0019).

Lies vorher `docs/proposals/coral-transcription-dev-prompts.md`
(Interview-Ergebnis, Machbarkeit und API-Vertrag), ADR 0012, 0019, 0022 und
0024, `crates/talkops-core/src/transcription_api.rs`,
`crates/talkops-core/src/transcription_profiles.rs`,
`crates/talkops-media-worker/src/{main.rs,live.rs,realtime.rs,api.rs,transcribe.rs}`,
`web/src/lib/components/TranscriptionSettings.svelte`, `docker-compose.yml`
und `docker/`.

Architektur (gleicher Server):
- Neuer Dienst `coral-stt` als eigenes Crate im Workspace
  (`crates/talkops-coral-stt`) und eigenes Image
  `ghcr.io/itsh-neumeier/talkops-coral-stt`. So bleiben Edge-TPU-
  Bibliotheken und Modelle aus dem Media-Worker heraus, und ein Absturz der
  TPU-Laufzeit trifft den Worker nicht.
- Der Dienst bietet nur lokal (Standard `127.0.0.1:8095`, konfigurierbar)
  die OpenAI-kompatible Teilmenge aus dem API-Vertrag an: `/health`,
  `/v1/models`, `/v1/audio/transcriptions`, `/v1/realtime?intent=transcription`.
  Der spätere eigenständige CoralAPI-Server soll ihn ohne Änderung am
  Worker ersetzen können. Kein Frigate, kein virtueller TPU, keine
  Netzwerkfreigabe in dieser Aufgabe.
- Rust. `unsafe` ist im Projekt verboten. Nutze für die TFLite-C-API und
  libedgetpu einen gepflegten sicheren Wrapper-Crate, falls einer reicht.
  Sonst schlage per neuer ADR eine eng begrenzte Ausnahme für ein einzelnes
  FFI-Crate vor (SAFETY-Kommentare, `#![deny(unsafe_op_in_unsafe_fn)]`,
  alle anderen Crates bleiben `#![forbid(unsafe_code)]`) und warte auf meine
  Zustimmung, bevor du `unsafe` einführst.
- Werkzeuge zur Modellkonvertierung und für Benchmarks dürfen Python sein
  (`tools/coral/`), laufen nur offline und sind nicht Teil der Images.

Phase 0 – Machbarkeits-Gate (zuerst, Ergebnis als Bericht in
`docs/research/coral-stt-benchmark.md`, eigener Commit, PR bleibt Entwurf):
- Wähle Kandidatenmodelle für DE und EN, die sich vollständig int8 als
  TFLite quantisieren und mit `edgetpu_compiler` kompilieren lassen
  (CTC-Modelle mit festen Fenstern, z. B. Conformer-CTC, Citrinet,
  wav2vec2-CTC klein), plus einen Sprach-ID-Klassifikator DE/EN. Prüfe die
  Lizenzen (Kompatibilität mit AGPL-3.0, Weitergabe kompilierter Modelle)
  und dokumentiere die Quellen.
- Miss: Anteil der Ops auf der TPU, Wortfehlerrate auf Telefonaudio (8 kHz),
  Echtzeitfaktor und Latenz pro Fenster auf USB und M.2, CPU-Last des Hosts,
  Vergleich mit whisper.cpp `base` auf derselben CPU.
- Gate: mindestens 2 Kanäle (ein Gespräch) in Echtzeit auf einer TPU,
  Live-Latenz pro Segment unter etwa 1,5 s, CPU-Last deutlich unter
  whisper.cpp `base`, Text als Live-Vorschau lesbar. Ohne Hardware im
  Container: baue Werkzeuge und Bericht-Gerüst, kennzeichne die Messungen
  als offen und frage mich nach den Messwerten von meiner Hardware.
- Ist das Gate nicht erfüllt, stoppe nach dem Bericht und schlage
  Alternativen vor (z. B. Coral nur für VAD/Sprach-ID).

Phase 1 – Dienst `coral-stt`:
- Geräteerkennung (USB inkl. Umnummerierung nach dem Firmware-Laden, PCIe
  über `/dev/apex_*`), mehrere TPUs, Gesundheitsstatus, Neustart einer
  hängenden TPU-Sitzung.
- Datei-API mit `verbose_json` (Segmente mit Zeiten, erkannte Sprache).
- Realtime-WebSocket kompatibel zu `realtime.rs` (session.update,
  input_audio_buffer.append/commit, delta/completed/failed, error),
  PCM16 24 kHz, `turn_detection: null`, Zusatzfeld `language` in
  `completed`. Feste überlappende Fenster, Deltas so früh wie möglich.
- Automatische Sprachwahl DE/EN pro Kanal aus `transcription.languages`.
- Lastgrenze und Warteschlange; Überlast als HTTP 503 mit `Retry-After`
  bzw. Realtime-`error` mit `code: "overloaded"`.
- Modelle liegen im Volume `models` (wie Whisper); Download beim ersten
  Start mit Prüfsumme, oder Vorbefüllung im Image – entscheide und begründe.
- Bindung nur an Loopback; kein API-Key nötig, aber optional unterstützt.
- Metriken/Status über `/health` (TPU-Anzahl, Art, Last, geladenes Modell).

Phase 2 – Integration in TalkOps:
1. Neue ADR (ergänzt 0024): Coral auf demselben Server, Hybrid-Betrieb,
   eigener Dienst, Fallback, späterer Wechsel auf einen externen Server.
2. Migration: `transcription_providers.kind` um `coral` erweitern
   (vorhandene Migrationen nicht ändern). Vorbelegte URL
   `http://127.0.0.1:8095/v1`, Key optional (verschlüsselt nach ADR 0008).
   Keine Kosten: keine Preisliste; entscheide und begründe, ob Nutzung in
   `transcription_usage` mit Preis 0 oder gar nicht erscheint. Kein Cloud-
   Datenschutzhinweis, denn Audio verlässt den Server nicht.
3. API (`routes/transcription.rs`): Verbindung anlegen und testen; der Test
   ruft `/health` und `/v1/models` und zeigt TPUs, Last und Modelle.
   utoipa-Annotation, Rollenprüfung, Audit-Eintrag ohne Secrets.
4. Media-Worker: Datei-Transkription über `/v1/audio/transcriptions`;
   Live über den vorhandenen Realtime-Client mit
   `transcription.languages` (Tenant-Sprache zuerst, dann die andere) und
   Übernahme von `language`. Der Realtime-Client muss für `coral`
   `ws://` (aus `http://`) erlauben, ohne OpenAI-Verbindungen zu schwächen.
5. Fallback: Ist `coral-stt` nicht erreichbar, meldet Überlast oder bricht
   der Stream ab, nutzt live lokales Whisper `base` im Teile-Modus und
   Aufnahmen/Voicemails das lokale Profil. `transcripts.engine` hält das
   tatsächlich genutzte Modell fest. Live-Vorschau und Transkript-Ansicht
   zeigen den Fallback. Neuer Versuch nach dem 30-s-Backoff aus ADR 0024.
6. Erkannte Sprache pro Transkript bzw. Segment speichern, falls noch kein
   Feld existiert (prüfen, ggf. Migration), und in der UI anzeigen.
7. UI (`TranscriptionSettings.svelte`): Verbindungsart „Coral (dieser
   Server)“ mit vorbelegter URL, Test-Button mit TPU-Status, Coral-Modelle
   in der Profilauswahl, Knopf „Hybrid übernehmen“ (`live` = Coral Realtime,
   `recording`/`voicemail` = Coral, `*_refine` = lokales Whisper). Alle
   Texte über `t()` in `en.ts` und `de.ts`; `docs/ui-guidelines.md`.

Phase 3 – Deployment und Doku:
- `docker/coral-stt/Dockerfile` auf Debian 13 „trixie“, UID/GID 10001,
  libedgetpu (Standard-Taktrate; Hinweis auf die Max-Variante und Wärme).
  CI baut und veröffentlicht das Image wie die anderen Images.
- `docker-compose.yml`: Dienst `coral-stt` hinter dem Profil `coral`
  (standardmäßig aus), nur `image:`, kein `build:`, `network_mode: host`
  mit Bindung an 127.0.0.1, gehärtet wie die anderen Dienste, Volume
  `models`, `devices:`/`group_add:` mit Beispielen für USB
  (`/dev/bus/usb`) und PCIe (`/dev/apex_0`). `docker-compose.dev.yml` baut
  das Image aus dem Quellcode. `.env.example` ergänzen.
- Doku in `docs/en/` und `docs/de/` (Aufzeichnung, laufende Gespräche):
  Einrichtung, Host-Vorbereitung (udev-Regel für USB, gasket-DKMS für
  M.2/PCIe auf Debian trixie, Proxmox-/LXC-Durchreichung), Erwartung an die
  Qualität (Hybrid), Fehlersuche. `docs/architecture.md` ergänzen.

Tests:
- `#[sqlx::test]` für alle neuen oder geänderten Queries.
- `coral-stt` mit einem CPU-/Mock-Backend, damit CI ohne Hardware läuft;
  Tests für Fenster, Sprachwahl, Realtime-Ereignisse und Überlast.
- Worker-Tests gegen einen Test-Double nach dem API-Vertrag (HTTP und
  WebSocket), inkl. aller Fallback-Pfade.
- Browser-Test der Einstellungen analog
  `tests/browser/transcription-settings.mjs`.
- Hardware-Prüfungen (USB und M.2) sind manuell und im PR gesondert
  ausgewiesen.

Akzeptanzkriterien / Einschränkungen:
- Phase 0 ist ein hartes Gate.
- Ohne Profil `coral` und ohne Coral-Verbindung ändert sich am bisherigen
  Verhalten nichts; lokal bleibt der Standard, `coral` ist ausdrücklich
  auszuwählen.
- Audio verlässt den Server nicht; der Dienst lauscht nur auf Loopback.
- Keine Secrets in Logs oder Audit-Einträgen.
- Alle CI-Prüfungen aus `.github/workflows/ci.yml` grün.
```

## Sources

- Home Assistant community: [Google Coral for Whisper and Piper?](https://community.home-assistant.io/t/google-coral-for-whisper-and-piper/664670)
- [whisper.cpp issue 1557: NPU support](https://github.com/ggml-org/whisper.cpp/issues/1557)
- [Google Coral Edge TPU benchmarks and experiments (2026)](https://www.x2q.net/post/google-coral-edge-tpu-experiments-2026/)
- [Community gasket driver](https://github.com/robertzaage/gasket-driver)
- Frigate ZMQ detector: [commit 152d9ed (mirror)](https://git.laurivan.com/Mirrors/blakeblackshear.frigate/commit/152d9ed4a04a02912087ad6a0b728f27a75532e9)
