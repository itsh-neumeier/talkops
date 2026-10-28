# Betrieb mit Portainer

[Handbuch](index.md) · [English](../en/portainer.md)

## Voraussetzungen

Du benötigst: Portainer-Stack-Administration und einen Linux-Docker-Endpoint.

Für vollständige Wiederherstellungsbackups TalkOps unter **Einstellungen → Backup** mit der Portainer-API verbinden. TalkOps liest dann vor jeder Sicherung Stack-Datei und Stack-Umgebung. Das Backup-Passwort separat bereitstellen. Siehe [Datensicherung und Wiederherstellung](datensicherung.md#portainer) für Verbindung, manuellen Import und Wiederherstellung auf einem neuen Host.

`docker-compose.yml` verwendet ausschließlich fertige Images von GHCR und ist
daher direkt als Portainer-Stack nutzbar.

1. **Stacks → Add stack**, Name `talkops`.
2. Build-Methode **Repository** (Repository-URL
   `https://github.com/itsh-neumeier/talkops`, Compose-Pfad `docker-compose.yml`)
   mit Release-Referenz `refs/tags/v1.8.14`, oder **Web editor** und den Inhalt
   von `docker-compose.yml` aus diesem Release einfügen.
3. Unter **Environment variables** → **Advanced mode** den Inhalt von
   `.env.example` einfügen und die Pflichtwerte setzen (`POSTGRES_PASSWORD`,
   `TALKOPS_SECRET_KEY`, `TALKOPS_ESL_PASSWORD`, `TALKOPS_XMLCURL_PASSWORD`).
   Zufallswerte z. B. mit `openssl rand -hex 32` erzeugen.
   Setze `TALKOPS_VERSION=1.8.14` passend zum Konfigurationsrelease. Erhalte bei
   Updates bestehende Geheimnisse. Spätere Versionen erklärt [Aktualisierung](aktualisierung.md).
4. **Deploy the stack**. Nach ca. einer Minute sollten alle Container
   „healthy“ sein.

Hinweise:

- Der Portainer-Endpoint muss ein Linux-Docker-Host sein (Host-Networking).
- Updates: Stack öffnen → **Pull and redeploy** (bei Repository-Stacks
  „Re-pull image“ aktivieren).
- `TALKOPS_SECRET_KEY` zusätzlich außerhalb von Portainer sichern.

## Ergebnis prüfen

Prüfe Image-Versionen, Stack-Mounts und Erfassung der Backup-Konfiguration.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.
