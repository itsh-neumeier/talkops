# Transkriptions-Jobs

## Aufnahme erneut transkribieren

1. Öffne die **Anrufliste** und klicke bei einem Anruf auf **Aufnahme**.
2. Klicke **Erneut transkribieren …**.
3. Wähle ein Modell. Der Dialog zeigt für jedes Modell:
   - die geschätzte Dauer (**gemessen** auf diesem Server oder
     **geschätzt** aus Standardwerten),
   - die geschätzten Kosten eines Cloud-Modells,
   - die Wartezeit für die Jobs davor in der Warteschlange.
4. Klicke **Starten**.

Das aktuelle Transkript bleibt sichtbar, bis das neue fertig ist. Pro
Aufnahme kann nur eine manuelle Transkription warten oder laufen. Du
brauchst die Berechtigung **Erneut transkribieren** und Zugriff auf die
Aufnahme.

## Seite „Transkriptionen“

Der Menüpunkt **Transkriptionen** zeigt die Transkriptions-Jobs. Du brauchst
die Berechtigung **Eigene Transkriptions-Jobs** (selbst gestartete Jobs und
Jobs der eigenen Nebenstellen) oder **Alle Transkriptions-Jobs**.

Der Tab **Laufend und wartend** zeigt pro Job:

- den Anruf, die Audiolänge und das Modell,
- den Zustand: läuft, wartet, pausiert, pausiert … oder stoppt …,
- einen Fortschrittsbalken mit Prozentwert,
- die Restzeit (laufend) oder das erwartete Ende (wartend).

Schaltflächen:

| Schaltfläche | Wirkung |
| --- | --- |
| Pausieren | Ein wartender Job bleibt in der Warteschlange. Ein laufender lokaler Job hält innerhalb von etwa einer Sekunde an und merkt sich die Stelle. |
| Fortsetzen | Der Job kommt wieder in die Warteschlange. Ein lokaler Job setzt an der Pausenstelle fort, ein Cloud-Job beginnt neu. |
| Stoppen | Der Job endet. Ein manueller oder zweiter Durchgang behält das aktuelle Transkript. Ein gestoppter erster Durchgang hinterlässt kein Transkript; eine Voicemail-E-Mail geht ohne Transkript hinaus. |
| Vorziehen | Der wartende Job rückt an den Anfang der Warteschlange. |

Der Tab **Verlauf** zeigt die fertigen, fehlgeschlagenen und gestoppten Jobs
der letzten 7 Tage (höchstens die neuesten 1000 Jobs).

### Verlauf: Gesprächspartner, Sortierung und Filter

Die Spalte **Gesprächspartner** zeigt Anrufer und Angerufenen des Gesprächs
oder der Voicemail. Die Namen kommen aus dem Telefonbuch, wie in der
[Anrufliste](anrufliste.md): ein Klick auf den Namen öffnet den
Telefonbucheintrag. Bei eingehenden Anrufen zeigt die Zeile darunter die
eigene Rufnummer, die der Anrufer gewählt hat.

- **Sortieren:** auf den Spaltentitel **Zeit**, **Dauer** oder **Ergebnis**
  klicken. Ein zweiter Klick kehrt die Reihenfolge um (absteigend,
  aufsteigend). **Dauer** ist die Verarbeitungszeit. Jobs, die nicht liefen,
  stehen am Ende.
- **Filtern:** eine Spalte mit Filter hat eine Trichter-Schaltfläche. Ein
  farbiger Punkt zeigt einen aktiven Filter.

  | Spalte | Filter |
  | --- | --- |
  | Zeit | Heute, gestern oder ein Datumsbereich in den letzten 7 Tagen (beide Tage eingeschlossen). |
  | Richtung | Eine oder mehrere Richtungen. Eine Voicemail von einer Nebenstelle ist intern, andere Voicemails sind eingehend. |
  | Modell | Ein oder mehrere Modelle der Jobs im Verlauf. |
  | Ergebnis | Fertig, fehlgeschlagen, gestoppt. |

- **Suchen:** das Suchfeld findet Text in Namen und Nummern der
  Gesprächspartner und im Namen des Benutzers, der den Job gestartet hat.
  Eine Suche nur mit Ziffern, z. B. `0911 2193`, findet auch `+49 911 2193…`.
- **Alle Filter zurücksetzen** löscht die Filter und die Suche. Die
  Sortierung bleibt.

Der Browser filtert und sortiert den geladenen Verlauf, daher wirkt jede
Änderung sofort. Die Seitenadresse enthält Tab, Filter und Sortierung. Ein
Neuladen oder ein kopierter Link zeigt daher dieselbe Liste. Der Verlauf lädt
bei geöffnetem Tab alle 30 Sekunden neu, die aktiven Jobs alle 2 Sekunden.

Auf schmalen Bildschirmen sind die Spalten *Modell* und *Dauer* ausgeblendet.

## Ressourcen

Mit der Berechtigung **Alle Transkriptions-Jobs** zeigt die Seite zusätzlich
jeden Media-Worker:

- **CPU**: Rechenzeit des Workers und seiner Whisper-Prozesse; 100 % ist ein
  Kern.
- **Arbeitsspeicher**: Speichernutzung und Grenze des Containers (oder des
  Hosts).
- **Worker und Whisper**: Speicher der Worker-Prozesse.
- **Last**: Lastdurchschnitt der letzten Minute.

Die Seite aktualisiert sich alle 2 Sekunden.

## Schätzungen

Der Worker misst jeden vollständigen Durchlauf: Rechenzeit pro Sekunde
Aufnahme, je Modell. Die Schätzung nutzt den Mittelwert der ersten Läufe und
folgt danach Änderungen langsam. Fortgesetzte Läufe zählen nicht.

## API

Siehe die [englische Dokumentation](../en/transcriptions.md#api).
