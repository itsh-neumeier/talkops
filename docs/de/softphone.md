# Browser-Softphone (WebRTC)

Das Softphone unterstützt Audio, Video, Halten, Stummschalten und Tastentöne.
Nach dem ersten Öffnen klingelt es mit den anderen Telefonen der gewählten
Nebenstelle und bleibt beim Navigieren innerhalb von TalkOps verbunden.

## Nebenstelle auswählen und erreichbar bleiben

Administratoren wählen unter **Telefonie-Identität** eine aktive Nebenstelle
des eigenen Mandanten. Eine feste Zuordnung zum Admin-Konto ist nicht nötig.
Nutzer und Operatoren verwenden ausschließlich ihre zugewiesenen aktiven
Nebenstellen. Bei einer Nebenstelle erfolgt die Auswahl automatisch; bei
mehreren erscheint ein Dropdown.

Die Auswahl zeigt Nebenstelle, Name und ausgehende Rufnummer. Die
Standardrufnummer ist gekennzeichnet. Ohne ausgehende Route sind externe
Anrufe nicht möglich; interne Anrufe können weiterhin funktionieren.
Rufnummerunterdrückung und Notrufrouting verwenden die bisherigen Regeln.

Auch eingehende Anrufe folgen der ausgewählten Nebenstelle. Die Auswahl
ändert weder den Besitzer noch die Einstellungen der Nebenstelle. Während
eines Gesprächs ist kein Wechsel möglich. Admin und Nutzer erhalten getrennte
Browser-SIP-Konten. Vorhandene Zugangsdaten des zugewiesenen Nutzers bleiben
beim Upgrade erhalten.

Nach dem ersten Öffnen bleibt das Softphone beim Navigieren in TalkOps aktiv.
Ein kleines Fenster zeigt Status und Gespräch und erlaubt Annehmen,
Ablehnen, Stummschalten und Auflegen. Abmelden, Neuladen oder Schließen des
Tabs trennt die Verbindung. Auch ein vom Browser angehaltener Tab kann die
Erreichbarkeit verlieren. Nach einem Verbindungsabbruch versucht TalkOps die
Wiederanmeldung; bei Bedarf **Neu verbinden** verwenden.

## Audio, Kontakte und letzte Anrufe

Unter **Audiogeräte und Tests** lassen sich Mikrofon und Ausgabe wählen.
Mikrofonzugriff erlauben, um Gerätenamen zu sehen. Der Mikrofontest zeigt den
Pegel und endet beim Anrufen oder Verlassen der Softphone-Ansicht.
Mikrofonänderungen gelten für den nächsten Anruf. **Ton testen** prüft die
gewählte Ausgabe. Unterstützt der Browser die Ausgabewahl nicht, erfolgt sie
im Betriebssystem. Bei getrennten Geräten wird auf den Systemstandard
zurückgegriffen; den Ton vor dem nächsten Gespräch prüfen.

**Hörbarer Klingelton** gilt nur für diesen Browser. Er aktiviert nicht
„Nicht stören“ für die Nebenstelle. Bei blockierter Wiedergabe **Klingelton
aktivieren / Ton testen** anklicken. Anrufbenachrichtigungen sind optional
und benötigen die Browser-Erlaubnis. Ein geschlossener Tab empfängt dadurch
keine Anrufe.

Das Telefonbuch lässt sich nach Name, Firma und Nummer durchsuchen.
Sterne markieren Favoriten und sortieren sie nach oben. Eine Nummer übernimmt
sie ins Wählfeld; die Telefontaste ruft direkt an. Favoriten und
Audioeinstellungen werden je Nutzer lokal in diesem Browser gespeichert.

**Letzte Anrufe** zeigt bis zu 30 Anrufe der gewählten Nebenstelle.
Der Filter **Verpasste eingehende Anrufe** zeigt nicht beantwortete eingehende Anrufe.
Ausgehende erfolglose Anrufe erscheinen als **Nicht beantwortet**.
Rückrufe verwenden die Nummer der Gegenstelle. **Aktualisieren** lädt die
Kontakte und Anrufliste erneut.

## Voraussetzungen

- **HTTPS**: Browser geben Mikrofon und Kamera nur sicheren Seiten frei.
  Am einfachsten mit der Caddy-Option (`COMPOSE_PROFILES=caddy`,
  `TALKOPS_DOMAIN=pbx.example.com`, siehe [Installation](installation.md)).
  Auf dem TalkOps-Rechner selbst funktioniert auch `http://localhost:8080`.
- Nutzer und Operatoren brauchen eine zugewiesene **Nebenstelle**
  (Admin: Nebenstelle → Benutzer). Administratoren können ohne feste
  Zuordnung eine aktive Nebenstelle auswählen.
- Aktuelles Chrome, Edge, Firefox oder Safari.

Beim ersten Öffnen legt TalkOps ein eigenes Gerät vom Typ *Browser* je Nutzer
und gewählter Nebenstelle an. Es kann dort deaktiviert oder gelöscht werden.

## Netzwerk

- Die Signalisierung (SIP über WebSocket) läuft über TalkOps selbst
  (`/api/v1/webrtc/ws`, nur für angemeldete Benutzer); FreeSWITCH lauscht
  dafür nur auf `127.0.0.1:5066`.
- Sprache und Video gehen direkt zwischen Browser und FreeSWITCH (UDP,
  RTP-Portbereich aus `.env`). Im lokalen Netz und über VPN funktioniert
  das ohne weitere Einstellungen. **Unterwegs ohne VPN** oder bei gesperrtem UDP
  hilft der TURN-Server (siehe unten).

## Bedienen mit der Tastatur

Ziffern, `*` und `#` – auch über den Nummernblock – wählen; im Gespräch
werden sie als Tastentöne gesendet (z. B. für Sprachmenüs). **Rücktaste**
löscht, **Enter** ruft an bzw. nimmt einen Anruf an, **Esc** legt auf,
lehnt ab oder leert das Wählfeld.

## Hinter einem Reverse-Proxy (z. B. Zoraxy)

- WebSocket-Weiterleitung einschalten, Timeout mindestens 3600 s und
  „bei Aktivität verlängern“, zum Ziel HTTP/1.1.
- Den Original-Host weitergeben oder `X-Forwarded-Host: <Domain>` setzen,
  sowie `X-Forwarded-For` (Adresse des Browsers für die Sprachverbindung).
- Keine eigene Permission-Policy setzen – TalkOps erlaubt Mikrofon und
  Kamera selbst.

## TURN-Server (Sprache über einen Port)

Ohne TURN läuft die Sprache direkt per UDP auf den RTP-Ports (16384–16999).
Ist das nicht möglich – unterwegs ohne VPN, strenge Firewall, Gäste-WLAN –,
leitet ein TURN-Server (coturn) sie über **einen** Port (3478) weiter.
TalkOps bringt coturn als optionalen Dienst mit und erzeugt für jeden
Softphone-Login eigene, 24 Stunden gültige Zugangsdaten.

1. In der `.env` bzw. den Stack-Variablen setzen:
   ```sh
   COMPOSE_PROFILES=turn
   TALKOPS_TURN_SECRET=$(openssl rand -hex 32)   # langer Zufallswert
   TALKOPS_TURN_PEER_IP=192.168.140.30           # LAN-Adresse dieses Servers
   TALKOPS_TURN_URLS=turn:talk.example.de:3478?transport=udp,turn:talk.example.de:3478?transport=tcp
   ```
   `TALKOPS_TURN_URLS` ist die Adresse, unter der Browser coturn erreichen
   (Domain oder IP).
2. Stack neu deployen; im Log steht `TURN enabled for softphones`.
3. Erreichbarkeit: Port **3478 (UDP und TCP)** zum Server freigeben – im LAN
   in der Firewall des Servers, für unterwegs zusätzlich als Portweiterleitung
   im Router. Mit Zoraxy geht das als **Stream Proxy** (TCP/UDP 3478 →
   `192.168.140.30:3478`).
4. Optional `TALKOPS_TURN_RELAY_ONLY=true`: Die Sprache läuft dann immer über
   TURN, auch wenn der direkte Weg ginge (zum Testen oder bei gesperrtem UDP).

coturn leitet nur zu `TALKOPS_TURN_PEER_IP` weiter, nie in den Rest des
Netzes. Die Relay-Ports 49160–49200 werden nur intern zwischen coturn und
FreeSWITCH genutzt und müssen nicht freigegeben werden.

## Rufnummer einmalig unterdrücken

Der Haken *Rufnummer einmalig unterdrücken* unter den Wähltasten gilt nur für
den nächsten Anruf: TalkOps wählt dann mit `*31` davor (auch vor
SIP-Adressen), danach ist der Haken wieder aus und die Nummer sichtbar.
Dauerhaft unterdrücken lässt sie sich an der Nebenstelle.

## Im Gespräch

- **Anzeige**: Bei eingehenden Anrufen zeigt das Softphone Name und Nummer
  des Anrufers, im Gespräch die Gegenstelle, die Dauer und „gehalten“.
  Nach dem Auflegen wird das Wählfeld geleert; **Anrufen** mit leerem Feld
  wählt die letzte Nummer erneut.
- **Halten**: Die Gegenstelle hört die Wartemusik (Einstellungen →
  *Wartemusik*: alle mitgelieferten Stücke, ein bestimmtes Stück oder
  eigene Musik/Ansage).
- **Konferenz**: Im Gespräch **Konferenz** wählen, Nummer eingeben,
  **Hinzufügen**. Das Gespräch wird zur Konferenz, der neue Teilnehmer
  wird angerufen (intern oder extern, wie ein normaler Anruf der eigenen
  Nebenstelle). Weitere Teilnehmer lassen sich genauso hinzufügen. Legt der
  Initiator auf, endet die Konferenz für alle.
- **Tastentöne** (Voicemail, Sprachmenüs) gehen als RTP-Telefonie-Events
  (RFC 2833) hinaus.
- **Rufnummer unterdrücken** für einen Anruf: `*31` oder `#31#` vor die
  Nummer setzen. Dauerhaft: Nebenstelle → *Rufnummer unterdrücken*.

## Video

Video ist pro Nebenstelle freizuschalten (Nebenstelle → *Videoanrufe*, Standard
aus). Ein Anruf kann nur Video nutzen, wenn der Anrufer Video sendet. TalkOps
bietet Video dann nur den Telefonen an, die der Anruf erlaubt:

| Anruf | Video |
| --- | --- |
| Nebenstelle zu Nebenstelle | wenn beide Nebenstellen Video haben |
| Nebenstelle zu Ruf-/Klingelgruppe | an die Mitglieder mit Video |
| Türsprechstelle | an alle angerufenen Telefone (lokaler Anruf) |
| Von oder zu einem Trunk | nur wenn der Trunk *Videoanrufe über diesen Trunk* hat und die Nebenstelle Video hat |
| Warteschlangen, Voicemail, Sprachmenüs, Konferenzen, SIP-Adressen ohne Trunk | nie |

Die meisten Anbieter unterstützen kein Video. Dort *Videoanrufe über diesen
Trunk* ausgeschaltet lassen. Dann fragt ein Videotelefon bei Anrufen von
außen nie, ob es mit Video annehmen soll. Ein WLAN-Handset mit dem
Codec-Profil *HD* oder *Robust* erhält kein Video.

Das Softphone zeigt die Video-Taste nur bei einer Nebenstelle mit Video und
*Mit Video annehmen* nur, wenn der eingehende Anruf Video enthält.


Video zu Tischtelefonen (z. B. Yealink T58W, VP59) und Türsprechstellen
nutzt H.264; Chrome, Edge, Firefox und Safari bieten H.264 an. Zwischen zwei
Browsern läuft auch VP8.

## Fehlersuche

| Anzeige | Ursache |
|---|---|
| Hinweis auf sichere Verbindung | Seite über HTTP statt HTTPS geöffnet |
| „getrennt“ | TalkOps oder FreeSWITCH nicht erreichbar; Seite neu laden |
| Klingelt, aber kein Ton | UDP/RTP zwischen Browser und Server blockiert (Firewall, Netz ohne VPN) |
| Kein Mikrofon | Browser-Berechtigung für die Seite prüfen |

Für genauere Fehlersuche in der Browser-Konsole `localStorage['talkops.sipDebug'] = '1'`
setzen und die Seite neu laden: Das Softphone protokolliert dann alle SIP-Nachrichten.
