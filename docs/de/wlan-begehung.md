# WLAN-Begehung

[Handbuch](index.md) · [English](../en/wifi-survey.md)

Eine WLAN-Begehung prüft Sprachqualität und Roaming eines WLAN-Handsets auf
dem ganzen Gelände. Ein Administrator geht mit dem Handset über das
Gelände, während TalkOps einen Testanruf offen hält. TalkOps misst den
Sprachstrom jede Sekunde, zeichnet das Gespräch auf und liest den
WLAN-Zustand des Handsets aus UniFi Network. Nach dem Anruf bewertet
TalkOps jedes Roaming und gibt Empfehlungen.

Die Architekturentscheidung ist [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md).

## Voraussetzungen

- Eine Nebenstelle mit einem Gerät der Art **WLAN-Handset** (z. B. Yealink
  AX83H oder AX86R).
- Für Access-Point-Daten: Das Gerät ist ein Konto auf einem provisionierten
  Telefon (TalkOps nutzt dessen MAC-Adresse), und eine UniFi-Verbindung ist
  eingerichtet (siehe unten). Ohne UniFi misst die Begehung nur den
  Sprachstrom.
- Berechtigungen unter **Einstellungen → Benutzer → Rollen**:

| Berechtigung | Funktion |
| --- | --- |
| `wifi_survey.run` | Begehungen starten und eigene Begehungen lesen. |
| `wifi_survey.manage` | Alle Begehungen lesen, Schwellen und Löschfrist ändern, Begehungen löschen. |
| `unifi.manage` | UniFi-Verbindung speichern und testen (sensibel). |

Die Admin-Rolle hat alle Berechtigungen, andere Rollen standardmäßig keine.

## Handset vorbereiten

1. VAD/Silence Suppression am Handset ausschalten. Mit VAD sieht Stille wie
   Paketverlust aus. Die Ergebnisseite warnt, wenn TalkOps VAD erkennt.
2. Handset laden.
3. Prüfen, dass Roaming aktiv ist und das Handset die Sprach-SSID nutzt.
4. Empfohlen: DSCP EF (46) für RTP am Handset einstellen.

## UniFi verbinden (nur lesend)

1. In UniFi Network unter **Einstellungen → Control Plane → Integrations**
   einen API-Schlüssel erstellen.
2. In TalkOps **Einstellungen → WLAN-Begehung → UniFi** öffnen.
3. Adresse der Konsole (z. B. `https://192.168.1.1`) und Site-Name
   (meist `default`) eintragen.
4. API-Schlüssel eintragen.
5. Bei selbstsigniertem Zertifikat **Zertifikat lesen** wählen, den
   Fingerabdruck mit der Konsole vergleichen und speichern. TalkOps
   akzeptiert dann nur dieses Zertifikat.
6. **Verbindung testen** wählen.

TalkOps speichert den API-Schlüssel verschlüsselt, zeigt ihn nie wieder an
und schreibt ihn nie in ein Log oder das Audit-Log. TalkOps sendet nur
lesende Anfragen an UniFi und ändert die UniFi-Konfiguration nie.

Der Verbindungstest zeigt, welche Endpunkte antworten:

| Endpunkt | Quelle | Verwendung |
| --- | --- | --- |
| `/proxy/network/integration/v1/info` | Integration API | Network-Version |
| `/proxy/network/integration/v1/sites` | Integration API | Site-ID |
| `/proxy/network/integration/v1/sites/{id}/devices` | Integration API | AP-Namen (Ersatz) |
| `/proxy/network/api/s/{site}/stat/sta/{mac}` | Klassische API | AP, Band, Kanal, Signal, Datenrate |
| `/proxy/network/api/s/{site}/stat/device-basic` | Klassische API | AP-Namen |

Die Integration API veröffentlicht Signal, Kanal und Datenrate eines
Clients nicht. TalkOps liest sie deshalb aus dem klassischen Endpunkt
`stat/sta`. Antwortet dieser nicht, hat die Begehung keine Signalwerte.

### UniFi-Network-Versionen

| Network-Version | Integration API | `stat/sta` mit API-Schlüssel | Stand |
| --- | --- | --- | --- |
| 10.3.58 (Referenz) | veröffentlicht | nicht dokumentiert | laut Dokumentation, nicht mit Hardware getestet |
| andere Versionen | — | — | nicht getestet |

Nach einem Test mit einer Version bitte eine Zeile ergänzen. Der
Verbindungstest zeigt Version und verfügbare Endpunkte.

## Begehung starten

1. **Einstellungen → WLAN-Begehung** öffnen oder in der Geräteliste einer
   Nebenstelle **WLAN-Begehung** wählen.
2. **Begehung starten** wählen.
3. WLAN-Gerät auswählen. Der Dialog zeigt MAC-Adresse und aktuellen
   UniFi-Zustand.
4. Startart wählen:
   - **TalkOps ruft das Handset jetzt an**: Nur dieses Gerät klingelt, nicht
     die anderen Geräte der Nebenstelle.
   - **Ich wähle \*39 am Handset**: innerhalb von 10 Minuten `*39` wählen.
     Ein `*39`-Anruf ohne vorbereitete Begehung spielt einen kurzen Hinweis
     und endet.
5. Optional eine Bezeichnung eingeben, z. B. „vor AP-Umbau“.
6. Checkliste bestätigen und **Starten** wählen.

## Während der Begehung

Das Handset spielt jede Sekunde einen kurzen Ton, alle 30 Sekunden sagt
eine Stimme „Messung läuft“. Ein fehlender Ton ist ein hörbarer Aussetzer.

Tasten am Handset:

| Taste | Funktion |
| --- | --- |
| `1` | Markierung „Störung“: Aussetzer gehört. |
| `2` | Markierung „neuer Ort“. |
| `3` | Markierung „Notiz“ (Text später ergänzen). |
| `#` | Test beenden. |

Die Live-Ansicht zeigt alle Werte auf einem Handy-Bildschirm:

- Ampel der letzten 5 Sekunden, Laufzeit, **Beenden**
- Access Point, Band und Kanal, Signal (RSSI), Datenrate
- Pakete der letzten Sekunde, Uplink-Verlust und längste Lücke der letzten
  10 Sekunden
- Downlink-Verlust und Jitter aus dem letzten RTCP-Bericht des Handsets
- geschätzter MOS, Anzahl Roamings und letztes Roaming mit Bewertung
- Schaltflächen für Markierungen und Notizen
- Diagramm der letzten 2 Minuten und Ereignisliste

Die Seite hält den Bildschirm an, wenn der Browser das unterstützt. Bricht
die Verbindung ab (Funkloch), misst der Server weiter; die Seite lädt die
fehlenden Werte nach.

Der Test endet beim Auflegen, mit `#`, mit **Beenden** oder nach der
Höchstdauer (Standard 60 Minuten).

## Ergebnis

Der Media-Worker wertet die Begehung nach dem Anruf aus. Die Ergebnisseite
zeigt:

- Zusammenfassung: Dauer, Roamings je Bewertung, größte Lücke, Verlust,
  geschätzter MOS, Signal, Markierungen, Codec
- Empfehlungen mit den gemessenen Werten
- Zeitleiste mit AP-Abschnitten, Signal, Verlust je Sekunde,
  Verlustphasen, Roamings und Markierungen. Ein Klick spielt die Aufnahme
  ab 3 Sekunden vor dieser Stelle.
- Roaming-Tabelle mit **Abspielen** je Roaming
- erkannte Muster, Zeit je Access Point und Markierungen mit editierbarer
  Notiz
- Export als CSV und JSON

Die Aufnahme ist stereo: links das Mikrofon des Handsets, rechts das
Testsignal von TalkOps.

### Bewertung je Roaming

TalkOps erkennt ein Roaming, wenn sich der Access Point zwischen zwei
UniFi-Abfragen ändert. Ein Wechsel von Band oder Kanal am selben Access
Point ist ein Bandwechsel. Das Fenster eines Roamings beginnt 3 Sekunden
vor der letzten Abfrage am alten und endet 3 Sekunden nach der ersten
Abfrage am neuen Access Point.

| Bewertung | Regel (Standard) |
| --- | --- |
| Sauber | Längste Lücke unter 50 ms |
| Hörbar | Längste Lücke 50 bis 150 ms |
| Schlecht | Längste Lücke über 150 ms oder mehr als 5 % Verlust im Fenster (Verlustblock) |

Die Lücke ist die Zahl fehlender Uplink-Pakete in aufeinanderfolgenden
Sekunden mal Paketdauer (meist 20 ms).

### Muster

| Muster | Regel (Standard) |
| --- | --- |
| Sticky Client | Signal 10 s unter -75 dBm an einem Access Point, danach Roaming oder Verlust |
| Pingpong | 3 Roamings zwischen denselben zwei Access Points in 60 s |
| Aussetzer ohne Roaming | Lücke ab 50 ms ohne Roaming in ±3 s |
| Roaming ohne Aussetzer | Roaming mit Bewertung „sauber“ |
| Schwache Abdeckung | Signal 5 s unter -80 dBm |

UniFi meldet für einen Client nicht das Signal anderer Access Points.
„Sticky Client“ zeigt deshalb nur, dass wahrscheinlich ein besserer Access
Point verfügbar war.

Die Schwellen stehen unter **Einstellungen → WLAN-Begehung →
Einstellungen**. Neue Schwellen gelten für neue Begehungen; **Neu
auswerten** wendet sie auf eine alte Begehung an.

### Zwei Begehungen vergleichen

In der Liste zwei ausgewertete Begehungen auswählen und **Vergleichen**
wählen. Die Seite zeigt beide Zusammenfassungen und die Roamings je
Access-Point-Paar.

## Grenzen der Messung

- TalkOps nutzt die Statistik von FreeSWITCH. Uplink-Werte haben eine
  Auflösung von einer Sekunde. FreeSWITCH liefert keinen Jitter je
  Sekunde; Jitter-Werte stammen aus den RTCP-Berichten des Handsets.
- Der MOS ist die Schätzung von FreeSWITCH aus verlorenen und verspäteten
  Paketen, keine ITU-T-Messung.
- Downlink-Werte hängen davon ab, wie oft das Handset RTCP-Berichte sendet.
- UniFi-Werte haben die Genauigkeit des Abfrageintervalls (etwa eine
  Sekunde).

## Datenschutz und Löschfrist

- TalkOps liest WLAN-Daten nur während einer aktiven Begehung und nur für
  die MAC-Adresse des Testgeräts.
- Ortsdaten sind nur die Markierung „neuer Ort“ und optionale Notizen.
  Keine personenbezogenen Daten in Notizen schreiben.
- Die Begehung zeichnet das Gespräch immer auf. Tester ist normalerweise
  der Administrator.
- TalkOps löscht Begehung, Messwerte und Aufnahme nach der Löschfrist
  (Standard 30 Tage, **Einstellungen → WLAN-Begehung → Einstellungen**).
- Start, Stopp, Markierungen, Export, Einstellungs- und UniFi-Änderungen
  erzeugen Audit-Einträge ohne API-Schlüssel und ohne Notiztext.
