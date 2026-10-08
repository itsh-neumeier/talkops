# Anrufsteuerung

Unter **Anrufsteuerung** legst du fest, wohin Anrufe gehen. Rufnummern,
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
Er wird unter **Anrufsteuerung → Smart Attendant** als Baum aus Schritten
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

## Anrufsperre

**Anrufsteuerung → Anrufsperre** weist eingehende Anrufe ab (SIP `603 Decline`):

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
