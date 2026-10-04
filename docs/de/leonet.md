# LEONET einrichten

LEONET-Anschlüsse registrieren **jede Rufnummer einzeln** mit eigenen
Zugangsdaten am Registrar `sip.leovoice.online`. TalkOps legt dafür pro
Rufnummer eine eigene Registrierung (FreeSWITCH-Gateway) an.

> Status der Vorlage: **ungetestet** – die Parameter stammen aus der
> Referenzkonfiguration des Projekts. Bitte nach dem ersten erfolgreichen Test
> Rückmeldung geben (Issue/PR), dann wird die Vorlage als „geprüft“ markiert.

## Was du brauchst

- Rufnummer(n) mit Ortsvorwahl, z. B. `08331 123456`
- das SIP-Passwort je Rufnummer aus dem LEONET-Kundenportal bzw. den
  Zugangsdaten

Der Benutzername wird automatisch gebildet: `leo` + Rufnummer international
ohne `+`, also `leo49` + Vorwahl ohne 0 + Rufnummer, z. B. `leo498331123456`.

## Schritte

1. **Einstellungen → Wählregeln:** Ortsvorwahl ohne 0 eintragen (z. B. `8331`),
   damit Ortsgespräche ohne Vorwahl funktionieren.
2. **Trunks → Neuer Trunk:** Anbieter „LEONET – SIP-Anschluss“ wählen, Namen
   vergeben, hinzufügen.
3. Im Trunk **Rufnummer hinzufügen:** Rufnummer im internationalen Format
   (`+498331123456`), SIP-Passwort und die Nebenstelle eintragen, bei der die
   Nummer klingeln soll. Für jede weitere Rufnummer wiederholen.
4. Nach wenigen Sekunden zeigt die Registrierung **registriert**. Andernfalls
   steht dort der Fehler (z. B. `403` = Passwort falsch).
5. **Einstellungen → Standardrufnummer:** die Hauptnummer wählen. Sie wird für
   abgehende Gespräche und Notrufe genutzt, wenn eine Nebenstelle keine eigene
   ausgehende Rufnummer hat.

## Technische Details

| Parameter | Wert |
|---|---|
| Registrar / Realm | `sip.leovoice.online` |
| Transport | UDP 5060 |
| Benutzername | `leo{E.164 ohne +}` |
| Gewählte Rufnummer | national (`0831…`, `0049…` für Ausland) |
| Absenderrufnummer | international ohne `+` |
| Keepalive | SIP OPTIONS alle 30 s |

Alle Werte lassen sich im Trunk unter **Bearbeiten → Erweiterte Einstellungen**
überschreiben, falls LEONET etwas ändert.

## Fehlersuche

- **`403 Forbidden`**: Passwort oder Benutzername falsch.
- **`TRYING`/`FAIL_WAIT`**: Registrar nicht erreichbar – Firewall (UDP 5080
  ausgehend vom TalkOps-Host, RTP 16384–16999) prüfen.
- **Kein Ton / einseitig**: öffentliche IP unter *Einstellungen → Öffentliche IP
  für Trunks* eintragen (oder `stun:stun.l.google.com:19302`).
