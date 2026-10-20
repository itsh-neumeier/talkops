# Überblick

[Handbuch](index.md) · [English](../en/overview.md)

TalkOps ist eine selbst gehostete Telefonanlage für Homelabs und kleine
Unternehmen. Ziel sind Installationen mit ungefähr 50 Nebenstellen. Die
Kapazität hängt von Gesprächen, Aufzeichnung, Transkription und Host ab.
Das ist keine gemessene Kapazitätsgrenze.

## Komponenten

| Komponente | Aufgabe |
| --- | --- |
| FreeSWITCH | SIP-Registrierung, Gespräche, Audio, Video und Fax-Medien. |
| TalkOps-Server | Weboberfläche, API, Routing, Provisionierung und Zugriffsprüfung. |
| PostgreSQL | Einstellungen, Benutzer, Anrufdatensätze und Warteschlange für Hintergrundaufträge. |
| Media-Worker | Lokale Spracherzeugung und Transkriptionsaufträge. |
| Caddy und coturn, optional | HTTPS und Vermittlung der Browser-Medien. |

Die [Architektur](../architecture.md) und [ADRs](../adr/README.md) erklären
Implementierungsentscheidungen. Das Handbuch beschreibt den Betrieb.
Alte Phasenangaben in Entwurfsdokumenten beschreiben die Entwicklungsgeschichte,
nicht den aktuellen Funktionsstand.

## Einstieg für Administratoren

Du benötigst Zugang zu einem Linux-Docker-Host und das erste Administratorkonto.

1. Lies [Installation](installation.md) oder [Portainer](portainer.md).
2. Führe die [ersten Schritte](erste-schritte.md) aus und prüfe einen internen Anruf.
3. Richte [Trunks und Rufnummern](trunks-und-rufnummern.md) ein und prüfe beide Anrufrichtungen.
4. Verbinde [Telefone](provisionierung.md) oder das [Browser-Softphone](softphone.md).
5. Richte [Rollen](benutzer-und-rollen.md), [Backups](datensicherung.md) und [Monitoring](monitoring.md) ein.

Erwartetes Ergebnis: Benutzer können sich anmelden, mit ihrer zugewiesenen
Identität telefonieren und Nachrichten abrufen. Ein geprüftes Backup für die
Wiederherstellung liegt außerhalb des Hosts.

## Einstieg für Benutzer

Lass dein Benutzerkonto erstellen und eine Nebenstelle zuweisen.
Unter **Einstellungen → Meine Telefone** findest du Geräte, Nicht stören und
Weiterleitung. Nutze je nach Berechtigung [Softphone](softphone.md),
[Telefonbuch](telefonbuch.md), [Anrufliste](anrufliste.md),
[Voicemail](voicemail.md) und [Fax](fax.md).
Funktionen außerhalb deiner Rolle können ausgeblendet sein. Eine leere persönliche
Seite kann bedeuten, dass dir noch keine Nebenstelle oder kein Ziel zugewiesen ist.

## Unterstützung und Testgrenzen

Provider-Vorlagen und konvertierte Provisionierungsvorlagen liefern
Konfigurationen. Sie belegen keine Tests für jeden Provider, jedes Gerätemodell
oder jede Firmware. Beachte den angezeigten Prüfstatus und die Geräte- oder
Provider-Anleitung. Spectralink- und Grandstream-Wave-Profile bleiben gesperrt,
wenn ihre Quellvorlagen leer sind. Firmware-Dateien sind nicht enthalten.

Automatisierte Tests prüfen Anwendung und simuliertes SIP-/Browser-Verhalten.
Prüfe reale Telefone, Provider, Faxübertragung und Wiederherstellung auf deiner
Installation. Beginne bei Problemen mit der [Fehlerbehebung](fehlerbehebung.md).

Der [Handbuchindex](index.md) enthält alle Themen. Das [Glossar](referenz.md)
erklärt die Begriffe der Anleitungen.
