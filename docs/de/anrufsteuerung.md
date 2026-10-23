# Anrufsteuerung

[Handbuch](index.md) · [English](../en/call-routing.md)

## Voraussetzungen

Du benötigst: routing.manage für Ablaufänderungen; Sperrberechtigungen sind getrennt.

## Lokale Whitelist für den Spam-Schutz

Unter **Einstellungen → Anrufsteuerung → Anrufsperre → Whitelist** lassen
sich einzelne internationale Rufnummern freigeben oder Freigaben entfernen.
Dafür ist das Recht **Whitelist verwalten** (`blocking.allowlist`) nötig.
Standardmäßig hat es nur der Administrator. Er kann es weiteren Rollen
unter **Benutzer → Rollen** zuweisen.

Das Recht allein erlaubt den Zugriff auf die Whitelist. Es gewährt keine
Routingverwaltung, keine Verwaltung der Sperrliste, keinen Zugriff auf
PhoneBlock-Zugangsdaten und keine Einsicht in alle Anrufe.

Eine Freigabe gilt für den gesamten Mandanten und sofort ab dem nächsten
eingehenden Anruf. Sie übersteuert sowohl die eigene Sperrliste als auch
PhoneBlock, einschließlich zwischengespeicherter Ablehnungen. Bei einer
gesperrten Vorwahl kann so genau eine Nummer freigegeben werden. Zeitpläne
und andere Routingregeln gelten weiterhin. Anonyme Anrufer und
Platzhalter-Einträge können nicht freigegeben werden.

Beim Löschen warnt der Dialog, dass die Nummer wieder blockiert werden
kann. Die ursprünglichen Sperrregeln bleiben erhalten. Alle Änderungen
werden im Audit-Log protokolliert. Die Freigabe ändert keine Liste im
PhoneBlock-Konto. Backups enthalten Whitelist und gespeicherte Anrufgründe.

Die Anzeige und direkte Freigabe aus der Anrufliste sind in der
[Anrufliste](anrufliste.md#blockierte-anrufe-und-whitelist) beschrieben.

Unter **Einstellungen → Anrufsteuerung** legst du fest, wohin Anrufe gehen. Rufnummern,
Ausweichziele, Menütasten und Öffnungszeiten können auf jedes dieser Ziele
zeigen: Nebenstelle, Voicemail einer Nebenstelle, Rufgruppe, Zeitsteuerung,
Smart Attendant oder Warteschlange. Interne Nummern sind über alle Arten hinweg
eindeutig.

## Rufgruppen

Mehrere Nebenstellen klingeln **gleichzeitig** oder **nacheinander**
(Klingeldauer je Mitglied). Mitglieder mit „Nicht stören“ werden übersprungen.
Nimmt niemand ab, geht der Anruf an das Ausweichziel – z. B. die Voicemail
einer Nebenstelle. Ein optionales Präfix („Support: “) zeigt den Telefonen,
über welche Gruppe der Anruf kommt.

## Öffnungszeiten (Zeitsteuerung)

- Zeitfenster je Wochentag (mehrere pro Tag möglich, z. B. 08–12 und 13–17 Uhr),
- gesetzliche **Feiertage** bundesweit oder je Bundesland (automatisch
  berechnet; Feiertage, die nur in einzelnen Gemeinden gelten, bitte als
  Schließtag eintragen),
- zusätzliche **Schließtage** (Betriebsferien, Brückentage),
- je ein Ziel für „geöffnet“ und „geschlossen“.

Der Modus lässt sich jederzeit umschalten: *Automatisch*, *Immer geöffnet*,
*Immer geschlossen* – in der Weboberfläche (auch für Operatoren) oder am
Telefon mit `*30<Nummer>` (z. B. `*3060`; ein tiefer Doppelton bedeutet
„geschlossen“, ein hoher „wieder automatisch“). Es gilt die Zeitzone aus den
Einstellungen.

## Smart Attendant

Ein Smart Attendant ist ein Ablauf für eingehende Anrufe – wie bei UniFi Talk.
Er wird unter **Einstellungen → Anrufsteuerung → Smart Attendant** als Baum aus Schritten
gebaut: Mit **+** kommt ein Schritt hinzu, ein Klick auf einen Schritt öffnet
rechts seine Einstellungen.

| Schritt | Was passiert |
|---|---|
| Tastenmenü | Ansage, Anrufer wählen per Taste (0–9, \*, #); je Taste ein eigener Zweig, dazu „Keine Eingabe“. Optional dürfen Anrufer Nebenstellen direkt wählen. |
| Telefone klingeln | Ausgewählte Nebenstellen klingeln gleichzeitig oder nacheinander; nimmt niemand ab, geht es mit dem Zweig „Keine Antwort“ weiter. |
| Audio abspielen | Spielt eine Ansage und macht dann weiter. |
| Zeitplan | Verzweigt nach einer Zeitsteuerung (Öffnungszeiten, Feiertage, Schließtage) in „Geöffnet“ und „Geschlossen“. |
| Voicemail | Anrufer hinterlassen eine Nachricht; jeder ausgewählte Empfänger bekommt sie in seine Voicemail-Box (mit E-Mail und Transkription wie gewohnt). |
| Weiterleiten | Zu Nebenstelle, Rufgruppe, Warteschlange, anderem Smart Attendant … |
| Anruf parken | Parkt auf einem freien Platz `*51`–`*59`; von jedem Telefon aus heranholbar. |
| Gehe zu Schritt | Springt zu einem anderen Schritt, z. B. „zurück zum Hauptmenü“. |
| Auflegen | Verabschiedet sich und beendet den Anruf. |

Ein leerer Zweig beendet den Anruf. Ein **Rechtsklick** auf einen Schritt öffnet ein Menü zum Bearbeiten, Ersetzen durch einen anderen Schritt oder Löschen (samt der Schritte darunter); ein Rechtsklick auf eine Taste entfernt diesen Zweig. Ansagen werden wie bei der Voicemail mit
der Computerstimme generiert, im Browser aufgenommen oder hochgeladen (siehe
[Voicemail](voicemail.md#ansagen-und-audio)). Sprachmenüs aus TalkOps 1.0
werden beim Update automatisch in einen Smart Attendant mit Tastenmenü
übernommen.

## Warteschlangen

Anrufer warten mit Musik, bis ein Agent frei ist (FreeSWITCH `mod_callcenter`).
Jede Warteschlange hat eine eigene Seite mit drei Reitern:

- **Allgemein:** Nummer, Name, Agenten (Reihenfolge per Pfeil). Agenten mit
  „Nicht stören“ oder ohne Geräte erhalten keine Anrufe.
- **Zeitplan:** eine Zeitsteuerung als Öffnungszeiten; außerhalb davon geht
  der Anruf an das Ziel „Außerhalb der Öffnungszeiten“.
- **Anrufbehandlung:**
  - *Begrüßung* (einmal vor dem Warten) und *Wartemusik* (in Schleife; ohne
    eigene Datei die Wartemusik aus Einstellungen → *Wartemusik*),
  - *Anrufverteilung*: am längsten frei, alle klingeln, reihum, der Reihe
    nach, wenigste Gespräche, zufällig; Klingeldauer je Agent, Pause nach
    jedem Gespräch,
  - *Größe*: höchstens wartende Anrufer – weitere gehen an das Überlaufziel,
  - *Wenn niemand annimmt* (nach der maximalen Wartezeit oder wenn 30
    Sekunden lang kein Agent verfügbar ist): weiterleiten **oder** eine
    Nachricht für einen oder mehrere Empfänger aufnehmen.

## Parken und Weitervermitteln

- **Parken:** einen Anruf zu `*51` … `*59` weitervermitteln parkt ihn auf
  diesem Platz; dieselbe Nummer von einem anderen Telefon holt ihn zurück.
  Yealink-BLF-Tasten können einen Parkplatz mit dem Wert `park+*51`
  überwachen.
- **Weitervermitteln** (Blind-Transfer) vom Telefon funktioniert zu internen
  Nummern, Parkplätzen und externen Rufnummern (über die Standardrufnummer).
  Für externe Nummern gilt das [Wählprofil](#wahlberechtigungen) der
  weitervermittelnden Nebenstelle.

## Anrufsperre

**Einstellungen → Anrufsteuerung → Anrufsperre** weist eingehende Anrufe ab (SIP `603 Decline`):

- **Gesperrte Nummern**: einzelne Nummern (`+4930123456`) oder ganze
  Vorwahlen mit `*` am Ende (`+49900*` für Mehrwertnummern).
- **Anonyme Anrufer** (Rufnummer unterdrückt) abweisen.
- **PhoneBlock** ([phoneblock.net](https://phoneblock.net/phoneblock/)):
  kostenlose Community-Liste von Spam- und Werbeanrufern. Auf phoneblock.net ein
  Konto anlegen, unter *Einstellungen → API-Schlüssel* einen Schlüssel
  (`pbt_…`) erzeugen und hier eintragen. Jeder eingehende Anruf wird mit
  höchstens 1,5 s Wartezeit geprüft; Antworten werden 6 Stunden
  zwischengespeichert. Ist PhoneBlock nicht erreichbar, klingelt der Anruf
  normal. *Nötige Meldungen*: PhoneBlock selbst sperrt ab 4.
  PhoneBlock rät, auf geschäftlichen Leitungen vorsichtig zu sperren.
- **Nummer testen** zeigt, ob und warum ein Anruf gesperrt würde.

Gesperrte Anrufe stehen im Log mit `inbound call blocked` und dem Grund.

## Wahlberechtigungen

**Anrufsteuerung → Wahlberechtigungen** legt fest, welche Rufnummern
Nebenstellen anrufen dürfen. Dafür ist das Recht `dialing.manage` nötig.
Standardmäßig haben es nur Administratoren. Das Konzept beschreibt
[ADR 0045](../adr/0045-outbound-dialing-permissions.md).

### Wählprofile

Ein Wählprofil besteht aus:

- **Nummern ohne Regel**: alle Nummern erlauben oder sperren.
- **SIP-Adressen**: Anrufe an `user@host` erlauben oder sperren.
- **Regeln**: ein Land, ein E.164-Präfix (`+49900*`) oder eine einzelne
  Nummer (`+4930123456`) erlauben oder sperren. Länder zeigt der Editor
  mit Flagge.

Die spezifischste Regel gewinnt:

1. Eine Regel für die vollständige Nummer.
2. Sonst die Regel mit dem längsten Präfix.
3. Sonst die Regel für das Land der Nummer.
4. Sonst die Einstellung für Nummern ohne Regel.

Haben zwei Regeln denselben Rang, gewinnt Sperren. Die Reihenfolge der
Regeln spielt keine Rolle. Länderregeln nutzen die Geodaten in
`presets/geo`. Findet TalkOps kein Land, greift keine Länderregel.

**Notrufe sind immer erlaubt.** TalkOps prüft die Notrufnummern aus den
Telefonie-Einstellungen nie gegen ein Profil.

Kurze Servicenummern (z. B. `11833`) prüft TalkOps in nationaler Form
(`+4911833`).

### Standardprofil und Nebenstellen

1. Bei einem Profil **Als Standard** wählen. Nebenstellen ohne eigenes
   Profil nutzen das Standardprofil.
2. Für ein eigenes Profil die Nebenstelle öffnen, **Bearbeiten** wählen und
   ein **Wählprofil** auswählen.

Ein Profil, das Standard ist oder von einer Nebenstelle genutzt wird, lässt
sich nicht löschen.

### Geprüfte Wege nach außen

| Weg | Geltendes Profil |
| --- | --- |
| Anruf vom Telefon oder Browser-Softphone | Profil der Nebenstelle. |
| Anruf an eine SIP-Adresse | Profil der Nebenstelle (Einstellung **SIP-Adressen**). |
| Rufumleitung auf eine externe Nummer | Profil der umleitenden Nebenstelle. |
| Weitervermitteln (Blind-Transfer) an eine externe Nummer | Profil der Nebenstelle, die weitervermittelt. Kann TalkOps sie nicht erkennen, das Standardprofil. |
| Konferenzteilnehmer vom Softphone | Profil der Nebenstelle des Softphones. |
| Fax-Versand | Jede aktive Nebenstelle des Fax-Besitzers muss die Nummer erlauben. Ein Benutzer ohne Nebenstelle nutzt das Standardprofil. |

Der Umleitungscode `*72` und die Anrufeinstellungen in der Weboberfläche
lehnen ein gesperrtes Umleitungsziel ab. Ändert sich das Profil später,
klingeln bei einem Anruf die Geräte der Nebenstelle statt des gesperrten
Ziels.

Ein gesperrter Anruf erhält `403 Forbidden`. Gibt es die Ansage
`dialing_blocked`, hört der Anrufer sie vorher. Die Anrufliste zeigt den
Anruf als **Gesperrt** mit dem Grund. Das Server-Log enthält
`outbound call blocked by dialing profile`.

### Profile nach dem Update

Das Update legt fünf Profile an, die sich ändern lassen:

| Profil | Nummern ohne Regel | Regeln |
| --- | --- | --- |
| Unrestricted | erlauben | keine |
| Internal only | sperren (auch SIP-Adressen) | keine |
| National | sperren | eigene Landesvorwahl erlauben, Mehrwertnummern sperren |
| National + Europe | sperren | eigenes Land, EU, EWR und Schweiz, Mehrwertnummern sperren |
| International | erlauben | Mehrwertnummern sperren |

- Bestehende Installationen telefonieren wie vorher: Standardprofil ist
  **Unrestricted**.
- Neue Installationen nutzen **International**.

**Mehrwertnummern sperren** im Editor fügt die Mehrwertregeln einem Profil
hinzu. Gesperrt werden `+49900`, `+49137`, `+49118`, `+4912`, `+4919`,
die österreichischen Bereiche `+43900`, `+43901`, `+43930`, `+43931`,
`+43939`, `+43118`, die Schweizer Bereiche `+41900`, `+41901`, `+41906`
sowie die Netze `+870`, `+881`, `+882`, `+883`. Shared-Cost-Nummern
(`+49180`) sind nicht enthalten; dafür eine Präfixregel anlegen.

### Nummer testen

**Nummer testen** zeigt Profil, Regel und Ergebnis für eine Nummer. Die
Nummer so eingeben, wie ein Telefon sie wählt, z. B. `0900 1234567`.

Die API bietet `GET` und `POST /api/v1/dialing-profiles`,
`PUT` und `DELETE /api/v1/dialing-profiles/{id}`,
`PUT /api/v1/dialing-profiles/{id}/default`,
`POST /api/v1/dialing-profiles/test` und
`PUT /api/v1/extensions/{id}/dialing-profile`. Jede Änderung erzeugt einen
Audit-Eintrag. Sicherungen enthalten die Profile und die gespeicherten
Anrufentscheidungen.

## Ergebnis prüfen

Prüfe jede geänderte Verzweigung mit einem Testanruf, auch Zeitüberschreitung und geschlossene Zeiten.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.
