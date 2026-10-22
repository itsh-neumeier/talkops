# UniFi voice SSID changes from the Wi-Fi survey: analysis and development prompt

Status: Prepared for a development chat. Not implemented.
Date: 2026-10-22 (Europe/Berlin).
Base: `origin/main`, `c093fa8` (TalkOps 1.8.6).
Task summary: [20261022-claude-unifi-voice-ssid-write-prompt](../ai-memory/20261022-claude-unifi-voice-ssid-write-prompt.md).

## Goal

The Wi-Fi survey (ADR 0041) gives recommendations for the voice SSID.
This plan prepares stage 4a: TalkOps proposes a change of the UniFi voice
SSID for a recommendation. After the user confirms, TalkOps applies it,
with a snapshot and a rollback.

## Analysis

### py-unifi is not the integration path

- `ubiquiti-community/py-unifi` is a Python client that the OpenAPI
  generator made from the community specification `unifi-api`. It covers
  the unofficial classic API (`rest/wlanconf`, `rest/device`) of Network
  8.0.26. The last commit is from January 2024.
- TalkOps uses Rust for the control plane. A Python dependency does not
  agree with the architecture.
- Decision: do not use py-unifi. Do not write through the classic API.

### The official Integration API can write Wi-Fi broadcasts

The current version is UniFi Network 11.0.x. It needs UniFi OS 6.0 or later.
The Integration API has
`PUT /proxy/network/integration/v1/sites/{siteId}/wifi/broadcasts/{id}`.
A community client that its authors verified against 10.6.106 and 11.0.89
documents these fields:

| Survey recommendation | Integration API field | Minimum version |
| --- | --- | --- |
| `fast_roaming` (802.11v) | `bssTransitionEnabled` | 10.x |
| `fast_roaming` (802.11r) | `securityConfiguration.wpa3FastRoamingEnabled` (WPA3 only) | 10.x |
| `min_rssi` (sticky client) | `handoffSuggestionsConfiguration` | 10.6.106 |
| `band_5ghz` | `broadcastingFrequenciesGHz` | 10.x |
| `voice_ssid` | `uapsdEnabled`, `multicastToUnicastConversionEnabled`, `arpProxyEnabled`, `pmfMode`, DTIM override for each frequency | 10.x |

Limits:

- A `PUT` replaces the full object. TalkOps must read the object, change
  the allowed fields and send the full object back.
- The object contains the Wi-Fi passphrase. Keep it in memory only. Do not
  store, log, audit or show it.
- From 10.6.106, `channel2gLockedTo6` and `dtimPeriod2gLockedTo3` are
  required in each update.
- The official API does not have 802.11r for WPA2, 802.11k or a minimum
  data rate. These stay hints.
- Access point radio settings (`txPowerMode` from 11.0.89, minimum RSSI)
  apply to all SSIDs of an access point. These stay hints in stage 4a.
- A UniFi API key has the permissions of the admin who created it. Use a
  dedicated admin with Network permissions only.
- A change of the SSID provisions all access points of the SSID again.
  The clients of the SSID disconnect for a short time.
- The field names come from community sources and from the reference for
  v10.0.162. Stage 0 must confirm them on the real 11.0 console.

## Development prompt

Copy this prompt into a new development chat.

```text
##DEV PROMT START
Du arbeitest als Entwicklungsassistent an TalkOps.
Lies AGENTS.md, CLAUDE.md und docs/release-workflow.md.
Übernimm den Arbeitsablauf aus docs/development-chat-prompt.md.
Claude Code und Codex arbeiten parallel. Nutze für diese Aufgabe
einen eigenen Git-Worktree und Feature-Branch. Prüfe vorher den
aktuellen Remote-Zielstand und vorhandene Änderungen. Bewahre
fremde Arbeitsstände.
Setze die Aufgabe vollständig um, ergänze passende Tests und
Dokumentation und führe die relevanten Prüfungen aus.
Triff gewöhnliche technische Entscheidungen selbst.
Committe die fertigen Änderungen, pushe deinen Feature-Branch
und erstelle eine PR. Diese Schritte sind autorisiert.
Dokumentiere Testergebnisse, Abhängigkeiten, Migrations-IDs
und vorgeschlagene Release Notes in der PR.
Merges in den Zielbranch, Release-Versionen, Tags und
Veröffentlichung übernimmt der separate Release-Chat.

Aufgaben:
WLAN-Begehung Stufe 4a: Empfehlungen der Begehung als Änderung an der
UniFi-VoIP-SSID vorschlagen und nach Bestätigung anwenden, mit Snapshot
und Rollback. Lesen UND Schreiben ausschließlich über die offizielle
UniFi Network Integration API (/proxy/network/integration/v1/...,
X-API-KEY). Keine Schreibzugriffe über die Classic API.

Kontext: docs/plans/unifi-voice-ssid-write-dev-prompt.md (Analyse),
docs/wlan-voice-survey-proposal.md ("Stage 4"), ADR 0041,
docs/ai-memory/20261022-claude-wifi-voice-survey.md, crates/talkops-unifi,
crates/talkops-core/src/wifi_survey/analysis.rs (Empfehlungs-IDs).
Referenz: developer.ui.com Network API (Wifi Broadcasts), Feldübersicht
https://github.com/Art-of-WiFi/unifi-network-application-api-client
(verifiziert gegen 10.6.106 und 11.0.89). py-unifi (Network 8.0.26,
Classic API) ist veraltet und wird nicht verwendet.

1. Neues ADR (ergänzt ADR 0041): Schreibzugriff nur über
   PUT /v1/sites/{siteId}/wifi/broadcasts/{id} der offiziellen API,
   standardmäßig aus, Mindestversion Network 10.6.106, Feld-Allowlist,
   Snapshot, Rollback, ausdrückliche Bestätigung, kein Write während
   einer aktiven Begehung.
2. talkops-unifi: WLAN-Liste lesen (GET wifi/broadcasts), VoIP-SSID
   auswählen. Separates Schreibmodul mit Read-Modify-Write (PUT
   ersetzt das ganze Objekt). Allowlist:
   bssTransitionEnabled, securityConfiguration.wpa3FastRoamingEnabled,
   handoffSuggestionsConfiguration, broadcastingFrequenciesGHz,
   uapsdEnabled, multicastToUnicastConversionEnabled, arpProxyEnabled,
   pmfMode, DTIM-Override pro Frequenz. Alle anderen Felder, inklusive
   versionsabhängiger Pflichtfelder (channel2gLockedTo6,
   dtimPeriod2gLockedTo3), unverändert zurückschreiben.
   Der GET-only-Test gilt weiter für den Lesepfad; ein neuer Test
   erzwingt, dass der Schreibpfad nur PUT auf wifi/broadcasts sendet
   und nur Allowlist-Felder ändert.
3. Mapping Empfehlung → Vorschlag: fast_roaming (BSS Transition,
   WPA3 Fast Roaming), min_rssi (Handoff Suggestions), band_5ghz
   (Frequenzen), voice_ssid (U-APSD, Multicast→Unicast, ARP-Proxy,
   DTIM, PMF). Nur Hinweis, nicht schreibbar: 802.11r bei WPA2,
   802.11k, minimale Datenrate, AP-Sendeleistung (txPowerMode),
   pingpong, coverage.
4. Daten (Migration): VoIP-SSID-ID an der UniFi-Verbindung, Schalter
   "Schreibzugriff erlauben", neue Permission unifi.write (sensibel,
   nur Admin), Tabelle der Änderungen (Snapshot vorher/nachher nur der
   Allowlist-Felder, Status, Survey-Bezug, Benutzer, Zeit).
   Passphrase und andere Secrets nur im Speicher; nie speichern,
   loggen, auditieren oder an die UI geben.
5. API + UI: auf der Ergebnisseite "Änderung vorschlagen" →
   Diff-Dialog (aktueller/neuer Wert, Warnung "Clients der SSID werden
   kurz getrennt", Kompatibilitätshinweis Fast Roaming) → Anwenden →
   Änderungsverlauf mit Rollback (erkennt Fremdänderungen seit dem
   Snapshot und fragt nach) → "Neue Begehung zum Vergleich starten".
   Audit-Eintrag für Anwenden und Rollback, ohne Secrets.
   Texte in en.ts und de.ts, docs/ui-guidelines.md beachten.
6. Verbindungstest meldet: Version, ob wifi/broadcasts lesbar ist,
   welche Allowlist-Felder diese Version liefert und ob Schreiben
   versionsseitig möglich ist. Kein Test-Write ohne Bestätigung.
7. Tests: axum-Mock mit Antworten im Format 10.6.106 und 11.0.89,
   401/403 beim PUT, fehlende Felder, Full-Replace erhält Fremdfelder,
   Rollback, Fremdänderung; #[sqlx::test] für alle Queries;
   Rollen- und Audit-Tests; Web-Checks. Doku docs/en/wifi-survey.md
   und docs/de/wlan-begehung.md (API-Key-Rechte, Risiken,
   Versionstabelle); ai-memory-Zusammenfassung.

Akzeptanzkriterien / Einschränkungen:
Bitte zuerst vorschlagen, wie und was du anpassen willst, und
mir den Vorschlag zur Review/Feedback geben, bevor du die
Umsetzung beginnst (Vorschlag als docs/…-proposal.md + ADR-Entwurf,
wie bei der WLAN-Begehung).
- Zielversion UniFi Network 11.0.x (Referenz 11.0.89) auf
  UniFi OS ≥ 6.0. Unter 10.6.106 bleibt TalkOps nur lesend.
- Ohne aktivierten Schreibschalter verhält sich TalkOps exakt wie
  heute.
- Kein Write ohne Bestätigung im Diff-Dialog; kein Write während
  einer laufenden Begehung.
- Vor jedem Write ein Snapshot; Rollback stellt genau die geänderten
  Felder wieder her.
- Keine Änderungen an AP-Funkeinstellungen und keine Classic-API-
  Writes in dieser Stufe.
- Offene Punkte für Stufe 0 auf der echten 11.0-Konsole benennen:
  Feldnamen per GET wifi/broadcasts/{id} bestätigen (anonymisiert,
  ohne Passphrase), Rechte des API-Keys für PUT, Dauer der
  Client-Trennung beim Anwenden.
##DEV-PROMT END##
```

## References

- [Wi-Fi survey proposal](../wlan-voice-survey-proposal.md), section "Stage 4"
- [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
- UniFi Network API reference, Update Wifi Broadcast:
  <https://developer.ui.com/network/v10.0.162/updatewifibroadcast>
- Community client for the official API:
  <https://github.com/Art-of-WiFi/unifi-network-application-api-client>
- py-unifi (not used): <https://github.com/ubiquiti-community/py-unifi>
