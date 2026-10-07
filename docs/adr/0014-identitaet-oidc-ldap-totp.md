# 0014 – Identität: lokale Konten mit TOTP, OIDC, LDAP/AD

- Status: Angenommen
- Datum: 2026-10-07

## Kontext

Phase 7 verlangt Anmeldung über LDAP/AD und OIDC sowie einen zweiten Faktor.
Zielgruppen: Familien-Homelab (meist lokale Konten, evtl. Authentik/Keycloak)
und kleine Firmen (oft Active Directory oder Entra ID).

## Optionen

1. **Eigene Implementierung** mit bewährten Bausteinen: `jsonwebtoken`
   (Signaturprüfung, rust_crypto), `ldap3`, TOTP nach RFC 6238 mit
   `hmac`/`sha1`.
2. **`openidconnect`-Crate** – vollständiger, zieht aber eine zweite
   reqwest-/TLS-Kette mit aws-lc nach (Build-Abhängigkeit cmake/nasm,
   ARM-Builds langsamer).
3. **Externer Auth-Proxy** (oauth2-proxy, Authelia vor TalkOps) – löst kein
   LDAP-Passwort am Telefon/Softphone und keine Rollen.

## Entscheidung

Option 1.

- **OIDC:** Authorization Code Flow mit PKCE (S256), `state` und `nonce`
  (serverseitig, 10 Minuten, einmalig), ID-Token-Prüfung gegen JWKS
  (nur asymmetrische Verfahren; Issuer, Audience, Ablauf, Nonce),
  Discovery/JWKS je Issuer eine Stunde gecacht, bei unbekanntem `kid` einmal
  neu geladen. Konten werden über `iss|sub` zugeordnet, nie über den Namen.
- **LDAP:** Suche mit Dienstkonto, Prüfung per Bind als Benutzer; leere
  Passwörter werden vorher abgewiesen (sonst unauthentifizierter Bind).
  Filterwerte RFC-4515-escaped. Stündlicher Abgleich deaktiviert entfernte
  Konten.
- **Rollen** kommen bei OIDC/LDAP aus Gruppen und werden bei jeder Anmeldung
  übernommen. Ein externes Konto übernimmt nie ein lokales gleichen Namens.
- **TOTP** für lokale und LDAP-Konten: Geheimnis verschlüsselt (ADR 0008),
  Schutz gegen Wiederverwendung je Zeitschritt, zehn gehashte
  Wiederherstellungscodes, Login-Challenge mit Versuchslimit. OIDC-Konten
  nutzen den zweiten Faktor ihres Anbieters.
- Lokale Konten bleiben immer aktiv (Notfallzugang).

## Konsequenzen

- Keine zusätzlichen Dienste; Tests laufen gegen einen nachgebauten
  OIDC-Anbieter und echtes OpenLDAP (in CI gestartet).
- Kein OIDC-Logout beim Anbieter (Front-/Back-Channel); TalkOps-Sitzungen
  enden nach 12 h Inaktivität oder beim Abmelden.
- WebAuthn/Passkeys sind nicht Teil dieser Phase.
