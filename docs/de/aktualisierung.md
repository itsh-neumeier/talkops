# Aktualisierung

[Handbuch](index.md) · [English](../en/updates.md)

Ein Update kann FreeSWITCH neu starten und laufende Gespräche beenden.
Nutze ein Wartungsfenster. Die Anwendung führt beim Start Datenbankmigrationen
aus. Schemaänderungen können ein einfaches Zurücksetzen des Images verhindern.
Lies vor der Installation die Hinweise des Ziel-Releases.

## Voraussetzungen

Du benötigst Host- oder Portainer-Administration. Backup-Einstellungen benötigen
`backups.manage`, Systemdiagnose `diagnostics.manage`.
Nutze veröffentlichte [Releases](https://github.com/itsh-neumeier/talkops/releases)
und [Changelog](../../CHANGELOG.md), keinen ungemergten Feature-Branch.

1. Notiere Image-Tag, Compose-Dateien, Projektnamen und aktive Profile.
2. Erstelle ein vollständiges verschlüsseltes [Backup](datensicherung.md) außerhalb des Hosts.
3. Prüfe Verfügbarkeit von Archiv, Passwort und `TALKOPS_SECRET_KEY`.
4. Lies Migrations- und Mount-Hinweise aller übersprungenen Releases.
5. Prüfe vor dem Stoppen unter **Einstellungen → System → Sitzungen** aktive Gespräche.

## Docker Compose aktualisieren

Das Beispiel wählt den bereits veröffentlichten Stand 1.8.18. Ersetze für spätere
Releases Git-Tag und Image-Version gemeinsam nach Prüfung der Hinweise.
Nutze dein bestehendes Deployment-Verzeichnis. Erhalte `.env` und alle Overrides.

1. Lade die Zielkonfiguration in ein separates Prüfverzeichnis:

   ```sh
   mkdir -p update-review
   curl -fL -o update-review/docker-compose.yml https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.18/docker-compose.yml
   curl -fL -o update-review/env.example https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.18/.env.example
   diff -u docker-compose.yml update-review/docker-compose.yml
   ```

2. Übernimm notwendige Änderungen in deine Deployment-Dateien. Erhalte eigene
   Mounts, Netzwerte, Geheimnisse und Datenvolumes. Prüfe genutzte Backup-/Ziel-Overrides
   aus demselben Release. Ersetze deine `.env` nicht durch die Beispieldatei.
3. Setze `TALKOPS_VERSION=1.8.18` in `.env`. `latest` folgt dem beweglichen
   stabilen Image-Tag; eine feste Version legt den gewünschten Stand ausdrücklich fest.
4. Prüfe, lade und starte mit **allen** bisherigen Compose-Optionen:

   ```sh
   docker compose config -q
   docker compose pull
   docker compose up -d
   docker compose ps
   ```

Ergänze etwa `-f docker-compose.yml -f docker-compose.backup.yml` bei jedem
Befehl, wenn du dieses Override nutzt. Erhalte `--project-name`, `--env-file`
und Profile. Ein anderer Projektname kann neue leere Volumes anlegen.

Für 1.8.4/1.8.5 benötigen Server, Worker und FreeSWITCH dasselbe beschreibbare
Fax-Volume mit Zugriff für UID/GID 10001. Ein Image-Update repariert keine alten
Stack-Mounts. Führe danach den administrativen Speichertest unter [Fax](fax.md) aus.

## Portainer aktualisieren

Prüfe Ziel-Stack und neue Umgebungsvariablen vor **Pull and redeploy**.
Wähle bei Repository-Stacks die gewünschte Git-Release-Referenz und aktiviere
Image-Downloads. Setze `TALKOPS_VERSION` auf die passende Image-Version.
Erhalte Umgebungswerte, Stack-Identität und Volumenzuordnungen. Prüfe danach
Stack-Erfassung und Backup-Passwort. Siehe [Portainer](portainer.md).

## Ergebnis prüfen

1. Prüfe Containerzustand sowie Start- und Migrationslogs.
2. Prüfe **Einstellungen → System → Status** und melde dich als normaler Benutzer an.
3. Prüfe interne, eingehende und ausgehende Anrufe mit beidseitigem Ton.
4. Prüfe Browsertelefon, Voicemail und genutzte Fax- oder Türfunktionen.
5. Erstelle und prüfe ein neues Backup samt entfernten Kopieraufträgen.

Erhalte bei Startfehlern Logs und Volumes. Nutze die [Fehlerbehebung](fehlerbehebung.md).
Wechsle Images nicht wiederholt über ein bereits migriertes Schema hinweg.
Stelle bei notwendiger Wiederherstellung kompatibles Backup, Konfiguration und
Images nach der Offline-[Wiederherstellungsanleitung](datensicherung.md) her.
Eine Wiederherstellung ersetzt Daten seit dem Backup.
