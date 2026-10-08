# Erste Schritte

Nach der [Installation](installation.md) ist TalkOps unter `http://<host>:8080`
erreichbar. Das Dashboard zeigt Admins eine Liste *Erste Schritte*, die diese
Schritte abhakt, sobald sie erledigt sind.

1. **Administrator anlegen.** Beim ersten Aufruf fragt TalkOps nach Benutzername
   und Passwort (mind. 10 Zeichen) für das erste Admin-Konto.
2. **Wählregeln.** *Einstellungen*: Ländervorwahl (Standard 49) und Ortsvorwahl
   ohne 0 eintragen. Notrufnummern (110, 112) sind vorbelegt.
3. **Trunk.** *Einstellungen → Trunks → Neuer Trunk*: Anbieter-Vorlage wählen und Zugangsdaten
   eintragen (z. B. [LEONET](leonet.md)). Unbekannter Anbieter: Vorlage
   „Generic SIP provider“.
4. **Standardrufnummer.** *Einstellungen → Standardrufnummer* setzen – ohne sie
   sind keine Gespräche nach außen und keine Notrufe möglich.
5. **Benutzer und Nebenstellen.** *Benutzer → Neuer Benutzer*, dann
   *Nebenstellen → Neue Nebenstelle* (2–8 Ziffern, nicht mit 0 oder 11
   beginnend) und den Benutzer zuordnen.
6. **Geräte.** In der Nebenstelle *Gerät hinzufügen*. TalkOps erzeugt
   SIP-Benutzername und -Passwort. Im Telefon eintragen:
   - SIP-Server/Registrar: IP-Adresse des TalkOps-Hosts, Port 5060 (UDP)
   - Benutzername / Authentifizierungsname: der angezeigte SIP-Benutzername
   - Passwort: das angezeigte SIP-Passwort

   Mehrere Geräte einer Nebenstelle klingeln gleichzeitig.
   Yealink-Autoprovisioning folgt in Phase 2.
7. **Rufnummern zuordnen.** *Einstellungen → Rufnummern*: für jede Rufnummer die Nebenstelle
   wählen, bei der sie klingeln soll.

Benutzer sehen nach dem Login unter *Meine Telefone* ihre Geräte samt
Zugangsdaten und unter *Anrufliste* ihre Gespräche.

## Wählen

| Eingabe | Wird gewählt als |
|---|---|
| `21` | Nebenstelle 21 |
| `110`, `112` | Notruf – immer über die Standardrufnummer, nie mit unterdrückter Rufnummer |
| `115`, `11833` | Sonderrufnummer, unverändert |
| `030 1234567` | national |
| `0043 1 234567`, `+43…` | international |
| `1234567` | Ortsnetz (mit hinterlegter Ortsvorwahl) |
| `*31 030 1234567`, `#31#030 1234567` | dieser Anruf mit unterdrückter Rufnummer |
