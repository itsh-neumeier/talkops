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
  das ohne weitere Einstellungen. **Unterwegs ohne VPN** braucht es eine
  öffentliche IP mit freigegebenen RTP-Ports bzw. einen TURN-Server – das
  ist noch nicht eingebaut.

## Video

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
