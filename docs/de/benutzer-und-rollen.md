# Benutzer und Rollen

[Handbuch](index.md) · [English](../en/users-and-roles.md)

## Voraussetzungen

Du benötigst: Ein Administratorkonto für Rollenänderungen; Benutzerverwaltung benötigt users.manage.

Das Recht **Whitelist verwalten** (`blocking.allowlist`) erlaubt das Lesen,
Hinzufügen und Entfernen exakter Rufnummern-Ausnahmen für den gesamten
Mandanten. Standardmäßig hat es nur der Administrator. Es ist unabhängig
von **Rufnummern sperren** und **Spam-Schutz einstellen**. Es erweitert
nicht die Sichtbarkeit von Anrufen. Siehe
[lokale Whitelist](anrufsteuerung.md#lokale-whitelist-für-den-spam-schutz).

Jeder Benutzer hat genau eine **Rolle**. Die Rolle bestimmt die
Berechtigungen. Unter **Einstellungen → Benutzer → Rollen** siehst und
änderst du die Rollen.

## Seitenpanel eines Benutzers

Einen Benutzernamen in der Liste auswählen, um das Seitenpanel zu öffnen:
**Übersicht** (Rolle, Anmeldung über, letzte Anmeldung, 2FA), **Nebenstellen**
des Benutzers und **Einstellungen** mit **Bearbeiten**, **Passwort setzen**,
**2FA zurücksetzen** und **Löschen**. Die Aktionen nutzen dieselben Dialoge
wie die Liste.

## Eingebaute Rollen

| Rolle | Berechtigungen nach dem Update |
| --- | --- |
| Administrator | Alle Berechtigungen. Die Rolle ist nicht änderbar. |
| Operator | Laufende Gespräche und Statistik, alle Anrufe in der Anrufliste, Live-Transkript eigener Gespräche, Telefonbuch, Rufnummernsperren, Zeitsteuerung umschalten, Lesezugriff auf Nebenstellen, Benutzer, Anrufsteuerung, Leitungen, Rufnummern und Telefone. |
| Standardbenutzer | Keine zusätzlichen Berechtigungen. |

Diese Vorgaben entsprechen den festen Rollen früherer Versionen. Ein Update
ändert keine Zugriffe.

Eigene Daten kann jeder Benutzer ohne Berechtigung nutzen: eigene
Nebenstellen, eigene Anrufe in der Anrufliste, eigene Aufnahmen und
Transkripte, eigene Voicemail, eigene Telefone und das Softphone mit den
eigenen Nebenstellen.

## Berechtigungen ändern

1. Öffne **Einstellungen → Benutzer** und wähle den Tab **Rollen**.
2. Setze oder entferne die Haken. Eine Spalte ist eine Rolle, eine Zeile
   eine Berechtigung. Der Haken in der Bereichszeile setzt alle
   Berechtigungen des Bereichs.
3. Eine gelbe Leiste zeigt die Zahl der geänderten Rollen. Klicke
   **Speichern**.

Änderungen gelten sofort, auch in offenen Sitzungen. Die Spalte
Administrator ist gesperrt. Mit **Kritisch** markierte Berechtigungen geben
Zugriff auf Geheimnisse, das System oder weitere Rechte (z. B.
„Benutzer verwalten“, „Datensicherung“).

## Eigene Rollen

1. Klicke **Neue Rolle**.
2. Gib einen Namen und bei Bedarf eine Beschreibung ein.
3. Optional: Wähle unter **Berechtigungen übernehmen von** eine Rolle.
4. Optional: Gib eine **Verzeichnisgruppe** ein (siehe unten).
5. Klicke **Speichern** und setze danach die Haken in der Tabelle.

Die Rolle wählst du im Benutzerdialog (Tab **Benutzer** → **Bearbeiten** →
**Rolle**). Eine eigene Rolle ohne Benutzer kannst du löschen.

## Regeln für Benutzerverwalter

Ein Benutzer mit der Berechtigung **Benutzer verwalten**, aber ohne die
Rolle Administrator:

- kann die Rolle Administrator nicht vergeben,
- kann nur Rollen vergeben, deren Berechtigungen er selbst hat,
- kann Benutzer mit mehr Berechtigungen nicht ändern, löschen oder ihr
  Passwort setzen.

Nur Administratoren legen Rollen an, ändern und löschen sie.

## Verzeichnis-Benutzer (LDAP, OIDC)

Die Gruppenfelder der [Anmeldung](anmeldung.md) wählen die Rollen
Administrator, Operator und Benutzer. Zusätzlich kann eine eigene Rolle eine
**Verzeichnisgruppe** haben. Bei jeder Anmeldung und beim stündlichen
LDAP-Abgleich erhält ein Mitglied dieser Gruppe die eigene Rolle.
Administratoren bleiben Administratoren. Die Benutzergruppe legt weiterhin
fest, wer sich anmelden darf.

## Berechtigungen

| Bereich | Berechtigung | Schlüssel |
| --- | --- | --- |
| Anrufe | Laufende Gespräche und Statistik sehen | `calls.active` |
| Anrufe | Alle Anrufdatensätze sehen | `calls.view_all` |
| Anrufe | Live-Transkript eigener Gespräche | `calls.live_transcript` |
| Anrufe | Live-Transkript aller Gespräche | `calls.live_transcript_all` |
| Aufzeichnungen | Alle Aufzeichnungen/Transkripte, protokolliert | `recordings.access_all` |
| Aufzeichnungen | Aufzeichnungen löschen | `recordings.delete` |
| Aufzeichnungen | Erneut transkribieren | `recordings.retranscribe` |
| Aufzeichnungen | Eigene Transkriptionsaufträge | `transcriptions.view` |
| Aufzeichnungen | Alle Transkriptionsaufträge und Worker-Ressourcen | `transcriptions.manage` |
| Voicemail | Alle Mailboxen | `voicemail.access_all` |
| Voicemail | Voicemail- und Mail-Einstellungen | `voicemail.settings` |
| Telefonbuch | Kontakte und Bereiche bearbeiten/importieren | `phonebook.edit` |
| Nebenstellen | Nebenstellen sehen | `extensions.view` |
| Nebenstellen | Nebenstellen verwalten | `extensions.manage` |
| Nebenstellen | Softphone mit beliebiger Nebenstelle | `softphone.any_extension` |
| Benutzer | Benutzer sehen | `users.view` |
| Benutzer | Benutzer verwalten, kritisch | `users.manage` |
| Anrufsteuerung | Anrufsteuerung sehen | `routing.view` |
| Anrufsteuerung | Anrufsteuerung verwalten | `routing.manage` |
| Anrufsteuerung | Zeitsteuerung umschalten | `routing.override` |
| Anrufsteuerung | Nummern sperren | `blocking.manage` |
| Anrufsteuerung | Spam-Schutz einstellen | `blocking.settings` |
| Anrufsteuerung | Exakte lokale Whitelist-Ausnahmen verwalten | `blocking.allowlist` |
| Anrufsteuerung | Wahlberechtigungen verwalten, kritisch | `dialing.manage` |
| Trunks | Trunks und Nummern sehen | `trunks.view` |
| Trunks | Trunks und Nummern verwalten, kritisch | `trunks.manage` |
| Telefone | Telefone sehen | `phones.view` |
| Telefone | Telefone verwalten | `phones.manage` |
| Türen | Türsprechstellen verwalten | `doors.manage` |
| System | Telefonieeinstellungen | `settings.manage` |
| System | Transkriptionseinstellungen, kritisch | `transcription.settings` |
| System | SIP-Anmeldeschutz, kritisch | `security.manage` |
| System | LDAP und SSO, kritisch | `identity.manage` |
| System | Backups, kritisch | `backups.manage` |
| System | Speicherziele, kritisch | `storage.manage` |
| System | Diagnose und Neustart, kritisch | `diagnostics.manage` |
| System | Änderungsprotokoll lesen | `audit.view` |
| System | Audit-Aufbewahrung und Export, kritisch | `audit.manage` |

Die zusätzlichen Faxrechte stehen in [Fax](fax.md). Sie gelten unabhängig von
den Rechten für Telefonie und Anrufdatensätze.

Filter, Downloads, Aufbewahrung und automatischen Export des
Änderungsprotokolls beschreibt [Änderungsprotokoll](audit-log.md).

## API

- `GET /api/v1/roles`: Katalog der Berechtigungen und Rollen (braucht
  `users.view` oder `users.manage`).
- `POST /api/v1/roles`, `PUT /api/v1/roles/{id}`, `DELETE /api/v1/roles/{id}`:
  nur Administratoren. Jede Änderung schreibt einen Audit-Eintrag.
- `POST /api/v1/users` und `PUT /api/v1/users/{id}` akzeptieren `role_id`
  (jede Rolle, eingebaut oder eigen). Ohne `role_id` wählt `role` eine
  eingebaute Rolle.
- `GET /api/v1/auth/me` liefert `permissions` (für Administratoren alle
  Schlüssel).

## Ergebnis prüfen

Melde dich als betroffener Testbenutzer an und prüfe erlaubte und gesperrte Aktionen.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.
