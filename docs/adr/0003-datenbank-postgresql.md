# 0003 – Datenbank: PostgreSQL

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

TalkOps speichert Konfiguration (User, Geräte, Trunks, Rufnummern, Routing),
Anrufdaten (CDR), Transkripte (Volltextsuche), Audit-Log und eine Job-Queue.
Mehrere Prozesse greifen gleichzeitig zu (Server, Media-Worker). Mandanten-
fähigkeit (`tenant_id`) soll vorbereitet sein.

## Optionen

1. **PostgreSQL** – robust, transaktional, `LISTEN/NOTIFY`, `SKIP LOCKED`,
   Volltextsuche (`tsvector`, Sprachkonfigurationen `german`/`english`), JSONB,
   Row-Level-Security für spätere Mandantentrennung. Erstklassig in `sqlx`.
2. **SQLite** – kein separater Dienst, ideal für Kleinstinstallationen; aber
   schwach bei paralleler Schreiblast mehrerer Prozesse/Container, keine
   `NOTIFY`, eingeschränkte Volltextsuche (FTS5 ohne deutsche Stemmer), Backup
   laufender DBs heikler.
3. **MariaDB/MySQL** – verbreitet, aber ohne die genannten Postgres-Features;
   kein Mehrwert gegenüber Postgres.

## Entscheidung

**PostgreSQL 17** (Image `postgres:17-alpine`) als einzige Datenbank. Zugriff
über `sqlx` mit zur Laufzeit geprüften Queries, Migrationen mit
`sqlx migrate` (Verzeichnis `migrations/`, eingebettet ins Binary, beim
Start automatisch angewendet – abschaltbar über `TALKOPS_AUTO_MIGRATE=false`).

## Konsequenzen

- Ein zusätzlicher Container; im Gegenzug entfällt ein separater Message-Broker
  ([ADR 0005](0005-job-queue-postgres.md)).
- Alle Fachtabellen tragen `tenant_id`; die Initialmigration legt einen
  Default-Tenant an.
- Backup = `pg_dump` + Medien-Volumes (Phase 8).
- Integrationstests laufen mit `#[sqlx::test]` gegen einen echten Postgres
  (CI: Service-Container).
