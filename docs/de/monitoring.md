# Monitoring

TalkOps stellt seinen Zustand im Prometheus-Textformat unter `/metrics` bereit.

## Übersicht in der Weboberfläche

Die Startseite zeigt Operatoren und Admins Kennzahlen (Anrufe, verpasste
Anrufe, Annahmequote, Ø Gesprächsdauer), ein Diagramm der Anrufe nach
Richtung für die letzte Stunde, den letzten Tag, die letzte Woche oder den
letzten Monat (Tage in der Zeitzone aus den Einstellungen), laufende
Gespräche, die letzten Anrufe und den Systemstatus (Dienste, registrierte
Geräte, Trunks, letzte Datensicherung). Sie aktualisiert sich alle 10 Sekunden.

## Aktivieren

Ein langes Zufallstoken in der `.env` (bzw. der Stack-Umgebung in Portainer)
setzen und den Stack neu starten:

```sh
TALKOPS_METRICS_TOKEN=$(openssl rand -hex 32)
```

Ohne Token antwortet der Endpunkt mit `404`, ohne oder mit falschem Token mit
`401`.

## Prometheus

```yaml
scrape_configs:
  - job_name: talkops
    scheme: https          # http, wenn das Caddy-Profil nicht genutzt wird
    authorization:
      credentials: <TALKOPS_METRICS_TOKEN>
    static_configs:
      - targets: ["pbx.example.com"]
```

## Metriken

| Metrik | Bedeutung |
| --- | --- |
| `talkops_build_info{version}` | Laufende Version (immer 1) |
| `talkops_database_up` | 1, wenn die Datenbank antwortet |
| `talkops_freeswitch_connected` | 1, wenn TalkOps mit dem Event Socket von FreeSWITCH verbunden ist |
| `talkops_registrations` | Registrierte SIP-Geräte |
| `talkops_trunk_registered{gateway}` | 1, wenn das Trunk-Konto beim Anbieter registriert ist |
| `talkops_trunk_up{gateway}` | 1, wenn der Trunk auf OPTIONS-Pings antwortet |
| `talkops_active_calls` | Laufende Gespräche (nur bei Verbindung zu FreeSWITCH) |
| `talkops_calls_total{direction}` | Beendete Anrufe (`inbound`, `outbound`, `internal`) |
| `talkops_calls_answered_total{direction}` | Angenommene Anrufe |
| `talkops_recordings`, `talkops_recordings_bytes` | Gespeicherte Aufzeichnungen und ihre Größe |
| `talkops_voicemail_new` | Ungehörte Voicemail-Nachrichten |
| `talkops_jobs{kind,status}` | Hintergrundjobs (Sprachausgabe, Transkription, Mail), die warten, laufen oder fehlgeschlagen sind |
| `talkops_door_station_online{name}` | 1, wenn der Ereignis-Stream einer Türsprechstelle verbunden ist |

Trunk-Metriken tragen den FreeSWITCH-Gateway-Namen des jeweiligen Trunk-Kontos
(`gw-<Konto-ID ohne Bindestriche>`).

## Beispiel-Alarme

```yaml
groups:
  - name: talkops
    rules:
      - alert: TalkOpsFreeSwitchDown
        expr: talkops_freeswitch_connected == 0
        for: 2m
      - alert: TalkOpsTrunkUnregistered
        expr: talkops_trunk_registered == 0
        for: 10m
      - alert: TalkOpsDoorStationOffline
        expr: talkops_door_station_online == 0
        for: 10m
      - alert: TalkOpsJobsFailing
        expr: delta(talkops_jobs{status="failed"}[1h]) > 0
```

## Fehlersuche in der Oberfläche

**Einstellungen → System** (Admin):

- **Status**: Version, Laufzeit von TalkOps und FreeSWITCH, aktive Kanäle.
- **Debug-Logging**: Stufe (Debug/Info/Notice/Warning), Dauer in Minuten
  (1–60) und optional **SIP-Trace** wählen, *Starten*, das Problem nachstellen.
  Das Log läuft live mit (filterbar) und lässt sich als Textdatei
  herunterladen – die Datei an die Support-Anfrage hängen. Der Mitschnitt
  endet nach der gewählten Zeit von selbst und schaltet den SIP-Trace wieder
  ab. Das entspricht `fs_cli -x "sofia global siptrace on"` mit `/log debug`,
  ohne Zugriff auf den Server.
- **Aktive Kanäle**: wie `show channels` – Kanal, Anrufer, Ziel, Status,
  Codec, Anwendung.
- **Sitzungen beim Anbieter**: *Alle Gespräche beenden* (BYE für jedes
  Gespräch), *Trunks ab- und anmelden* oder beides. Hilft, wenn der Anbieter mit
  „403 Too many simultaneous sessions“ ablehnt. Trunk-Gespräche nutzen
  Session-Timer (600 s), damit der Anbieter verwaiste Gespräche selbst beendet.
- **Neustart**: *FreeSWITCH neu starten* oder *Alle Dienste neu starten*
  (FreeSWITCH, Media-Worker, TalkOps). Die Dienste beenden sich und Docker
  startet sie dank `restart: unless-stopped` neu; laufende Gespräche werden
  getrennt.
