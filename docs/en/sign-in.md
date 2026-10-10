# Sign-in: two-factor, single sign-on, LDAP/AD

TalkOps has three kinds of accounts:

| Kind | Login | Second factor |
|---|---|---|
| local | user name + password in TalkOps | TOTP (optional) |
| LDAP / Active Directory | user name + directory password | TOTP (optional) |
| single sign-on (OIDC) | through the identity provider | at the provider |

Local accounts always keep working – keep at least one local admin in case
the provider or the directory is down.

## Two-factor login (TOTP)

**Settings → Two-factor login → set up:** scan the QR code with an
authenticator app (e.g. Aegis, Google Authenticator, 1Password, Bitwarden)
and enter the code it shows. TalkOps then shows **ten recovery codes** –
store them safely; each replaces an app code once.

- Codes are valid for 30 seconds (±30 s tolerance) and only once.
- After five wrong codes the password has to be entered again.
- Lost phone: an admin resets two-factor login under **Settings → Users → Reset 2FA**
  (audited; the person is logged out).

## Single sign-on with OpenID Connect

Every OIDC provider with discovery (`/.well-known/openid-configuration`)
works, e.g. Keycloak, Authentik, Authelia, Zitadel, Microsoft Entra ID,
Google.

1. Create a client at the provider:
   - type: *confidential* (with secret) or *public* (PKCE only)
   - redirect URI: `https://<your-talkops>/api/v1/auth/oidc/callback` –
     TalkOps shows the exact URI in the settings
   - scopes: `openid profile email` (plus a groups claim, see below)
2. **Settings → Single sign-on and directory** (admin): issuer URL (e.g.
   `https://auth.example.com/realms/home`), client ID, secret.
3. The login page shows the button *Log in with single sign-on* (text can
   be changed).

Behind a reverse proxy, enter the public URL under **Advanced** so the
redirect URI is right.

**Groups:** the provider must put the groups into the ID token (default
claim `groups`; Keycloak: mapper "Group Membership", Authentik: scope
`profile` contains `groups`, Entra ID: "groups claim" – it sends IDs instead
of names).

## LDAP / Active Directory

**Settings → Single sign-on and directory → LDAP / Active Directory:**

| Field | OpenLDAP (example) | Active Directory (example) |
|---|---|---|
| URL | `ldaps://ldap.example.com` | `ldaps://dc1.example.com:636` |
| Service account | `cn=talkops,ou=services,dc=example,dc=com` | `CN=svc-talkops,OU=Service,DC=example,DC=com` |
| Base DN | `dc=example,dc=com` | `DC=example,DC=com` |
| User filter | `(&(objectClass=inetOrgPerson)(uid={username}))` | `(&(objectClass=user)(sAMAccountName={username}))` |
| Attributes | `uid`, `cn`, `mail`, `memberOf` | `sAMAccountName`, `displayName`, `mail`, `memberOf` |

TalkOps looks the user up with the service account (which needs read
access) and checks the password by binding as that user. Empty passwords
are rejected. **Test connection** shows DN, groups and the resulting role
of a test user.

- OpenLDAP needs the `memberof` overlay for `memberOf`.
- `ldaps://` and StartTLS verify the certificate against the system CAs. For
  your own CA, mount the file into the container and set `SSL_CERT_FILE`.
- Hourly sync: accounts removed from the directory or the allowed group are
  disabled and logged out; role changes apply right away. To allow an
  account again, an admin enables it under **Settings → Users**.

## Roles from groups

| Field | Effect |
|---|---|
| Administrator | members become admins |
| Operator | members become operators |
| User | only members may log in (empty = everyone in the directory) |

OIDC: group names from the claim. LDAP: group DN or just the CN
(`pbx-admins` matches `CN=pbx-admins,OU=Groups,…`). Case does not matter.
The role is taken from the directory at every login – roles changed in
TalkOps for these accounts only last until then.

**Security:** a directory or SSO login never takes over a local account with
the same name; the login then fails with a message and an admin has to
rename or delete the local account.
