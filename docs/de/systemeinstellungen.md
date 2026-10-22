# Systemeinstellungen und Speicher

[Handbuch](index.md) · [English](../en/system-settings.md)

Öffne **Einstellungen**. Die Navigation zeigt die von deiner Rolle erlaubten
Bereiche. Administratoren sehen alle Bereiche. Ein ausgeblendeter Bereich
bedeutet nicht, dass ein Dienst fehlt. Prüfe vor Rechtevergabe
[Benutzer und Rollen](benutzer-und-rollen.md).

## Einstellungsübersicht

| Bereich | Berechtigung | Zweck und Anleitung |
| --- | --- | --- |
| Telefonie | `settings.manage` | Wahlregeln, Notruf-/Standardnummer, öffentliche IP, SIP-Adressen und Telefonzeitzone; [Trunks und Rufnummern](trunks-und-rufnummern.md). |
| Anrufbehandlung | `settings.manage` | Aufzeichnung nach Richtung, Aufbewahrung, Transkription und Wartemusik; [Aufzeichnung](aufzeichnung.md), [Ansagen](ansagen.md). |
| Transkriptionsverbindungen | `transcription.settings` unter Anrufbehandlung | Engines, Zwecke, Cloud-Verbindungen und API-Schlüssel; [Aufzeichnung](aufzeichnung.md). |
| Voicemail | `voicemail.settings` | Standardsprache, Stimmen, Tempo und Systemtexte; [Voicemail](voicemail.md). |
| E-Mail | `voicemail.settings` | SMTP, Design und Mailtexte; [E-Mail](e-mail.md). |
| Fax | `fax.manage` | Aktivierung, Absendernummern, Transport, Ziele und Mail-Eingang; [Fax](fax.md). |
| Benachrichtigungen | `notifications.manage` | Ereignisregeln und Versandprotokoll; [Benachrichtigungen](benachrichtigungen.md). |
| Anmeldung & Sicherheit | `identity.manage` oder `security.manage` | OIDC/LDAP und SIP-Anmeldeschutz; [Anmeldung](anmeldung.md), [Installation](installation.md). Jeder Teil prüft seine Berechtigung. |
| System | `diagnostics.manage` | Status, Diagnose, Sitzungen und Wartung; [Monitoring](monitoring.md). |
| Backup | `backups.manage` | Erfassung, Verschlüsselung, Aufträge, Archive und Wiederherstellung; [Datensicherung](datensicherung.md). |
| Speicher | `storage.manage` | Wiederverwendbare Speicherverbindungen und Verbindungstests; Anleitung unten. |
| Mein Konto | Eigenes angemeldetes Konto | Passwort und persönliche Kontoeinstellungen; [Anmeldung](anmeldung.md). |

Auch die Einrichtungsseiten liegen unter Einstellungen: Nebenstellen, Benutzer,
Anrufsteuerung, Leitungen, Rufnummern, Telefone, Türsprechstellen und
Änderungsprotokoll. **Meine Telefone** zeigt eigene zugewiesene Nebenstellen.
Die zugehörigen Kapitel stehen im [Index](index.md).

## Einstellung ändern

1. Öffne den passenden Bereich und notiere den bisherigen Wert.
2. Lies Feldhinweis und Anleitung vor einer Änderung gemeinsamer Vorgaben.
3. Ändere eine zusammengehörige Feldgruppe und nutze deren **Speichern**-Schaltfläche.
4. Lade den Bereich neu und prüfe, ob die Änderung erhalten bleibt.
5. Prüfe den betroffenen Ablauf mit einem Testkonto oder Testanruf.

Die Bereiche nutzen getrennte Formulare. Das Speichern der Aufzeichnung
speichert kein anderes offenes Formular. Eine Vorgabe kann viele Nebenstellen
betreffen. Eine Telefonzeitzonenänderung kann einen Provisionierungsabruf
benötigen; die Zeitanzeige im Browser ist davon getrennt.
Aufzeichnung und Cloud-Transkription können den Speicherort von Gesprächsdaten
ändern. Die Medienanleitungen erklären Zwecke und Zugriffe.

## Speicherverbindung erstellen

Speicherziele beschreiben eine Verbindung. Sicherungsaufträge und Audit-Export
verweisen darauf. Das Anlegen eines Ziels erstellt noch keinen Sicherungsplan
und verschiebt keine laufenden Gesprächsmedien auf entfernten Speicher.

1. Öffne **Einstellungen → Speicher** und füge ein Ziel hinzu.
2. Wähle den angebotenen Verbindungstyp und trage Server, Pfad und Zugangsdaten ein.
3. Prüfe den gewünschten Ordner mit Durchsuchen und Verbindungstest.
4. Speichere und weise das Ziel einem Sicherungsauftrag oder Audit-Export zu.
5. Starte den Auftrag und prüfe das erzeugte Archiv oder den Export.

Unterstützte Zieltypen und Regeln für lokale Mounts stehen unter
[Sicherungsziele](datensicherung.md#sicherungsziele). Lasse gespeicherte
Geheimnisse unverändert, wenn du nur andere Felder ändern willst. Prüfe vor dem
Entfernen angezeigte Verwendungen. Ein Verbindungstest prüft keine Wiederherstellung.

## Diagnose und Wartung

Unter **Einstellungen → System** zeigt **Status** den Komponentenzustand,
**Diagnose** eine begrenzte Aufzeichnung, **Sitzungen** registrierte Geräte und
Gespräche und **Wartung** die angebotenen Dienstaktionen.
Plane Neustarts bei ruhender Telefonie. Siehe [Monitoring](monitoring.md),
[Aktualisierung](aktualisierung.md) und [Fehlerbehebung](fehlerbehebung.md).
Gespeicherte Änderungen stehen im [Änderungsprotokoll](audit-log.md).
Entferne Geheimnisse und private Gesprächsdaten aus Supportnachweisen.
