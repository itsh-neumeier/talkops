# Anmeldung: Zwei-Faktor, Single Sign-on, LDAP/AD

TalkOps kennt drei Arten von Konten:

| Art | Anmeldung | Zweiter Faktor |
|---|---|---|
| lokal | Benutzername + Passwort in TalkOps | TOTP (optional) |
| LDAP / Active Directory | Benutzername + Verzeichnis-Passwort | TOTP (optional) |
| Single Sign-on (OIDC) | über den Identitätsanbieter | beim Anbieter |

Lokale Konten funktionieren immer weiter – richten Sie mindestens einen
lokalen Admin ein, falls der Anbieter oder das Verzeichnis ausfällt.

## Zwei-Faktor-Anmeldung (TOTP)

**Einstellungen → Zwei-Faktor-Anmeldung → einrichten:** QR-Code mit einer
Authenticator-App scannen (z. B. Aegis, Google Authenticator, 1Password,
Bitwarden), den angezeigten Code eingeben, fertig. Danach zeigt TalkOps
**zehn Wiederherstellungscodes** – sicher aufbewahren; jeder ersetzt einmal
einen App-Code.

- Codes gelten 30 Sekunden (±30 s Toleranz) und nur einmal.
- Nach fünf falschen Codes muss das Passwort erneut eingegeben werden.
- Handy verloren: ein Admin setzt unter **Einstellungen → Benutzer → 2FA zurücksetzen** die
  Zwei-Faktor-Anmeldung zurück (protokolliert, die Person wird abgemeldet).

## Single Sign-on mit OpenID Connect

Unterstützt werden alle OIDC-Anbieter mit Discovery
(`/.well-known/openid-configuration`), z. B. Keycloak, Authentik, Authelia,
Zitadel, Microsoft Entra ID, Google.

1. Beim Anbieter einen Client anlegen:
   - Typ: *vertraulich* (mit Secret) oder *öffentlich* (nur PKCE)
   - Redirect-URI: `https://<ihr-talkops>/api/v1/auth/oidc/callback` –
     TalkOps zeigt die genaue URI in den Einstellungen an
   - Scopes: `openid profile email` (plus ein Gruppen-Claim, siehe unten)
2. **Einstellungen → Single Sign-on und Verzeichnis** (Admin): Issuer-URL
   (z. B. `https://auth.example.com/realms/home`), Client-ID, Secret.
3. Auf der Anmeldeseite erscheint die Schaltfläche *Mit Single Sign-on
   anmelden* (Text anpassbar).

Hinter einem Reverse-Proxy unter **Erweitert** die öffentliche URL
eintragen, damit die Redirect-URI stimmt.

**Gruppen:** Der Anbieter muss die Gruppen im ID-Token liefern (Standard-Claim
`groups`; Keycloak: Mapper „Group Membership“, Authentik: Scope `profile`
enthält `groups`, Entra ID: „Gruppenansprüche“ – dort kommen IDs statt
Namen).

## LDAP / Active Directory

**Einstellungen → Single Sign-on und Verzeichnis → LDAP / Active Directory:**

| Feld | OpenLDAP (Beispiel) | Active Directory (Beispiel) |
|---|---|---|
| URL | `ldaps://ldap.example.com` | `ldaps://dc1.example.com:636` |
| Dienstkonto | `cn=talkops,ou=services,dc=example,dc=com` | `CN=svc-talkops,OU=Service,DC=example,DC=com` |
| Basis-DN | `dc=example,dc=com` | `DC=example,DC=com` |
| Benutzerfilter | `(&(objectClass=inetOrgPerson)(uid={username}))` | `(&(objectClass=user)(sAMAccountName={username}))` |
| Attribute | `uid`, `cn`, `mail`, `memberOf` | `sAMAccountName`, `displayName`, `mail`, `memberOf` |

TalkOps sucht den Benutzer mit dem Dienstkonto (das Leserechte braucht) und
prüft das Passwort, indem es sich als dieser Benutzer anmeldet. Leere
Passwörter werden abgelehnt. **Verbindung testen** zeigt für einen
Testbenutzer DN, Gruppen und die Rolle, die er bekäme.

- OpenLDAP braucht das `memberof`-Overlay für `memberOf`.
- `ldaps://` und StartTLS prüfen das Zertifikat gegen die System-CAs. Für
  eine eigene CA die Datei in den Container einbinden und `SSL_CERT_FILE`
  setzen.
- Stündlicher Abgleich: Wer aus dem Verzeichnis oder der erlaubten Gruppe
  entfernt wurde, wird deaktiviert und abgemeldet; Rollenwechsel greifen
  sofort. Wieder freischalten: Admin unter **Einstellungen → Benutzer**.

## Rollen aus Gruppen

| Feld | Wirkung |
|---|---|
| Administrator | Mitglieder werden Admin |
| Operator | Mitglieder werden Operator |
| Benutzer | nur Mitglieder dürfen sich anmelden (leer = alle aus dem Verzeichnis) |

OIDC: Gruppennamen aus dem Claim. LDAP: Gruppen-DN oder nur der CN
(`pbx-admins` passt zu `CN=pbx-admins,OU=Groups,…`). Groß/Kleinschreibung
spielt keine Rolle. Die Rolle wird bei jeder Anmeldung aus dem Verzeichnis
übernommen – in TalkOps geänderte Rollen dieser Konten gelten nur bis dahin.

**Sicherheit:** Ein Verzeichnis- oder SSO-Login übernimmt nie ein lokales
Konto mit demselben Namen; dann erscheint eine Fehlermeldung und ein Admin
muss das lokale Konto umbenennen oder löschen.
