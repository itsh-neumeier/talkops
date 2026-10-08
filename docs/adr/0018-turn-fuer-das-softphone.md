# 0018 – TURN für das Browser-Softphone

- Status: Angenommen
- Datum: 2026-10-08

## Kontext

Die Sprache des Browser-Softphones läuft per WebRTC direkt zwischen Browser
und FreeSWITCH (UDP, RTP-Portbereich). Hinter Reverse-Proxys, in Netzen mit
gesperrtem UDP oder unterwegs ohne VPN kommt sie nicht an. Ein HTTP-Proxy wie
Zoraxy oder Caddy kann Medienströme nicht transportieren.

## Optionen

1. Nur direkte Medien, Freigaben/VPN dokumentieren.
2. TURN-Server (coturn) als optionaler Dienst; Zugangsdaten aus TalkOps.
3. FreeSWITCH-Medien über TCP/WebSocket tunneln (nicht vorgesehen,
   proprietär).

## Entscheidung

Option 2. coturn (`coturn/coturn:*-trixie`) läuft als Compose-Profil `turn`
im Host-Netz. TalkOps gibt dem Softphone mit `/api/v1/me/webrtc` TURN-URLs
und Zugangsdaten nach dem „TURN REST API“-Schema: Benutzername
`<Ablaufzeit>:<Benutzer-ID>`, Passwort = Base64(HMAC-SHA1(Secret,
Benutzername)), 24 Stunden gültig; gespeichert wird nichts
(`talkops_core::turn`). coturn prüft mit `use-auth-secret`. coturn darf nur
zur FreeSWITCH-Adresse weiterleiten (`denied-peer-ip` alles,
`allowed-peer-ip` = `TALKOPS_TURN_PEER_IP`), damit es kein offenes Relais
ins LAN ist. Mit `TALKOPS_TURN_RELAY_ONLY` erzwingt das Softphone TURN.

Das Softphone fragt keinen öffentlichen STUN-Server mehr; ohne TURN ermittelt
TalkOps die Browser-Adresse selbst (mDNS-Kandidaten, siehe 1.1.2).

## Konsequenzen

- Ein Port (3478 UDP/TCP) genügt; per Router-Weiterleitung oder
  Stream-Proxy auch von außen nutzbar.
- Zusätzliche Latenz über das Relais, nur wenn der direkte Weg scheitert
  (oder erzwungen wird).
- TLS-TURN (`turns:`, 5349) ist noch nicht eingerichtet; die Medien selbst
  sind per DTLS-SRTP Ende-zu-Ende zwischen Browser und FreeSWITCH
  verschlüsselt.
