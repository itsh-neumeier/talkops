# 0013 – Türsprechstellen: Dahua VTO als SIP-Gerät, Steuerung über die HTTP-API

- Status: Angenommen
- Datum: 2026-10-06

## Kontext

Phase 6 bindet Dahua-VTO-Türstationen an: Klingeln mit Video aufs
Telefon, Tür öffnen (Telefon, Web, Home Assistant), Schnappschüsse und ein
Ereignisprotokoll. Dahua dokumentiert seine HTTP-API nur für Partner (NDA);
öffentlich sind Kurzanleitungen und Handbücher (SIP-Einstellungen) sowie
Open-Source-Integrationen (z. B. rroller/dahua, DahuaVTO2MQTT), die die
HTTP-CGI-Schnittstelle seit Jahren nutzen.

## Optionen

1. **VTO als normales SIP-Gerät** (Dritt-Server-Modus) einer Nebenstelle,
   Türöffner/Bilder/Ereignisse über die HTTP-CGI-API (Digest-Auth).
2. **Dahuas eigenes Protokoll** (DHIP, TCP 5000) für Ereignisse und
   Steuerung – mächtiger, aber binär, undokumentiert, fehleranfällig.
3. **Dahua-SIP-Server-Modus** (VTO als Anlage) mit TalkOps als Gegenstelle –
   widerspricht „TalkOps ist die Anlage“.

## Entscheidung

Option 1.

- Die VTO ist ein Gerät (Typ `door`) einer Nebenstelle. Jeder Anruf dieser
  Nebenstelle ist ein Klingeln und geht an ein konfiguriertes Ziel (die
  gleichen Ziele wie überall, `route_to`); Mehrtasten-Stationen
  bilden gewählte Nummern auf eigene Ziele ab.
- Video (H.264) läuft durch FreeSWITCH (`mod_h26x`, kein Transcoding,
  kein Bypass), damit auch NAT- und Codec-Eigenheiten der VTO abgefangen sind.
- HTTP-API (`talkops-doorbell`): `accessControl.cgi?action=openDoor`,
  `snapshot.cgi`, `eventManager.cgi?action=attach` (multipart, Heartbeat),
  `magicBox.cgi?action=getDeviceType` als Verbindungstest. Digest-Auth mit
  wiederverwendeter Challenge, Basic als Rückfall. Je Station ein Listener
  mit Backoff; Online-Status aus dem Stream.
- Tür öffnen: `*85`/`*86` (ESL-Outbound-App mit Ansage), Web-UI, Token-Hook
  `/hooks/door/<id>/open` (Bearer, nur SHA-256 gespeichert). DTMF während des
  Gesprächs reicht FreeSWITCH an die VTO durch (deren eigener Code).
- Ereignisse (`door_events`) mit Schnappschuss beim Klingeln, Webhook je
  Station (verschlüsselt gespeichert, enthält oft ein Geheimnis). Bilder
  zeigen Personen: Aufbewahrung wie Gesprächsaufnahmen.

## Konsequenzen

- Kein eigener Medienserver und keine Abhängigkeit von Dahua-Clouddiensten;
  jede SIP-fähige Türstation klingelt, HTTP-Funktionen sind Dahua-spezifisch.
- Die CGI-Aufrufe sind nicht offiziell spezifiziert; Firmware-Unterschiede
  (Pfadvarianten, fehlende Ereignisse) sind möglich. Tests laufen gegen einen
  nachgebauten Server; ein Test mit echter Hardware steht aus.
- Livebild im Browser ist ein Schnappschuss-Intervall (2 s), kein Stream;
  echtes Video im Browser kommt mit WebRTC (Phase 7).
- Der Ereignis-Stream nutzt die CGI-Variante, nicht DHIP: Klingel-Ereignisse
  erkennt TalkOps zuverlässig am SIP-Anruf, nicht am Stream.
