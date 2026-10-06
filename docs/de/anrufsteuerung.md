# Anrufsteuerung

Unter **Anrufsteuerung** legst du fest, wohin Anrufe gehen. Rufnummern,
Ausweichziele, Menütasten und Öffnungszeiten können auf jedes dieser Ziele
zeigen: Nebenstelle, Voicemail einer Nebenstelle, Rufgruppe, Zeitsteuerung,
Sprachmenü oder Warteschlange. Interne Nummern sind über alle Arten hinweg
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

## Sprachmenüs

Anrufer hören eine Ansage und wählen per Tastatur. Die Ansage wird aus Text
von der Computerstimme erzeugt oder als WAV-Datei hochgeladen. Jeder Taste
(0–9, \*, #) wird ein Ziel zugeordnet; optional dürfen Anrufer Nebenstellen
direkt wählen. Ohne gültige Eingabe nach der eingestellten Zahl an Versuchen
geht der Anruf an das Ziel „Keine Eingabe“ (oder wird beendet).

## Warteschlangen

Anrufer warten mit Musik, bis ein Agent frei ist (FreeSWITCH `mod_callcenter`).
Strategien: am längsten frei, alle klingeln, reihum, der Reihe nach, wenigste
Gespräche, zufällig. Einstellbar sind maximale Wartezeit, Klingeldauer je
Agent und eine Pause nach jedem Gespräch. Agenten mit „Nicht stören“ oder ohne
Geräte erhalten keine Anrufe. Nach Ablauf der Wartezeit – oder wenn 30 Sekunden
lang kein Agent verfügbar ist – geht der Anruf an das Ausweichziel.

## Parken und Weitervermitteln

- **Parken:** einen Anruf zu `*51` … `*59` weitervermitteln parkt ihn auf
  diesem Platz; dieselbe Nummer von einem anderen Telefon holt ihn zurück.
  Yealink-BLF-Tasten können einen Parkplatz mit dem Wert `park+*51`
  überwachen.
- **Weitervermitteln** (Blind-Transfer) vom Telefon funktioniert zu internen
  Nummern, Parkplätzen und externen Rufnummern (über die Standardrufnummer).
