# 0015 – Browser-Softphone: SIP über WebSocket durch TalkOps, SIP.js

- Status: Angenommen
- Datum: 2026-10-07

## Kontext

Phase 7 verlangt ein Softphone mit Video im Browser. FreeSWITCH kann
WebRTC (DTLS-SRTP, ICE) über SIP-über-WebSocket (`mod_sofia`) oder Verto.
Browser verlangen HTTPS für Mikrofon/Kamera und `wss://` von HTTPS-Seiten.

## Optionen

1. **Verto** (`mod_verto`, JSON-RPC) – eigenes Protokoll, eigene Clients.
2. **SIP über WebSocket** direkt an FreeSWITCH (`wss-binding`) – braucht ein
   Zertifikat in FreeSWITCH und einen weiteren offenen Port.
3. **SIP über WebSocket, durch TalkOps weitergereicht** – FreeSWITCH bindet
   `ws` nur an localhost, TalkOps reicht die Verbindung für angemeldete
   Benutzer unter derselben Origin durch; TLS macht Caddy/der Proxy.

## Entscheidung

Option 3 mit SIP.js (`SimpleUser`) im Browser.

- Gleiche Origin, gleiches Zertifikat, Zugriff nur mit Sitzung
  (SameSite=Strict-Cookie, zusätzlich Origin-Prüfung); FreeSWITCHs
  WebSocket ist von außen nicht erreichbar.
- Je Benutzer und Nebenstelle ein Gerät vom Typ `browser`, automatisch
  angelegt; Zugangsdaten nur für den Besitzer.
- Das interne Profil nutzt `inbound-late-negotiation`, damit Browser
  (OPUS/VP8 bevorzugt) Telefone ohne OPUS erreichen und Codecs möglichst
  ohne Transcoding gewählt werden.
- Aufnahmen starten per `execute_on_answer`, nicht vor dem `bridge`
  (sonst blockiert der Medienaufbau eines WebRTC-Anrufers).

## Konsequenzen

- Medien laufen direkt zwischen Browser und FreeSWITCH (UDP); ohne VPN von
  unterwegs fehlt TURN – später nachrüstbar (coturn-Option).
- Kein Video-Transcoding: Video zu Tischtelefonen braucht H.264 im Browser.
- E2E-Test mit Playwright/Chromium (falsche Medien) gegen echtes
  FreeSWITCH.
