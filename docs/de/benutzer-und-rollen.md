# Benutzer und Rollen

Jeder Benutzer hat genau eine **Rolle**. Die Rolle bestimmt die
Berechtigungen. Unter **Einstellungen → Benutzer → Rollen** siehst und
änderst du die Rollen.

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

Die vollständige Liste mit den Schlüsseln steht in der
[englischen Dokumentation](../en/users-and-roles.md#permissions).

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
