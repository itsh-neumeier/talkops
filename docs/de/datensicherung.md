# Datensicherung und Wiederherstellung

Ein Backup enthält Datenbank, Voicemail, Ansagen, Provisioning, Türbilder,
ausstehende und zurückgewiesene Anrufdatensätze aus `cdr-spool` und
optional Gesprächsaufzeichnungen. Vollständige Wiederherstellungsbackups enthalten
zusätzlich ausgewählte `.env`-, Compose- und Override-Dateien einschließlich
`TALKOPS_SECRET_KEY`. Das Manifest enthält Prüfsummen, Versionsinformationen,
Compose-Projekt, Profile und Angaben zu fehlender Konfiguration.

Neue verschlüsselte Archive heißen `talkops-YYYYMMDD-HHMMSS.tar.gz.age` und
verwenden das age-Passwortformat. Das separate Backup-Passwort außerhalb der
gesicherten Dateien aufbewahren. Ohne dieses Passwort ist eine Wiederherstellung
nicht möglich. Reine Datenbackups ohne Passwort bleiben unverschlüsselt.

Modelle, Docker-Images, die FreeSWITCH-Laufzeitdatenbank und frühere Backups
werden nicht gesichert. Externe Speicher und Dienste benötigen eigene
Wiederherstellungsanleitungen. Archive regelmäßig auf einen anderen Rechner
kopieren; Backups auf derselben Platte helfen bei einem Plattenausfall nicht.

## Vollständige Compose-Backups

1. Eine separate Passwortdatei erstellen. Der ausführende Benutzer ist Eigentümer;
   Gruppe 10001 gibt dem Container Leserechte:

   ```sh
   openssl rand -base64 32 > backup-password
   sudo chown "$(id -u)":10001 backup-password
   sudo chmod 640 backup-password
   ```

2. `TALKOPS_DEPLOYMENT_FILES` in `.env` auf alle tatsächlich verwendeten Dateien
   setzen, beispielsweise `.env,docker-compose.yml,docker-compose.backup.yml`.
   Weitere Overrides und benötigte `*.env`-Dateien ergänzen; unbenutzte Namen
   entfernen. Keine Datei mit dem Backup-Passwort aufnehmen.
3. Mit dem Backup-Override starten:

   ```sh
   docker compose -f docker-compose.yml -f docker-compose.backup.yml up -d
   ```

Weitere aktive `-f`-, `--env-file`-, `--project-name`- und `--profile`-Optionen
bei jedem Befehl angeben. `COMPOSE_PROJECT_NAME` und `COMPOSE_PROFILES` liefern
die gesicherten Metadaten. Bei Auswahl über CLI-Optionen die Metadaten prüfen.

Das Override bindet das Deployment-Verzeichnis nur lesbar ein. TalkOps liest
ausschließlich ausgewählte Dateien. Das Passwort wird separat eingebunden.
Für andere Host-Pfade `TALKOPS_DEPLOYMENT_SOURCE` und
`TALKOPS_BACKUP_PASSWORD_SOURCE` setzen. `.env` privat halten und UID 10001
Leserechte auf eingebundene Konfiguration und Passwortdatei geben.

Unter **Einstellungen → Backups** Uhrzeit, Aufbewahrung und Aufzeichnungen
festlegen. Fehlende Dateien oder Zugangsdaten werden angezeigt. Geplante,
manuelle und CLI-Backups verwenden dieselben Einstellungen:

```sh
docker compose -f docker-compose.yml -f docker-compose.backup.yml exec talkops talkops backup
```

`--without-recordings` lässt Gesprächsaufzeichnungen weg. Ohne Deployment-Dateien
sind reine Datenbackups weiterhin möglich. Deployment-Dateien benötigen ein
verschlüsseltes Backup.

## Portainer

TalkOps kann den ursprünglichen Stack nicht aus seiner Container-Umgebung ableiten.

1. Die tatsächlich verwendete Stack-YAML als `docker-compose.yml` speichern.
2. Stack-Umgebungswerte in eine lokale `.env` kopieren.
3. Diese Dateien und Overrides unter **Einstellungen → Backups** auswählen.
4. Tatsächlichen Projektnamen und Profile eingeben.
5. Konfiguration speichern.

Importierte Dateien werden mit dem Anwendungsschlüssel verschlüsselt abgelegt.
Nach jeder Stack-Änderung erneut importieren. Anzeige und Manifest enthalten das
Importdatum. Eingebundene Quelldateien haben Vorrang. CLI-Sicherung importierter
Konfiguration benötigt ebenfalls `TALKOPS_SECRET_KEY`.

`TALKOPS_BACKUP_PASSWORD` separat als Container-Umgebungswert bereitstellen oder
eine private Datei einbinden und `TALKOPS_BACKUP_PASSWORD_FILE` setzen. Dieses
Passwort nicht in die importierte `.env` aufnehmen und separat aufbewahren.

## Wiederherstellung vor dem ersten Containerstart

Ein TalkOps-Image mit Format-2-Unterstützung verwenden. Produktiv einen passenden
Release-Tag festlegen. Die Prüfung benötigt keine Datenbank oder Anwendungsschlüssel.

1. Archiv auf dem neuen Linux-Host als `backup.archive` ablegen.
2. Passwort in einer privaten Datei `backup-password` ablegen, die der ausführende
   Benutzer lesen kann. Das Werkzeug übergibt es über die Container-Umgebung.
3. Host-Werkzeug exportieren:

   ```sh
   docker run --rm ghcr.io/itsh-neumeier/talkops-server:latest recovery-script > recover.sh
   ```

4. Deployment-Dateien in ein neues Verzeichnis exportieren:

   ```sh
   bash recover.sh prepare --archive backup.archive --password-file backup-password --output recovered
   ```

   Bestehende Dateien werden nicht überschrieben. Der Export enthält auch
   `recovery.json`. Exportierte Zugangsdaten sind unverschlüsselt: privat halten.
5. IP-Adressen, Domain, Ports, Pfade, Volumes, Profile, Projektname und Images
   prüfen. Ursprünglichen `TALKOPS_SECRET_KEY` beibehalten. Fehlende Angaben
   ergänzen. Keine Compose-Datei unbekannter Herkunft ausführen.
6. Geprüfte Dateien im Deployment-Verzeichnis ablegen. Die separat benötigte
   Passwortdatei bereitstellen und UID 10001 nötige Leserechte geben.
7. Mit allen aktiven Deployment-Optionen wiederherstellen:

   ```sh
   bash recover.sh restore --archive /path/to/backup.archive \
     --password-file /path/to/backup-password \
     -f docker-compose.yml -f docker-compose.backup.yml \
     --env-file .env --project-name talkops
   ```

Das Werkzeug prüft Archiv und Compose-Konfiguration und verlangt `RESTORE` als
Bestätigung. Es stoppt TalkOps, Media-Worker und FreeSWITCH, startet nur PostgreSQL,
stellt Daten wieder her und startet anschließend den Stack mit Zustandsprüfung.
`--yes` nur nach Prüfung des Ziels verwenden. `--image` wählt das Prüf-Image.
TalkOps benötigt keinen Docker-Socket.

## Geführte Ersteinrichtung

**Aus Backup wiederherstellen** auswählen. Für die Browser-Prüfung auf dem Host
ein zufälliges `TALKOPS_SETUP_TOKEN` mit mindestens 32 Zeichen setzen und TalkOps
neu starten. Token, Archiv und Passwort im Assistenten eingeben.

Der Assistent zeigt Datum, Versionen, Volumes und fehlende Konfiguration, bietet
nach Bestätigung einen Download der unverschlüsselten Deployment-Dateien und
führt durch Host- und Portainer-Schritte. Nach dem Offline-Restore prüft er, ob
ein Konto vorhanden ist, und öffnet die normale Anmeldung.

Browser-Grenzen: 512 MiB Upload und 8 GiB entpackte Daten. Größere Archive mit dem
Host-Werkzeug prüfen. CLI-Standard: 64 GiB; `inspect --max-bytes` oder
`TALKOPS_RESTORE_MAX_BYTES` ändern diese Grenze. Temporärer Speicher muss komprimierte
und entpackte Daten aufnehmen können. Höchstens 100.000 Einträge, 32 MiB Manifest
und 1 MiB Deployment-Daten. Sicherung und Wiederherstellung verwenden dieselben Grenzen.

Die Prüfung benötigt lokales Token und CSRF-Anfrageheader. Prüfung und Kontoanlage
verwenden dieselbe Datenbanksperre. Sobald ein Konto existiert, ist der Prüfzugang
geschlossen. Das Setup-Token danach entfernen.

## Alte Backups und manueller Restore

Format-1-Archive bleiben lesbar. Ursprüngliche `.env`, Compose-Dateien und Schlüssel
bereitstellen. Ohne Schlüssel bleiben gespeicherte Zugangsdaten unlesbar. Bei alten
Archiven `--password-file` weglassen. Ohne Deployment-Dateien meldet der Export einen Fehler.

Die drei schreibenden Dienste stoppen; PostgreSQL laufen lassen. Ein temporärer
Container benötigt dieselben Volumes und die geprüfte Zielumgebung:

```sh
docker compose stop freeswitch talkops media-worker
docker compose up -d --wait postgres
docker compose run --rm --no-deps \
  -v "$PWD/backup.archive:/restore.archive:ro" \
  talkops restore --yes /restore.archive
docker compose up -d --wait
```

Bei jedem Befehl aktive Overrides und Umgebungsoptionen angeben. Für verschlüsselte
Archive das Passwort bereitstellen. In Portainer geprüfte Stack- und Umgebungswerte
importieren, Dienste stoppen und den entsprechenden Restore-Container auf dem Host
ausführen. Tatsächliche Volume-Namen verwenden; keinen zweiten Stack mit leeren Daten starten.

## Fehler und Prüfung

Vor Änderungen wird das komplette Archiv geprüft. Falsche Passwörter, beschädigte
Daten, neuere Schemas, falsche Prüfsummen, unsichere Pfade, Links und Gerätedateien
werden abgelehnt. Dateien werden vorbereitet und Dumps mit PostgreSQL-Werkzeugen
geprüft. Datenbank- und Dateisperren verhindern konkurrierende unterstützte Prozesse.
Bei NAS-Speicherung die Unterstützung für Dateisperren prüfen. Für Volumes echte
Verzeichnisse verwenden; Symlinks als Volume-Wurzel werden abgelehnt.

Ältere Schemas werden migriert. Sitzungen und Login-Challenges werden gelöscht.
Konten, Passwörter und MFA bleiben erhalten. Nach Anmeldung Dienstzustand,
Telefonregistrierungen und einen Testanruf prüfen. Auf dem Ziel die Konfigurationssicherung
aktivieren oder die tatsächliche Portainer-Konfiguration erneut importieren.

Bei einem Fehler Dienste gestoppt lassen und `restore-state.json` im Backups-Volume
lesen. Phasen: `database`, `volumes`, `complete`. Ursache beheben und mit dem
Originalarchiv erneut versuchen. Fehler vor dem Restore schreiben keinen neuen
Phasenstand. Datenbank und Dateien sind keine gemeinsame Transaktion; ein automatischer
Rollback beider Bereiche ist nicht vorgesehen.
