# 0005 – Job-Queue in PostgreSQL statt eigenem Broker

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

Langlaufende Aufgaben laufen asynchron: Transkription (Whisper, Sekunden bis
Minuten), TTS-Generierung (Piper), Voicemail-Mails, Webhook-Zustellung mit
Retry, Retention-Löschläufe. Die Last ist gering (kleine Firmen/Haushalte),
Zuverlässigkeit (kein Jobverlust bei Neustart) aber wichtig.

## Optionen

1. **Tabelle in PostgreSQL** mit `FOR UPDATE SKIP LOCKED` + `LISTEN/NOTIFY`.
2. **Redis** (z. B. Streams) – zusätzlicher Dienst, Persistenz muss
   konfiguriert werden, Jobs und Fachdaten nicht transaktional gekoppelt.
3. **NATS JetStream / RabbitMQ** – leistungsfähig, aber für diese Last
   überdimensioniert; weiterer Dienst zum Betreiben und Sichern.
4. **Fertige Crates** (`apalis`, `graphile_worker`-Ports …) – möglich, aber
   die benötigte Logik ist klein; eine eigene Implementierung vermeidet eine
   weitere API-Abhängigkeit mit eigenem Release-Rhythmus.

## Entscheidung

Eigene, kleine Queue in PostgreSQL (`talkops_core::jobs`, Tabelle `jobs`):

- `enqueue` fügt ein und sendet `NOTIFY talkops_jobs`; akzeptiert jeden
  Executor, sodass ein Job **in derselben Transaktion** wie die auslösende
  Änderung angelegt werden kann (z. B. Aufnahme gespeichert → Transkriptions-Job).
- `claim` holt den nächsten fälligen Job eines unterstützten Typs per
  `UPDATE … WHERE id = (SELECT … FOR UPDATE SKIP LOCKED)`.
- Fehler → erneuter Versuch mit exponentiellem Backoff (30 s · 2ⁿ, max. 6 h)
  bis `max_attempts`, danach Status `failed`.
- Jobs verwaister Worker (Lease abgelaufen) werden mit `requeue_stale`
  zurückgestellt.
- Worker warten per `LISTEN` mit Polling-Fallback.

## Konsequenzen

- Kein zusätzlicher Dienst; Jobs sind im normalen DB-Backup enthalten.
- Durchsatz ist begrenzt (Hunderte Jobs/s) – für die Zielgruppe mehr als genug.
- Abgeschlossene Jobs müssen periodisch aufgeräumt werden (Retention-Job, Phase 5).
