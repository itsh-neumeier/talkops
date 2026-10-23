# TalkOps

[Deutsch](README.de.md) · [English](README.md)

TalkOps ist eine selbst gehostete Telefonanlage für Homelabs und kleine
Unternehmen. Sie verbindet SIP-Telefonie, Telefon-Provisionierung,
Browsertelefonie, Voicemail, Fax, Türsprechstellen und lokale Sprachverarbeitung
in einem Docker-Compose-Stack.

[Installation](docs/de/installation.md) · [Deutsches Handbuch](docs/de/index.md) ·
[English handbook](docs/en/index.md) ·
[Releases](https://github.com/itsh-neumeier/talkops/releases) · [Changelog](CHANGELOG.md)

## Funktionen

- **Telefonie:** Provider-Vorlagen, mehrere Trunks und Rufnummern, Nebenstellen,
  Gruppen, Öffnungszeiten, Smart Attendant, Warteschlangen, Parken, Weiterleitung
  und Anrufsperren.
- **Geräte:** Provisionierung verschiedener Hersteller, Herstellereinstellungen,
  Gerätekopien, Funktionstasten und Verzeichnisse; Anleitungen für Yealink und Dahua.
- **Alltag:** Browser-Softphone mit Video, gemeinsames Telefonbuch, Anrufliste,
  Voicemail, Faxversand/-empfang und Mail-to-Fax nur für Anhänge.
- **Medien:** erzeugte, aufgenommene oder hochgeladene Ansagen, Aufzeichnung,
  lokale Piper-Spracherzeugung und whisper.cpp-Transkription. Optionale
  Cloud-Transkription wird getrennt nach Zweck eingerichtet.
- **WLAN:** [Sprachmessung und Roaming-Auswertung](docs/de/wlan-begehung.md) mit
  lesendem UniFi-Zugriff, Live-Messwerten und Aufzeichnungswiedergabe.
- **Administration:** lokale Konten, TOTP, OIDC, LDAP/AD, Rollen, Änderungsprotokoll,
  verschlüsselte Wiederherstellungsbackups, Speicherziele, Diagnose und Metriken.

Provider- und konvertierte Modellvorlagen haben getrennte Prüfstände.
Vorlagenabdeckung belegt keinen Test jedes Telefons oder jeder Firmware.
Prüfe Provider, Geräte, Faxübertragung und Wiederherstellung in deiner Umgebung.
Siehe [Testgrenzen](docs/de/ueberblick.md#unterstützung-und-testgrenzen).

## Voraussetzungen

Nutze einen Linux-Host mit x86_64 oder arm64, Docker Engine ab 24 und Compose-
Plugin. SIP und RTP nutzen Host-Netzwerk. Docker Desktop gehört nicht zum
unterstützten Deployment. Halte die [Ports](docs/de/referenz.md#netzwerkvorgaben)
frei. Mikrofonzugriff im Browser benötigt HTTPS. Transkription benötigt je nach
Modell zusätzlichen Arbeitsspeicher und CPU.

## Schnellstart: neue Installation

Lies bei einem vorhandenen Backup zuerst die [Wiederherstellung](docs/de/datensicherung.md),
bevor du neue Zugangsdaten erzeugst. Diese Befehle wählen Konfiguration und
Images des veröffentlichten Stands 1.8.7. Nutze Git-Release und Image-Version
immer passend zueinander.

```sh
mkdir talkops && cd talkops
curl -fLO https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.7/docker-compose.yml
curl -fL -o .env https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.7/.env.example
sed -i 's/^TALKOPS_VERSION=.*/TALKOPS_VERSION=1.8.7/' .env
sed -i "s/^POSTGRES_PASSWORD=$/POSTGRES_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_SECRET_KEY=$/TALKOPS_SECRET_KEY=$(openssl rand -hex 32)/" .env
sed -i "s/^TALKOPS_ESL_PASSWORD=$/TALKOPS_ESL_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_XMLCURL_PASSWORD=$/TALKOPS_XMLCURL_PASSWORD=$(openssl rand -hex 16)/" .env
chmod 600 .env
docker compose config -q
docker compose up -d
docker compose ps
```

Öffne `http://<host>:8080` und lege im vertrauenswürdigen Netz den ersten
Administrator an. Die [ersten Schritte](docs/de/erste-schritte.md) führen zum
Anruf; [Installation](docs/de/installation.md) erklärt HTTPS und Geheimnisse.
Bewahre `TALKOPS_SECRET_KEY` und ein Wiederherstellungsbackup sicher auf.
Vollständige Backup-Erfassung benötigt die Dateizugriffs-Einrichtung aus der
[Backup-Anleitung](docs/de/datensicherung.md).

`latest` ist der bewegliche stabile Image-Tag. Eine feste Version macht Updates
planbar; siehe [Aktualisierung](docs/de/aktualisierung.md). Ersetze keine
bestehenden Deployment-Dateien durch dieses Neuinstallationsbeispiel.

## Handbuch

| Zielgruppe | Deutsch | English |
| --- | --- | --- |
| Administrator: installieren, einrichten und betreiben | [Administrationspfad](docs/de/index.md#administrationspfad) | [Administrator path](docs/en/index.md#administrator-path) |
| Benutzer: Anrufe, Kontakte und Nachrichten | [Benutzerpfad](docs/de/index.md#benutzerpfad) | [User path](docs/en/index.md#user-path) |
| Alle Themen | [Vollständiges Handbuch](docs/de/index.md) | [Complete handbook](docs/en/index.md) |
| Ein Ablauf funktioniert nicht | [Fehlerbehebung](docs/de/fehlerbehebung.md) | [Troubleshooting](docs/en/troubleshooting.md) |

Aus den Quellen wird auch das GitHub-Wiki erzeugt. Repository-Links funktionieren
bereits vor der Wiki-Veröffentlichung. Maintainer nutzen die
[Wiki-Bau- und Veröffentlichungsanleitung](docs/wiki/README.md).

## Architektur

| Komponente | Aufgabe |
| --- | --- |
| FreeSWITCH 1.10 | SIP- und Medien-Engine. |
| TalkOps-Server, Rust | Routing, FreeSWITCH-XML/ESL-Steuerung, REST-API und statische Weboberfläche. |
| PostgreSQL 17 | Dauerhafte Daten und Warteschlange für Hintergrundaufträge. |
| Media-Worker, Rust | Spracherzeugung und Transkription. |
| SvelteKit, TypeScript und Tailwind | Browseroberfläche. |

Details stehen in [Architektur](docs/architecture.md) und [ADRs](docs/adr/README.md).
Bestehende Entwurfsdokumente sind teilweise deutsch.

## Entwicklung und Support

[CLAUDE.md](CLAUDE.md) enthält Einrichtung und Konventionen, [AGENTS.md](AGENTS.md)
den Codex-Einstieg und [CI](.github/workflows/ci.yml) die genauen Prüfungen.
Parallele Aufgaben nutzen getrennte Worktrees und folgen
[Release-Ablauf](docs/release-workflow.md),
[Entwicklungsprompt](docs/development-chat-prompt.md) und
[Release-Manager-Prompt](docs/release-manager-prompt.md).
Lies vor Beiträgen die [Dokumentationsregeln](docs/documentation-style.md) und
passende [Aufgabennotizen](docs/ai-memory/README.md).

Melde reproduzierbare Probleme über [GitHub Issues](https://github.com/itsh-neumeier/talkops/issues)
mit Version und bereinigten Diagnosen. Beiträge geprüfter
[Provider-Vorlagen](docs/trunk-presets.md) sind willkommen.

## Lizenz

[AGPL-3.0-only](LICENSE). FreeSWITCH nutzt MPL 1.1. Importierte
Provisionierungsquellen behalten eigene Herkunfts- und Lizenzangaben.
