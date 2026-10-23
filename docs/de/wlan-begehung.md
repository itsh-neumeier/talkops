# WLAN-Begehung

[Handbuch](index.md) · [English](../en/wifi-survey.md)

Eine WLAN-Begehung prüft Sprachqualität und Roaming eines WLAN-Handsets auf
dem ganzen Gelände. Ein Administrator geht mit dem Handset über das
Gelände, während TalkOps einen Testanruf offen hält. TalkOps misst den
Sprachstrom jede Sekunde, zeichnet das Gespräch auf und liest den
WLAN-Zustand des Handsets aus UniFi Network. Nach dem Anruf bewertet
TalkOps jedes Roaming und gibt Empfehlungen.

Die Architekturentscheidungen sind [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
(Begehung, Lesezugriff), [ADR 0042](../adr/0042-unifi-voice-ssid-write-access.md)
(Änderungen an der VoIP-SSID) und [ADR 0043](../adr/0043-wifi-handset-band-and-roaming.md)
(Roaming-Schwelle der Handsets, Bandschutz).

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
| `unifi.manage` | UniFi-Verbindung speichern und testen, VoIP-SSID auswählen, Vorschläge und Änderungsverlauf lesen (sensibel). |
| `unifi.write` | Schreibzugriff einschalten, Änderungen an der VoIP-SSID anwenden und zurücknehmen (sensibel). Mit `phones.manage` auch die Roaming-Schwelle des Handsets der Begehung. |

Die Admin-Rolle hat alle Berechtigungen, andere Rollen standardmäßig keine.

## Handset vorbereiten

1. VAD/Silence Suppression am Handset ausschalten. Mit VAD sieht Stille wie
   Paketverlust aus. Die Ergebnisseite warnt, wenn TalkOps VAD erkennt.
2. Handset laden.
3. Prüfen, dass Roaming aktiv ist und das Handset die Sprach-SSID nutzt.
4. Empfohlen: DSCP EF (46) für RTP am Handset einstellen.

## UniFi verbinden

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
und schreibt ihn nie in ein Log oder das Audit-Log. Ohne **Schreibzugriff
erlauben** (siehe [Empfehlungen in UniFi anwenden](#empfehlungen-in-unifi-anwenden))
sendet TalkOps nur lesende Anfragen an UniFi und ändert die
UniFi-Konfiguration nie.

Der Verbindungstest zeigt, welche Endpunkte antworten:

| Endpunkt | Quelle | Verwendung |
| --- | --- | --- |
| `/proxy/network/integration/v1/info` | Integration API | Network-Version |
| `/proxy/network/integration/v1/sites` | Integration API | Site-ID |
| `/proxy/network/integration/v1/sites/{id}/devices` | Integration API | AP-Namen (Ersatz) |
| `/proxy/network/api/s/{site}/stat/sta/{mac}` | Klassische API | AP, Band, Kanal, Signal, Datenrate |
| `/proxy/network/api/s/{site}/stat/device-basic` | Klassische API | AP-Namen |
| `/proxy/network/integration/v1/sites/{id}/wifi/broadcasts` | Integration API | SSID-Liste, VoIP-SSID |

Die Integration API veröffentlicht Signal, Kanal und Datenrate eines
Clients nicht. TalkOps liest sie deshalb aus dem klassischen Endpunkt
`stat/sta`. Antwortet dieser nicht, hat die Begehung keine Signalwerte.

### UniFi-Network-Versionen

| Network-Version | Integration API | `stat/sta` mit API-Schlüssel | Schreiben auf VoIP-SSID | Stand |
| --- | --- | --- | --- | --- |
| 10.3.58 (Referenz) | veröffentlicht | nicht dokumentiert | nein (nur lesend) | laut Dokumentation, nicht mit Hardware getestet |
| unter 10.6.106 | — | — | nein (nur lesend) | — |
| 10.6.106 | veröffentlicht | — | ja | laut veröffentlichter API-Spezifikation, nicht mit Hardware getestet |
| 11.0.89 (Ziel) | veröffentlicht | — | ja | laut veröffentlichter API-Spezifikation, nicht mit Hardware getestet |
| andere Versionen | — | — | — | nicht getestet |

UniFi Network 11.0.x braucht UniFi OS 6.0 oder neuer.

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

## Empfehlungen in UniFi anwenden

TalkOps kann einige Empfehlungen auf die VoIP-SSID in UniFi Network
anwenden. Die Funktion ist standardmäßig aus. Ohne sie bleibt TalkOps ein
rein lesendes Werkzeug.

### Regeln

- TalkOps ändert nur die SSID, die als VoIP-SSID ausgewählt ist. Sie muss
  eine Standard-SSID sein, die auf dieser Konsole angelegt wurde. IoT-SSIDs
  und zentral (Site Manager) verwaltete SSIDs ändert TalkOps nicht.
- TalkOps ändert nur diese Einstellungen: BSS Transition (802.11v), WPA3
  Fast Roaming, Protected Management Frames (PMF), Handoff-Vorschläge
  (minimaler RSSI), Frequenzbänder, U-APSD, Multicast zu Unicast,
  ARP-Proxy und die DTIM-Periode je Band.
- TalkOps schreibt nur über die offizielle UniFi Network API mit
  `PUT /proxy/network/integration/v1/sites/{siteId}/wifi/broadcasts/{id}`.
  Über die klassische API schreibt TalkOps nie. Funkeinstellungen der
  Access Points ändert TalkOps nicht.
- TalkOps schreibt nur nach Bestätigung der Werte im Diff-Dialog.
- Während eine Begehung vorbereitet ist, anruft oder läuft, schreibt
  TalkOps nicht.
- Nötig ist UniFi Network 10.6.106 oder neuer. Bei älteren Versionen bleibt
  TalkOps nur lesend.
- Vor jeder Änderung speichert TalkOps die Werte der geänderten
  Einstellungen (Snapshot). Ein Rollback stellt genau diese Einstellungen
  wieder her.
- Das WLAN-Passwort der SSID speichert, protokolliert und zeigt TalkOps
  nie.

### Risiken

- **Clients werden kurz getrennt.** UniFi wendet die SSID auf allen Access
  Points neu an. Alle Clients der SSID trennen sich und verbinden sich neu.
  Ein Gespräch über diese SSID kann abbrechen. Änderungen außerhalb der
  Arbeitszeit anwenden.
- **Kompatibilität von Fast Roaming.** Manche älteren Clients können sich
  mit 802.11r oder WPA3 Fast Roaming nicht verbinden. Dokumentation der
  Handsets prüfen.
- **Nur 5 GHz.** Ohne 2,4 GHz verlieren Clients, die nur 2,4 GHz können,
  die SSID. Der Dialog wählt diese Zeile standardmäßig nicht aus.
- **Änderungen durch andere.** Die UniFi-API hat keine Versionskennung.
  TalkOps liest die SSID direkt vor jeder Änderung neu und lehnt die
  Änderung ab, wenn sich die Einstellungen seit dem Vorschlag geändert
  haben.
- **Vollständiges Update.** Die API ersetzt das ganze SSID-Objekt. TalkOps
  liest die SSID direkt vor der Änderung und schickt alle anderen
  Einstellungen unverändert zurück, auch das Passwort. Liefert UniFi das
  Passwort nicht, schreibt TalkOps nicht.

### Rechte des API-Schlüssels

Der Verbindungstest schreibt nicht. Erst die erste Änderung zeigt, ob der
API-Schlüssel schreiben darf. Ein nur lesender Schlüssel führt zur Meldung
„Der UniFi-API-Schlüssel darf lesen, aber nicht schreiben“. Dann den
Schlüssel mit einem Administratorkonto erstellen, das die WLAN-Einstellungen
der Site ändern darf.

### Schreibzugriff einschalten

1. **Einstellungen → WLAN-Begehung → UniFi** öffnen.
2. **Verbindung testen** wählen. Bei Network 10.6.106 oder neuer zeigt der
   Status **Schreiben laut Version: möglich**.
3. Unter **VoIP-SSID und Schreibzugriff** **SSIDs lesen** wählen.
4. Die SSID der WLAN-Handsets auswählen.
5. **Schreibzugriff erlauben** wählen. Dafür ist die Berechtigung
   `unifi.write` nötig.
6. **Speichern** wählen.

Eine Änderung der Konsolenadresse oder des Site-Namens löscht die Auswahl
der VoIP-SSID und schaltet den Schreibzugriff aus.

### Änderung vorschlagen und anwenden

1. Das Ergebnis einer analysierten Begehung öffnen.
2. Unter **Empfehlungen** **Änderung vorschlagen** wählen. TalkOps liest
   die VoIP-SSID. Noch ändert sich nichts.
3. Der Dialog zeigt jede Einstellung mit aktuellem und neuem Wert.
   Unerwünschte Zeilen abwählen.
4. Die Warnungen lesen. Der Dialog nennt auch Empfehlungen, die TalkOps
   nicht schreibt, z. B. 802.11r, 802.11k, minimale Datenrate und
   Sendeleistung.
5. **Mir ist bewusst, dass Clients dieser SSID kurz getrennt werden**
   wählen.
6. **Anwenden** wählen.
7. **Neue Begehung zum Vergleich starten** wählen und beide Begehungen
   vergleichen.

| Empfehlung | Vorgeschlagene Änderung |
| --- | --- |
| Langsames Roaming (`fast_roaming`) | BSS Transition an; WPA3 Fast Roaming an, wenn 802.11r an ist |
| Klebender Client (`min_rssi`) | Handoff-Vorschlag für 5 GHz bei der Schwelle der Begehung (Standard -75 dBm, Grenzen -80 bis -60 dBm) |
| 2,4 GHz (`band_5ghz`) | 2,4 GHz entfernen (standardmäßig nicht ausgewählt) |
| VoIP-SSID (`voice_ssid`) | U-APSD an, Multicast zu Unicast an, ARP-Proxy an, DTIM 1 (2,4 GHz: 3, wenn UniFi es sperrt), PMF optional, wenn es aus ist |

### Änderungsverlauf und Rollback

**Einstellungen → WLAN-Begehung → WLAN-Änderungen** listet jede Änderung
mit Zeit, Benutzer, Einstellungen und Status. Ein Klick auf die Zeit zeigt
die Werte vor und nach der Änderung.

1. Bei einer Änderung **Zurücknehmen** wählen.
2. Die Werte prüfen, die TalkOps wiederherstellt.
3. **Jetzt zurücknehmen** wählen.

Hat jemand eine dieser Einstellungen nach der Änderung durch TalkOps in
UniFi geändert, zeigt der Dialog die Werte nach TalkOps, die aktuellen
Werte und die Werte, die der Rollback schreibt. **Trotzdem zurücknehmen**
nur wählen, wenn der Rollback diese Änderungen überschreiben soll.

Das Audit-Log protokolliert jedes Ein- und Ausschalten des Schreibzugriffs,
jede Änderung und jeden Rollback mit Einstellungen und Werten, nie mit dem
Passwort.

## Roaming-Schwelle der Handsets und Bandschutz

Auch das Handset entscheidet, wann es roamt. TalkOps provisioniert die
Roaming-Schwelle der Yealink AX83H und AX86R und schützt die Handsets vor
einer Bandänderung an der VoIP-SSID. WLAN-Name und Passwort der Handsets
provisioniert TalkOps nicht.

### Roaming-Schwelle

Fällt das Signal des aktuellen Access Points unter die Roaming-Schwelle,
sucht das Handset einen besseren Access Point. Parameter:
`static.network.wifi.roaming_threshold`, -100 bis -30 dBm, Werkseinstellung
-70 dBm (Yealink AX83H Wi-Fi Roaming Guide und Administrator's Manual).

- Für alle WLAN-Handsets unter **Einstellungen → Telefone (Provisionierung) →
  Komfort-Einstellungen → WLAN** setzen, oder für ein Handset auf seiner
  Telefonseite.
- Aus einer Begehung: Hat die Begehung einen klebenden Client gefunden
  (`min_rssi`), schlägt der Diff-Dialog die Schwelle der Begehung plus 5 dB
  vor, begrenzt auf -75 bis -60 dBm (Standard-Begehung: -80 → -75). Das
  Handset sucht dann, bevor der Access Point es wegdrückt.

Vorschlag anwenden:

1. Ergebnis der Begehung öffnen und **Änderung vorschlagen** wählen.
2. Im Abschnitt **Handset** aktuellen und neuen Wert prüfen.
3. Bestätigen und **Anwenden** wählen. Nötig sind `unifi.write` und
   `phones.manage`.

TalkOps speichert den Wert nur für dieses Handset und fordert es auf, seine
Konfiguration abzuholen (`check-sync`). Der Änderungsverlauf zeigt **wartet
auf Handset**, bis das Handset sie abgeholt hat. Ein nicht erreichbares
Handset erhält den Wert beim nächsten Provisionierungsintervall. Der Rollback
stellt den früheren Wert wieder her. Hat jemand den Wert seitdem geändert,
fragt der Rollback nach.

### Bandschutz

Bevor TalkOps ein Band von der VoIP-SSID entfernt (z. B. 2,4 GHz), liest
TalkOps das Band jedes provisionierten WLAN-Handsets aus UniFi:

| UniFi meldet | Ergebnis |
| --- | --- |
| Verbunden auf einem Band, das bleibt | OK |
| Mit der VoIP-SSID auf dem Band verbunden, das entfällt | Gesperrt: Das Handset würde die SSID verlieren |
| Nicht verbunden | Das Handset muss mit Namen bestätigt werden |

Ein Rollback, der ein Band entfernt, nutzt dieselbe Prüfung.

Die Band-Einstellung des Handsets provisioniert TalkOps noch nicht. Yealink
dokumentiert ihren Parameter nicht. Ein Konfigurationsexport zeigt den Namen
`static.network.wifi.band.mode`, die Bedeutung der Werte ist aber nicht
bestätigt. Das Handset auf
**WLAN-Band: 2.4G & 5G** lassen (es bevorzugt dann 5 GHz). Ein Handset mit
nur einem Band geht offline, wenn die VoIP-SSID dieses Band nicht mehr
ausstrahlt, und TalkOps kann es dann nicht mehr erreichen.

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
