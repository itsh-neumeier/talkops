# 0006 – FreeSWITCH-Steuerung über mod_xml_curl und Event Socket

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

Die gesamte Konfiguration (User, Geräte, Trunks, Routing, Zeitsteuerung)
wird im Web-UI gepflegt und liegt in PostgreSQL. FreeSWITCH muss diese Daten
nutzen, ohne dass XML-Dateien gepflegt, generiert oder synchronisiert werden.
Anruflogik (IVR, Voicemail, Türöffner, Recording-Steuerung) soll in Rust
liegen und testbar sein.

## Optionen

1. **Generierte XML-Dateien + `reloadxml`** – einfach, aber Race Conditions,
   Dateisystem-Kopplung zwischen Containern, kein Zugriff auf Laufzeitkontext.
2. **`mod_xml_curl`** – FreeSWITCH fragt Directory/Dialplan/Konfiguration
   per HTTP an; Antwort wird zur Laufzeit aus der DB generiert.
3. **Lua/JavaScript im FreeSWITCH** (`mod_lua`, `mod_v8`) – Logik im
   FreeSWITCH-Prozess, zweite Codebasis und Sprache, schwer testbar.
4. **Event Socket inbound/outbound** – Events empfangen, API-Kommandos
   senden; outbound: pro Anruf verbindet FreeSWITCH sich mit unserem Server
   und übergibt die Steuerung.

## Entscheidung

Kombination aus 2 und 4, ohne eingebettete Skriptsprachen:

- **Bootstrap-Konfiguration** im Image (`docker/freeswitch/conf`): nur Module,
  Event Socket, xml_curl-Binding, RTP-Portbereich. Umgebungsabhängige Werte
  setzt `docker-entrypoint.sh` (`vars_env.xml`).
- **`mod_xml_curl`** mit Bindings `directory|dialplan|configuration` auf
  `POST /fs/xml` (HTTP Basic Auth, Timeout 3 s). TalkOps antwortet aus der DB
  oder mit `not found` (→ Fallback auf Bootstrap). Insbesondere `sofia.conf`
  (SIP-Profile, Trunks als Gateways) kommt dynamisch.
- Der **Dialplan** ist pro Anruf eine kleine generierte Extension; die
  Routing-Entscheidung (Rufgruppe, Zeitsteuerung, Weiterleitung …) fällt in Rust.
- **ESL inbound** (eine dauerhafte Verbindung, Reconnect mit Backoff,
  nur `127.0.0.1`): Events (Channel-Lifecycle, `sofia::register`, Presence),
  API-Kommandos (`uuid_*`, `sofia profile … rescan/killgw`, NOTIFY).
- **ESL outbound** (ab Phase 3): interaktive Abläufe (Voicemail, IVR,
  Türöffner per DTMF) über `socket 127.0.0.1:8084 async full`.
- **`mod_xml_cdr`** als zweiter, zuverlässiger CDR-Kanal (HTTP-POST mit
  Retry), damit CDRs eine ESL-Unterbrechung überstehen.

## Konsequenzen

- TalkOps muss vor FreeSWITCH bereit sein (Compose: `depends_on:
  service_healthy`); ist TalkOps später kurz weg, laufen bestehende Gespräche
  weiter, neue Lookups schlagen nach 3 s fehl.
- Änderungen im UI wirken sofort für neue Lookups; für SIP-Profile/Gateways
  sendet TalkOps `sofia profile <p> rescan` bzw. `killgw` per ESL.
- Die xml_curl-Antworten sind die zentrale Schnittstelle und werden mit
  Snapshot-Tests der erzeugten XML abgesichert (Phase 1).
