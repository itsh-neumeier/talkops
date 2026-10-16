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
[Sicherungsziele](#sicherungsziele) kopieren Archive automatisch.

## Vollständige Compose-Backups

`docker-compose.yml` bindet das Compose-Verzeichnis nur lesbar unter
`/var/lib/talkops/deployment` ein. Mit `TALKOPS_DEPLOYMENT_FILES=auto`
(Standard) sichert TalkOps diese Dateien des Verzeichnisses automatisch:
`.env`, `*.env`, `docker-compose*.yml`/`.yaml` und `compose*.yml`/`.yaml`.
TalkOps lässt Links, Dateien über 1 MiB, für UID 10001 nicht lesbare Dateien
und Dateien mit dem Backup-Passwort aus. **Einstellungen → Backup**
zeigt gesicherte und ausgelassene Dateien.

1. Eine separate Passwortdatei erstellen. Der ausführende Benutzer ist Eigentümer;
   Gruppe 10001 gibt dem Container Leserechte:

   ```sh
   openssl rand -base64 32 > backup-password
   sudo chown "$(id -u)":10001 backup-password
   sudo chmod 640 backup-password
   ```

2. Dem Container Leserechte auf `.env` geben, sonst privat halten:

   ```sh
   sudo chown "$(id -u)":10001 .env && sudo chmod 640 .env
   ```

3. Mit dem Backup-Override starten. Es bindet nur die Passwortdatei ein:

   ```sh
   docker compose -f docker-compose.yml -f docker-compose.backup.yml up -d
   ```

Weitere aktive `-f`-, `--env-file`-, `--project-name`- und `--profile`-Optionen
bei jedem Befehl angeben. `COMPOSE_PROJECT_NAME` und `COMPOSE_PROFILES` liefern
die gesicherten Metadaten. Bei Auswahl über CLI-Optionen die Metadaten prüfen.

Für eine feste Auswahl `TALKOPS_DEPLOYMENT_FILES` auf eine kommagetrennte Liste
setzen, z. B. `.env,docker-compose.yml`. Für andere Host-Pfade
`TALKOPS_DEPLOYMENT_SOURCE` und `TALKOPS_BACKUP_PASSWORD_SOURCE` setzen. Das
Compose-Verzeichnis darf keine weiteren privaten Daten enthalten, die UID 10001
lesen kann.

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

Portainer speichert den Stack in seinem eigenen Datenvolume. Der relative Bind
aus `docker-compose.yml` bindet dann ein leeres Verzeichnis ein; TalkOps nutzt
stattdessen die Portainer-API:

1. In Portainer unter **My account → Access tokens** ein Token anlegen. Einen
   Benutzer verwenden, der den TalkOps-Stack lesen darf.
2. In TalkOps **Einstellungen → Backup** öffnen und auf
   **Portainer verbinden** klicken.
3. Die Portainer-URL eingeben, z. B. `https://127.0.0.1:9443`. `http://` ist
   nur für `127.0.0.1` erlaubt.
4. Auf **Zertifikat prüfen** klicken. Bei einem selbstsignierten Zertifikat den
   SHA-256-Fingerabdruck mit dem Zertifikat von Portainer vergleichen und
   **Dieses Zertifikat festlegen** wählen. Spätere Verbindungen lehnen andere
   Zertifikate ab.
5. Token eingeben, auf **Stacks laden** klicken und den TalkOps-Stack wählen.
6. Auf **Speichern und Stack lesen** klicken.

Vor jeder Sicherung liest TalkOps die Stack-Datei (als `docker-compose.yml`) und
die Umgebungsvariablen des Stacks (als `.env`) und speichert sie mit dem
Anwendungsschlüssel verschlüsselt. `TALKOPS_BACKUP_PASSWORD` lässt TalkOps aus.
Ist Portainer nicht erreichbar, verwendet die Sicherung die zuletzt gespeicherte
Kopie, und die Anzeige zeigt den Fehler. Das Token wird verschlüsselt gespeichert
und nie angezeigt.

Ohne Portainer-Verbindung die Dateien manuell importieren:

1. Die tatsächlich verwendete Stack-YAML als `docker-compose.yml` speichern.
2. Stack-Umgebungswerte in eine lokale `.env` kopieren.
3. Diese Dateien und Overrides unter **Einstellungen → Backups** auswählen.
4. Tatsächlichen Projektnamen und Profile eingeben.
5. Konfiguration speichern.

Importierte Dateien werden mit dem Anwendungsschlüssel verschlüsselt abgelegt.
Nach jeder Stack-Änderung erneut importieren. Eingebundene Quelldateien haben
Vorrang vor Portainer-Verbindung und Import. CLI-Sicherung importierter
Konfiguration benötigt ebenfalls `TALKOPS_SECRET_KEY`.

`TALKOPS_BACKUP_PASSWORD` separat als Container-Umgebungswert bereitstellen oder
eine private Datei einbinden und `TALKOPS_BACKUP_PASSWORD_FILE` setzen. Dieses
Passwort nicht in die importierte `.env` aufnehmen und separat aufbewahren.

## Sicherungsziele

Sicherungsziele erhalten Kopien der Backup-Archive. Ein Sicherungsziel hat
zwei Teile (ADR 0036):

- Ein **Speicherziel** enthält die Verbindung: Protokoll, Server, Ordner,
  Passwort oder Schlüssel und Upload-Limit. Unter **Einstellungen → Speicher**
  auf **Speicherziel hinzufügen** klicken. Dafür ist die Berechtigung
  `storage.manage` nötig. Andere Funktionen können später dieselben
  Speicherziele nutzen.
- Ein **Sicherungsauftrag** wählt ein Speicherziel und optional einen
  Unterordner. Er hat Zeitplan, Aufbewahrung und die Einstellung für
  Aufzeichnungen. Unter **Einstellungen → Backup → Sicherungsaufträge** auf
  **Auftrag hinzufügen** klicken. Dafür ist die Berechtigung `backups.manage`
  nötig.

TalkOps kopiert die Archive mit [rclone](https://rclone.org). Das Server-Image
enthält eine fest eingestellte rclone-Version.

| Protokoll | Feld „Ordner“ | Hinweise |
| --- | --- | --- |
| SMB | `freigabe/ordner` | SMB 2 oder 3. TalkOps bindet die Freigabe nicht ein. |
| S3 | `bucket/ordner` | AWS oder ein kompatibler Dienst. Außer bei AWS die Endpoint-URL angeben. Selbst betriebene Server brauchen meist Path-Style-URLs. |
| WebDAV | Ordner unterhalb der URL | Nextcloud: `https://SERVER/remote.php/dav/files/BENUTZER` und ein App-Passwort. |
| SFTP | Ordner; `/pfad` beginnt im Wurzelverzeichnis | Passwort oder privater Schlüssel ohne Passphrase. |
| FTP | Ordner | Standard ist explizites TLS. Unverschlüsseltes FTP braucht den Schalter für unsichere Übertragung. |
| Lokal | absoluter Pfad | Unterhalb von `TALKOPS_BACKUP_TARGET_ROOTS` (Standard `/var/lib/talkops/targets`). |

Regeln:

- Entfernte Ziele erhalten nur verschlüsselte Archive. Zuerst
  `TALKOPS_BACKUP_PASSWORD` oder `TALKOPS_BACKUP_PASSWORD_FILE` setzen, siehe
  [Vollständige Compose-Backups](#vollständige-compose-backups).
- Unverschlüsseltes FTP und `http://`-URLs übertragen Daten im Klartext. Dafür
  muss **Übertragung ohne Verschlüsselung erlauben** aktiv sein.
- TalkOps speichert Passwörter und Schlüssel mit `TALKOPS_SECRET_KEY`
  verschlüsselt. API und Audit-Log zeigen sie nicht an. Ein leeres
  Passwortfeld behält den gespeicherten Wert.
- Für jeden TalkOps-Server einen eigenen Ordner verwenden. Die Aufbewahrung
  löscht TalkOps-Archive im Ordner, aber keine anderen Dateien.
- Jeder Sicherungsauftrag braucht einen eigenen Ordner. TalkOps lehnt einen
  zweiten Auftrag mit demselben Speicherziel und Unterordner ab.
- Eine Änderung am Speicherziel gilt für alle Aufträge, die es nutzen. Die
  Spalte **Verwendet von** zeigt diese Aufträge. Ein verwendetes Speicherziel
  lässt sich nicht löschen. Zuerst den Auftrag löschen oder auf ein anderes
  Speicherziel verschieben.

### Ordner-Browser und Verbindungstest

Der Editor des Speicherziels enthält Ordner-Browser und Verbindungstest.
**Durchsuchen** zeigt eine Ordnerebene. Einen Ordner wählen und auf **Diesen
Ordner verwenden** klicken. **Ordner anlegen** erstellt einen neuen Ordner.
S3 zeigt die Buckets und SMB die Freigaben, wenn der Server das erlaubt.
Sonst den Namen des Buckets oder der Freigabe in das Feld „Ordner“ eintragen.

**Verbindung testen** legt den Ordner an, schreibt eine Testdatei, liest sie
zurück und löscht sie. SFTP: Der erste Test oder das erste Durchsuchen
akzeptiert den Host-Schlüssel des Servers und zeigt den SHA-256-Fingerabdruck.
Den Fingerabdruck vor dem Speichern mit dem Server vergleichen
(`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`). Spätere Verbindungen
lehnen einen anderen Host-Schlüssel ab.

**Testen** in der Liste der Speicherziele prüft die gespeicherte Verbindung.
Die Liste zeigt Ergebnis und Zeitpunkt des letzten Tests.

### Zeitplan

Jeder Sicherungsauftrag hat einen eigenen Zeitplan in der Zeitzone des
Containers (`TZ`):

- Wochentage wählen. Ein Auftrag ohne Wochentag läuft nur mit **Jetzt sichern**.
- Uhrzeit des ersten Laufs des Tages festlegen.
- Optional: den Lauf alle 1 bis 12 Stunden bis Mitternacht wiederholen.

Ist ein Auftrag in derselben Minute fällig wie die tägliche lokale Sicherung,
erhält er dieses Archiv. Für andere Läufe schreibt TalkOps ein Archiv in einen
temporären Ordner im Backups-Volume, kopiert es zu allen fälligen Aufträgen und
löscht es danach. Das Backups-Volume braucht Platz für ein Archiv.
**Gesprächsaufzeichnungen einschließen** ist eine Einstellung je Auftrag.

### Intelligente Aufbewahrung

Nach jedem Upload behält TalkOps ein Archiv im Ordner des Auftrags, wenn eine
dieser Regeln es auswählt:

- **Neueste Archive**: die neuesten N Archive (mindestens 1).
- **Täglich**: das neueste Archiv jedes Tages für N Tage.
- **Wöchentlich**: das neueste Archiv jeder Woche (Montag bis Sonntag) für N Wochen.
- **Monatlich**: das neueste Archiv jedes Monats für N Monate.
- **Jährlich**: das neueste Archiv jedes Jahres für N Jahre.

Standard sind 3 neueste Archive, 7 Tage, 4 Wochen und 12 Monate: ein Archiv für
jeden Tag der letzten Woche, danach eines je Woche des letzten Monats, danach
eines je Monat des letzten Jahres. Alle Zeiträume auf 0 setzen, um nur die
neuesten N Archive zu behalten. Upload-Reste (`*.partial`), die älter als sechs
Stunden sind, werden ebenfalls gelöscht.

### NFS, USB-Platten und SMB-Mounts

Der Server-Container hat keine Linux-Capabilities und kann keine Freigaben
einbinden. Der Docker-Daemon des Hosts bindet sie ein; TalkOps nutzt sie als
lokales Ziel:

```sh
# .env: TALKOPS_NFS_HOST=nas.local und TALKOPS_NFS_EXPORT=/export/talkops
docker compose -f docker-compose.yml -f docker-compose.targets.yml up -d
```

Danach ein lokales Speicherziel mit dem Ordner `/var/lib/talkops/targets/nfs`
anlegen.
Der Export muss Schreibzugriff für UID/GID 10001 erlauben. Eine USB-Platte oder
ein anderer Host-Ordner funktioniert genauso mit einem Bind-Mount (siehe
Kommentar in `docker-compose.targets.yml`). In Portainer dieselbe
Volume-Definition in den Stack übernehmen.

### Status und Überwachung

Die Auftragsliste zeigt Status, letzten erfolgreichen Lauf, letzten Fehler und
nächsten Lauf je Auftrag. Ein fehlgeschlagener Auftrag hält die anderen nicht
an. Nach Änderungen an Netzwerk, Passwort oder Speicherziel die Liste prüfen.

### Update von TalkOps 1.7

Das Update verschiebt die Verbindung jedes Sicherungsziels in ein Speicherziel
mit demselben Namen. Der Sicherungsauftrag behält Zeitplan, Aufbewahrung,
letzten Lauf und Archive. Rollen mit `backups.manage` erhalten zusätzlich
`storage.manage`. Es ist nichts zu tun.

## Wiederherstellung von einem Sicherungsziel

Im Betrieb:

1. Beim Sicherungsauftrag auf **Archive** klicken.
2. Beim Archiv auf **Auf diesen Server** klicken. TalkOps lädt das Archiv in die
   lokale Sicherungsliste. Vorhandene Archive werden nie ersetzt.
3. Das Archiv offline wiederherstellen:

   ```sh
   bash recover.sh restore --backup talkops-YYYYMMDD-HHMMSS.tar.gz.age \
     --password-file backup-password -f docker-compose.yml --env-file .env
   ```

   `--backup` verwendet ein Archiv im Backups-Volume. Alle aktiven Overrides
   und Optionen wie unten beschrieben angeben.

Bei der Ersteinrichtung:

1. **Aus Backup wiederherstellen** wählen und das Setup-Token eingeben.
2. **Von einem Sicherungsziel** wählen und die Verbindung des Ziels eingeben.
   TalkOps speichert diese Angaben nicht.
3. Auf **Durchsuchen** klicken, zum Ordner gehen und ein Archiv wählen.
4. Backup-Passwort eingeben und **Backup prüfen** klicken. TalkOps lädt das
   Archiv in das Backups-Volume und zeigt die Prüfung.
5. Den angezeigten Befehl `recover.sh restore --backup …` auf dem Host ausführen.

Nach der Wiederherstellung sind die gespeicherten Ziele der wiederhergestellten
Datenbank wieder aktiv.

Auf einem Host ohne laufenden TalkOps-Server listet `recover.sh fetch` Archive
und lädt sie herunter. Die Verbindung kommt in eine private JSON-Datei; das
Format steht in der [englischen Anleitung](../en/backup.md#restore-from-a-backup-target):

```sh
chmod 600 target.json
bash recover.sh fetch --target-file target.json --list
bash recover.sh fetch --target-file target.json --name talkops-YYYYMMDD-HHMMSS.tar.gz.age --output .
```

Bei SFTP ohne `host_keys` zeigt der Befehl den Fingerabdruck; nach dem Vergleich
`--trust-host-key` ergänzen. Die Zieldatei nach der Wiederherstellung löschen.

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
