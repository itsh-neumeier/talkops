# Fehlerbehebung

[Handbuch](index.md) · [English](../en/troubleshooting.md)

Beginne mit Symptom und Zeitpunkt. Erhalte Konfiguration und Logs, bis die
Ursache geklärt ist. Lösche zur Behebung von Registrierungs- oder Speicherfehlern
keine Datenbank- oder Medienvolumes.

## Voraussetzungen

Benutzer können eigene Anrufe und Geräte prüfen. Fremde Daten benötigen die
passende Rolle. **Einstellungen → System** benötigt `diagnostics.manage`.
Host-Befehle benötigen Docker-Zugriff. Nutze das Deployment-Verzeichnis mit
allen aktiven Compose-Overrides, Umgebungsdateien und bisherigem Projektnamen.

## Erste Dienstprüfungen

```sh
docker compose ps
docker compose logs --since 15m talkops freeswitch media-worker
```

Prüfe Komponenten unter **Einstellungen → System → Status** und
Geräteregistrierungen sowie aktive Anrufe unter **Sitzungen**.
Logs können Rufnummern und private Daten enthalten; entferne sie vor Weitergabe.
Web/API nutzt standardmäßig Port 8080. Verwende den tatsächlich konfigurierten
Port und Host.

## Symptome

| Symptom | Erste Prüfung | Anleitung |
| --- | --- | --- |
| Oberfläche nicht erreichbar | Containerzustand, HTTP-Port, Proxy und HTTPS-Zertifikat. | [Installation](installation.md), [Monitoring](monitoring.md). |
| Anmeldung scheitert | Web-Passwort statt SIP-Passwort, TOTP, OIDC-/LDAP-Gruppen. | [Anmeldung](anmeldung.md), [Rollen](benutzer-und-rollen.md). |
| Telefon offline | Registrar, SIP-Zugangsdaten, Geräteadresse und Anmeldesperre. | [Nebenstellen](nebenstellen-und-geraete.md), [Provisionierung](provisionierung.md). |
| Trunk nicht registriert | Providerdaten, Authentifizierungsname, Registrar und Transport. | [Trunks](trunks-und-rufnummern.md), [LEONET](leonet.md). |
| Registriert, aber kein Klingeln | Rufnummernziel, Nicht stören, Weiterleitung, Zeitplan und Klingelzeit. | [Anrufsteuerung](anrufsteuerung.md). |
| Eingehender Anruf abgewiesen | Gespeicherter Sperrgrund, anonyme Regel, lokale Liste oder PhoneBlock. | [Anrufliste](anrufliste.md), [Anrufsteuerung](anrufsteuerung.md). |
| Kein oder einseitiger Ton | RTP-Erreichbarkeit, öffentliche IP/NAT und ausgehandelter Codec. | [Codec-Profile](codec-profile.md), [Softphone](softphone.md). |
| Browseranruf scheitert | HTTPS, Mikrofonzugriff, gewählte Identität und WebSocket-Proxy. | [Softphone](softphone.md). |
| Provisionierungsabruf scheitert | Vom Gerät erreichbare URL, Zugangsdaten, Modelldateien und Transport. | [Provisionierung](provisionierung.md), [Herstellereinstellungen](hersteller-provisionierung.md). |
| Ansage oder Transkript wartet | Worker-Zustand, Auftrag, Modell und Ressourcen. | [Ansagen](ansagen.md), [Transkriptionen](transkriptionen.md). |
| Fax-Vorbereitung scheitert | Administrativer Speichertest, gemeinsame Mounts, Platz und UID-10001-Zugriff. | [Fax](fax.md). |
| Mailversand scheitert | SMTP-Host, TLS, Zugang, Empfänger und Versandlog. | [E-Mail](e-mail.md), [Benachrichtigungen](benachrichtigungen.md). |
| Sicherungskopie scheitert | Verschlüsselung/Passwort, Zieltest, Mount-Wurzeln und Auftragsergebnis. | [Datensicherung](datensicherung.md). |
| Türvideo oder Öffner scheitert | Gerätespezifische Zugangsdaten, Videoregel und Öffnerkonfiguration. | [Türsprechstellen](tuersprechstelle.md), [Dahua](tuersprechstelle-dahua.md). |

## Anruffehler eingrenzen

1. Rufe zwischen zwei internen Geräten an, um Providerprobleme abzugrenzen.
2. Prüfe Klingeln, beidseitigen Ton und normales Auflegen.
3. Prüfe einen gewöhnlichen ausgehenden und einen separaten eingehenden Anruf.
4. Vergleiche abgeschlossene Anrufe mit Anrufliste und Zeitstempeln.

Nutze eine normale Testnummer, keine Notrufnummer. Registrierung prüft
SIP-Zugangsdaten, nicht den RTP-Medienweg. Prüfe konfigurierte RTP-Ports auf Host
und Netzwerk. Browseranrufe benötigen außerdem WebRTC/WSS und gegebenenfalls
TURN. Veröffentliche weder PostgreSQL noch den FreeSWITCH-Event-Socket zur
Behebung von Medienfehlern.

Verzögerte Anrufdatensätze können von FreeSWITCH zur Wiederholung zwischengespeichert
werden. Die [Installationsdiagnose](installation.md) erklärt Spool-Logs und Import.
Erhalte diese Daten während der Diagnose.

## Speicher- und Wiederherstellungsfehler

Nutze vor erneuter Fax-Vorbereitung den Speichertest. Er prüft den Serverzugriff;
prüfe bei Bedarf Worker- und FreeSWITCH-Mounts getrennt. Erhalte Anhänge und
fehlgeschlagene Versandresultate. Sende ein Fax mit unklarem Ergebnis erst
erneut, wenn der Übertragungsstatus geprüft ist.

Nutze nach einem Update mit Migrationen passendes Backup und die Offline-
[Wiederherstellung](datensicherung.md) statt eines ungeprüften Downgrades.
Prüfe danach Anrufe, Datenzugriff und neue Sicherungen vor normaler Nutzung.

## Supportbericht vorbereiten

Notiere TalkOps-Version, Image-Tags, Installationsart, Architektur, Gerätemodell
und Firmware, Provider, genaue Meldung, Zeitpunkte und Reproduktionsschritte.
Ergänze erwartetes und tatsächliches Ergebnis, bereinigte Logs sowie passende
Anfrage-/Anruf-IDs. Vergleiche bei Medienfehlern interne, ein- und ausgehende
Tests. Nutze bei Bedarf eine begrenzte Aufzeichnung unter
**Einstellungen → System → Diagnose**. Prüfe Dateien vor Weitergabe.

Reiche den Bericht über [GitHub Issues](https://github.com/itsh-neumeier/talkops/issues)
ein. Füge keine `.env`, Passwörter, Tokens, Provisionierungs-QR-Codes, privaten
Aufnahmen oder vollständigen Backup-Archive bei.
