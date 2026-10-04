# 0002 – Telefonie-Engine: FreeSWITCH statt Asterisk oder reinem Rust-Stack

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

TalkOps braucht: SIP-Registrar für Tischtelefone, DECT, Softphones und
Türsprechstellen; mehrere SIP-Trunks (UDP/TCP/TLS, SRTP); **H.264-Video**
(Dahua VTO → Yealink-Videotelefone/Browser); **WebRTC** (SIP over WSS,
DTLS-SRTP); Aufzeichnung (Stereo, Kanal A/B); Konferenzen, Queues, Parken,
Pickup, Intercom; T.38-Fax; DTMF in allen Varianten. Zielgruppe sind Homelabs
und kleine Firmen – Betriebssicherheit ist wichtiger als Eigenbau.

## Optionen

1. **FreeSWITCH 1.10.x**
   - + Video-Passthrough (H.264) und Video-Konferenzen nativ, WebRTC über
     `mod_sofia`/`mod_rtc` integriert.
   - + Komplett per Netzwerk steuerbar: `mod_xml_curl` (Konfiguration aus HTTP)
     und Event Socket (Events + Call-Control). Passt zur Idee „Control Plane in Rust“.
   - + Stereo-Recording, `mod_callcenter`, `mod_conference`, `mod_valet_parking`,
     `mod_spandsp` (T.38) vorhanden. UniFi Talk basiert selbst auf FreeSWITCH.
   - − Kleinere Community als Asterisk, offizielle Pakete nur über SignalWire-Repo
     mit Token → wir bauen aus Quellcode.
2. **Asterisk (PJSIP) + ARI**
   - + Sehr große Community, viele Anleitungen, ARI ist eine gute REST-API.
   - − Video nur eingeschränkt (Passthrough ja, aber Konferenz/Transcoding schwach),
     WebRTC-Setup fragiler.
   - − Konfiguration klassisch dateibasiert; Realtime-Backends existieren, sind
     aber weniger flexibel als ein dynamisch generierter Dialplan per HTTP.
3. **Reiner Rust-Stack** (eigener SIP-Stack, RTP, Mixer, z. B. auf Basis
   `rsipstack`/`webrtc-rs`)
   - + Ein Binary, volle Kontrolle, keine C-Abhängigkeiten.
   - − SIP-Interop (NAT, Re-INVITE, PRACK, Session-Timer, Provider-Eigenheiten),
     Jitterbuffer, Codecs, SRTP, T.38, Video – Jahre an Arbeit, die FreeSWITCH
     bereits gelöst hat. Widerspricht „Robustheit vor Eigenbau“.

## Entscheidung

**FreeSWITCH 1.10.x** (aktuell **v1.10.12**) als Medien- und SIP-Engine.

- Build aus Quellcode im Multi-Stage-Dockerfile (`docker/freeswitch/Dockerfile`),
  Versionen von FreeSWITCH, sofia-sip (v1.13.17) und spandsp (fester Commit)
  sind gepinnt.
- Kuratierte Modulliste (`build-modules.conf`); nicht benötigte Module mit
  zusätzlichen Abhängigkeiten (`mod_verto`, `mod_signalwire`, `mod_lua`, …) entfallen.
- G.729 (`mod_g729`/bcg729) ist vorerst nicht enthalten, da in DACH G.711a,
  G.722 und Opus dominieren; Nachrüstung bei Bedarf über die Modulliste.
- FreeSWITCH bleibt „dumm“: siehe [ADR 0006](0006-steuerung-xml-curl-und-esl.md).

## Konsequenzen

- Der FreeSWITCH-Image-Build dauert 10–40 Minuten; CI baut es nur bei
  Änderungen unter `docker/freeswitch/`, Releases bauen arm64 nativ.
- Updates von FreeSWITCH sind bewusste Schritte (Version-Bump + Integrationstest).
- Lizenz: FreeSWITCH steht unter MPL 1.1; das Image wird unverändert aus den
  Upstream-Quellen gebaut.
