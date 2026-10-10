# Browser-Softphone (WebRTC)

Jeder Benutzer mit Nebenstelle kann direkt im Browser telefonieren – mit
Video, Halten, Stummschalten und Tastentönen. Das Softphone klingelt
zusammen mit den anderen Telefonen der Nebenstelle, solange die Seite
**Softphone** geöffnet ist.

## Voraussetzungen

- **HTTPS**: Browser geben Mikrofon und Kamera nur sicheren Seiten frei.
  Am einfachsten mit der Caddy-Option (`COMPOSE_PROFILES=caddy`,
  `TALKOPS_DOMAIN=pbx.example.com`, siehe [Installation](installation.md)).
  Auf dem TalkOps-Rechner selbst funktioniert auch `http://localhost:8080`.
- Der Benutzer braucht eine **Nebenstelle** (Admin: Nebenstelle → Benutzer).
- Aktuelles Chrome, Edge, Firefox oder Safari.

Beim ersten Öffnen legt TalkOps automatisch ein Gerät „Browser“ (Typ
*Browser*) an der Nebenstelle an. Es erscheint wie andere Geräte und kann
dort deaktiviert oder gelöscht werden.

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
aus). Interne Anrufe übertragen Video nur, wenn beide Nebenstellen es
eingeschaltet haben; Anrufe über einen Trunk nur, wenn zusätzlich am Trunk
*Videoanrufe über diesen Trunk* an ist (nur bei Anbietern mit Video). Sonst wird
nur Audio übertragen – auch über die Video-Taste, die das Softphone ohne
Freischaltung gar nicht erst anzeigt. Türsprechstellen senden ihr Bild immer.


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
