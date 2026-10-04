# 0009 – Basis-Images: Debian 13 „Trixie“

- Status: Angenommen
- Datum: 2026-10-05

## Kontext

Phase 0 baute alle Images auf Debian 12 „Bookworm“, das seit Juni 2026 nur
noch über Debian LTS gepflegt wird. Debian 13 „Trixie“ ist das aktuelle
Stable-Release. Zwei Hürden für FreeSWITCH 1.10.12:

1. Trixie enthält kein PCRE1 (`libpcre3`) mehr. FreeSWITCH-master ist auf PCRE2
   umgestellt, ein Release mit dieser Änderung gibt es noch nicht.
2. GCC 14 macht mehrere bisherige Warnungen zu Fehlern; FreeSWITCH baut mit
   `-Werror`, und curl 8.14 liefert zusätzliche Typprüfungen.

## Optionen

1. Bei Bookworm bleiben (LTS bis Mitte 2028).
2. Trixie + PCRE 8.45 selbst bauen (PCRE1 ist upstream eingestellt, keine
   Sicherheitsupdates).
3. Trixie + FreeSWITCH-master (unreleased) statt eines Releases.
4. **Trixie + FreeSWITCH 1.10.12 mit zurückportierten Upstream-PCRE2-Commits.**

## Entscheidung

Option 4:

- Alle eigenen Images nutzen `debian:trixie-slim` bzw. `rust:<ver>-trixie`,
  `node:<ver>-trixie-slim`; Postgres läuft als `postgres:17-trixie`.
  Caddy bleibt beim offiziellen Image (`caddy:2-alpine`, statisches Go-Binary,
  kein Debian-Image verfügbar).
- `docker/freeswitch/patches/0001-backport-pcre2.patch` enthält die Upstream-
  Commits 65bc7c1, 9092470 und 02549c1 (beschränkt auf Kern und gebaute Module).
  Der Patch entfällt mit dem ersten FreeSWITCH-Release, das sie enthält.
- `CFLAGS` enthält `-Wno-error` plus die von GCC 14 neu als Fehler
  eingestuften Diagnosen; Warnungen bleiben im Build-Log sichtbar.
- Der CI-Smoke-Test prüft, dass alle konfigurierten Module laden.

## Konsequenzen

- Aktuelle Bibliotheken mit regulärem Debian-Security-Support (OpenSSL 3.5,
  curl 8.14, PCRE2 10.46) statt LTS bzw. eingestelltem PCRE1.
- Ein kleiner, dokumentierter Patch-Stapel muss bei FreeSWITCH-Updates
  geprüft werden.
- Präzisierung zu [ADR 0003](0003-datenbank-postgresql.md): Image-Variante
  `postgres:17-trixie` statt `-alpine`.
