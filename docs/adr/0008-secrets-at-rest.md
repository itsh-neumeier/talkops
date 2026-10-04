# 0008 – Verschlüsselung von SIP- und Trunk-Zugangsdaten

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

„Keine Klartext-Passwörter“ gilt für die gesamte Anlage. Aber:

- **Web-Logins** brauchen nur eine Verifikation → Hash genügt.
- **SIP-Passwörter von Nebenstellen** müssen im Klartext an Yealink-Telefone
  provisioniert und per QR-Code an Apps übergeben werden.
- **Trunk-Zugangsdaten** (z. B. LEONET: eigene Zugangsdaten je Rufnummer)
  braucht FreeSWITCH im Klartext, um sich beim Provider zu registrieren.

## Optionen

1. Klartext in der DB – ausgeschlossen.
2. Nur Hashes (`a1-hash` für SIP-Digest) – verhindert Provisioning und
   Trunk-Registrierung.
3. **Symmetrische, authentifizierte Verschlüsselung** mit einem
   Anwendungsschlüssel außerhalb der Datenbank.

## Entscheidung

- Web-Passwörter: **argon2id** (`argon2`-Crate, OWASP-Parameter).
- SIP-/Trunk-/LDAP-Bind-Secrets: **XChaCha20-Poly1305** (`chacha20poly1305`-Crate),
  zufällige 192-Bit-Nonce je Wert, gespeichert als `v1:<nonce>:<ciphertext>`
  (Base64). Der 256-Bit-Schlüssel kommt aus `TALKOPS_SECRET_KEY`
  (64 Hex-Zeichen), niemals aus der DB. Das Versionspräfix erlaubt spätere
  Schlüsselrotation.
- Für die SIP-Authentifizierung von Nebenstellen liefert das Directory
  bevorzugt `a1-hash` statt des Klartext-Passworts an FreeSWITCH.
- Secrets werden nie geloggt und über die API nur beim Anlegen bzw. auf
  explizite Anforderung (Provisioning-QR) herausgegeben.

## Konsequenzen

- Ohne `TALKOPS_SECRET_KEY` sind gespeicherte Zugangsdaten nicht lesbar →
  Schlüssel gehört ins Backup-Konzept (Doku, Phase 8).
- DB-Dumps allein enthalten keine verwertbaren SIP-Zugangsdaten.
- Implementierung in Phase 1 (`talkops_core::crypto`) mit Tests für
  Roundtrip, Manipulationserkennung und falschen Schlüssel.
