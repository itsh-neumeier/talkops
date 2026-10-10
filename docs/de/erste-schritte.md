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
5. **Benutzer und Nebenstellen.** *Einstellungen → Benutzer → Neuer Benutzer*, dann
   *Einstellungen → Nebenstellen → Neue Nebenstelle* (ab 100, 3–8 Ziffern, z. B.
   frei 100–9999; nicht mit 0 beginnend, nicht 115 und keine Nummer, die mit
   einer Notrufnummer anfängt oder in ihr steckt, z. B. 110, 112, 1120) und den
   Benutzer zuordnen. `*1`–`*99` sind für Systemcodes reserviert, deshalb
   beginnen interne Nummern bei 100 – das gilt auch für Gruppen,
   Zeitsteuerungen, Menüs und Warteschlangen. Zweistellige Nummern aus älteren
   Installationen bleiben erhalten, neue lassen sich nicht mehr anlegen.
6. **Geräte.** In der Nebenstelle *Gerät hinzufügen*. TalkOps erzeugt
   SIP-Benutzername und -Passwort. Im Telefon eintragen:
   - SIP-Server/Registrar: IP-Adresse des TalkOps-Hosts, Port 5060 (UDP)
   - Benutzername / Authentifizierungsname: der angezeigte SIP-Benutzername
   - Passwort: das angezeigte SIP-Passwort

   Mehrere Geräte einer Nebenstelle klingeln gleichzeitig.
   Yealink-Autoprovisioning folgt in Phase 2.
7. **Rufnummern zuordnen.** *Einstellungen → Rufnummern* (oder beim Trunk): für jede
   Rufnummer die (primäre) Nebenstelle wählen, bei der sie klingeln soll. Unter
   *Weitere Nebenstellen klingeln lassen …* klingeln zusätzliche Nebenstellen
   gleichzeitig; unbeantwortet geht der Anruf auf die Voicemail der primären.
   Dasselbe geht von der Nebenstelle aus: im Dialog *Bearbeiten* unter
   *Eingehende Rufnummern, die hier klingeln* anhaken.

Benutzer sehen nach dem Login unter *Einstellungen → Meine Telefone* ihre Geräte samt
Zugangsdaten und unter *Anrufliste* ihre Gespräche.

## Wählen

| Eingabe | Wird gewählt als |
|---|---|
| `210`, `610` | Nebenstelle 210 bzw. 610 |
| `*610` | ebenfalls Nebenstelle 610 (Stern + interne Nummer, ab `*100`, 3–4 Stellen) |
| `*1` … `*99` | Systemcodes, siehe [Yealink-Telefone](yealink.md#4-kurzwahlen-am-telefon) |
| `110`, `112` | Notruf – immer über die Standardrufnummer, nie mit unterdrückter Rufnummer |
| `115`, `11833` | Sonderrufnummer, unverändert |
| `030 1234567` | national |
| `0043 1 234567`, `+43…` | international |
| `1234567` | Ortsnetz (mit hinterlegter Ortsvorwahl) |
| `*31 030 1234567`, `#31#030 1234567` | dieser Anruf mit unterdrückter Rufnummer |
| `name@domain.de` | SIP-Adresse: über einen Trunk-Account derselben Domain, sonst direkt übers Internet |

### SIP-Adressen und Accounts ohne Rufnummer

Manche Anbieter (z. B. sip2sip.info / SIP Thor) vergeben nur eine SIP-Adresse
wie `name@sip2sip.info`, keine Telefonnummer:

1. *Einstellungen → Trunks → Neuer Trunk*, Vorlage *Generic SIP provider*,
   Registrar des Anbieters (z. B. `sip2sip.info`).
2. Im Trunk *Account hinzufügen* mit Benutzername und Passwort – eine
   Rufnummer ist nicht nötig. Unter *Eingehende Anrufe an diesen Account*
   die Nebenstelle (oder Gruppe, Voicemail …) wählen, bei der Anrufe an die
   SIP-Adresse klingeln sollen.
3. Rauswählen: im Softphone oder am Telefon die Adresse eingeben, z. B.
   `anna@sip2sip.info`. Adressen derselben Domain gehen über den Account,
   alle anderen direkt übers Internet.

SIP-Adressen sind erkennbar an Buchstaben im Namen; reine Ziffern
(`12345@domain`) bleiben Telefonnummern. Abschalten unter *Einstellungen →
Telefonie → SIP-Adressen wählen erlauben*. Für direkte Anrufe übers
Internet sollte die externe IP eingetragen sein, sonst kommt der Ton
womöglich nicht an.
