# 0016 – Betrieb: Datensicherung, SIP-Anmeldeschutz und Härtung

- Status: Angenommen
- Datum: 2026-10-07

## Kontext

Phase 8 macht TalkOps betriebsfertig. Familien und kleine Betriebe haben
selten eigene Backup-Infrastruktur; eine Anlage mit SIP-Port im Internet
wird innerhalb von Minuten von Scannern (SIPVicious & Co.) nach schwachen
Passwörtern abgesucht. Die Container sollen mit möglichst wenig Rechten
laufen, ohne Portainer-Kompatibilität (kein `build:`) aufzugeben.

## Optionen

Datensicherung:

1. Nur dokumentieren (`pg_dump` + Volumes von Hand).
2. Externes Backup-Werkzeug (restic, Borg) als zusätzlicher Container.
3. **Eingebaut:** TalkOps schreibt ein Archiv mit `pg_dump` und den Volumes,
   zeitgesteuert und auf Knopfdruck; Wiederherstellung per CLI.

SIP-Anmeldeschutz:

1. fail2ban auf dem Host – braucht Log-Parsing und Host-Zugriff, nicht
   Portainer-tauglich.
2. Firewall-Regeln aus dem Container (nftables) – braucht `NET_ADMIN`.
3. ACLs in FreeSWITCH (`apply-register-acl`) – die ACLs von Sofia sind
   Vertrauenslisten (Treffer überspringen teils die Authentisierung), als
   Sperrliste ungeeignet.
4. **In TalkOps:** Fehlversuche aus dem ESL-Ereignis
   `sofia::register_failure` zählen, gesperrten Adressen im Directory
   (`mod_xml_curl`, Parameter `ip`) keinen Benutzer liefern.

## Entscheidung

Datensicherung Option 3, SIP-Anmeldeschutz Option 4.

- **Archiv:** `talkops-JJJJMMTT-HHMMSS.tar.gz` mit `manifest.json` (Format,
  Version, Schemastand, Volumes), `database.dump` (`pg_dump -Fc`) und
  `files/<volume>/…`. Das Passwort geht über `PGPASSWORD` an `pg_dump`,
  nie über die Kommandozeile. Täglich zu einer einstellbaren Stunde,
  Aufbewahrung der letzten N Archive, Download im UI.
- **Wiederherstellung** nur offline (`talkops restore --yes`): Schema
  `public` leeren, `pg_restore --single-transaction`, danach Migrationen
  (ältere Sicherungen werden hochgezogen, neuere abgelehnt), dann die
  Volumes im Archiv ersetzen. Symlinks und Pfade außerhalb des Volumes
  werden beim Entpacken verworfen.
- **SIP-Schutz:** Standard 10 Fehlversuche in 10 Minuten → 60 Minuten
  Sperre, einstellbar; Loopback (FreeSWITCH selbst, WebRTC-Relay) und
  eingetragene Netze werden nie gesperrt. Sperren liegen in der Datenbank
  (überstehen Neustarts), Fehlerzähler im Speicher.
- `/fs/xml` und `/fs/cdr` antworten nur Loopback-Peers (oder
  `TALKOPS_FS_PEERS`) und keinen Anfragen über einen Reverse-Proxy.
- **Härtung:** Sicherheits-Header und eine CSP mit Skript-Hashes
  (SvelteKit `csp.mode = hash`); in Compose `no-new-privileges`,
  `cap_drop: ALL`, schreibgeschützte Root-Dateisysteme für Server und
  Media-Worker.
- **Metriken:** `/metrics` im Prometheus-Format, nur mit
  `TALKOPS_METRICS_TOKEN` (Bearer).

## Konsequenzen

- Das Server-Image enthält `postgresql-client` passend zur Postgres-Version
  des Compose-Stacks (17); ein Wechsel der Postgres-Hauptversion muss beide
  anheben.
- Sicherungen liegen standardmäßig auf derselben Platte; die Doku verlangt
  das Kopieren an einen anderen Ort (Download oder NAS-Volume).
- Eine gesperrte Adresse bekommt `403` auch mit richtigem Passwort; viele
  Telefone hinter einer NAT-Adresse können sich gegenseitig aussperren –
  dafür gibt es vertrauenswürdige Netze und das Entsperren im UI.
- Länderfilter (GeoIP) sind nicht enthalten; sie bräuchten eine externe
  Datenbank mit eigener Lizenz.
