# Architecture Decision Records

Architekturentscheidungen von TalkOps im Format nach Michael Nygard
(Kontext → Entscheidung → Konsequenzen). Neue Entscheidungen bekommen die
nächste freie Nummer; überholte ADRs werden nicht gelöscht, sondern auf
„Ersetzt durch ADR NNNN“ gesetzt.

| Nr. | Titel | Status |
|---|---|---|
| [0001](0001-adrs-verwenden.md) | Architekturentscheidungen als ADRs festhalten | Angenommen |
| [0002](0002-telefonie-engine-freeswitch.md) | Telefonie-Engine: FreeSWITCH statt Asterisk oder reinem Rust-Stack | Angenommen |
| [0003](0003-datenbank-postgresql.md) | Datenbank: PostgreSQL | Angenommen |
| [0004](0004-frontend-sveltekit.md) | Frontend: SvelteKit-SPA, ausgeliefert vom Rust-Server | Angenommen |
| [0005](0005-job-queue-postgres.md) | Job-Queue in PostgreSQL statt eigenem Broker | Angenommen |
| [0006](0006-steuerung-xml-curl-und-esl.md) | FreeSWITCH-Steuerung über mod_xml_curl und Event Socket | Angenommen |
| [0007](0007-deployment-und-netzwerk.md) | Deployment: Docker Compose mit Host-Networking | Angenommen |
| [0008](0008-secrets-at-rest.md) | Verschlüsselung von SIP- und Trunk-Zugangsdaten | Angenommen |
| [0009](0009-basis-images-debian-trixie.md) | Basis-Images: Debian 13 „Trixie“ (FreeSWITCH mit PCRE2-Backport) | Angenommen |
| [0010](0010-voicemail-und-sprachausgabe.md) | Voicemail in Rust, Sprachausgabe mit Piper, Mail aus dem TalkOps-Dienst | Angenommen |
| [0011](0011-warteschlangen-mod-callcenter.md) | Warteschlangen mit mod_callcenter, Konfiguration aus TalkOps | Angenommen |
| [0012](0012-aufzeichnung-und-transkription.md) | Gesprächsaufzeichnung mit record_session, lokale Transkription mit whisper.cpp | Angenommen |
| [0013](0013-tuersprechstellen-dahua.md) | Türsprechstellen: Dahua VTO als SIP-Gerät, Steuerung über die HTTP-API | Angenommen |
| [0014](0014-identitaet-oidc-ldap-totp.md) | Identität: lokale Konten mit TOTP, OIDC, LDAP/AD | Angenommen |
| [0015](0015-webrtc-softphone.md) | Browser-Softphone: SIP über WebSocket durch TalkOps, SIP.js | Angenommen |
| [0016](0016-betrieb-sicherung-und-sip-schutz.md) | Betrieb: Datensicherung, SIP-Anmeldeschutz und Härtung | Angenommen |
| [0017](0017-audio-clips-und-smart-attendant.md) | Audio-Clips, Smart Attendant und Warteschlangen-Einstellungen | Angenommen |
| [0018](0018-turn-fuer-das-softphone.md) | TURN für das Browser-Softphone | Angenommen |
| [0019](0019-transkription-in-zwei-durchgaengen-und-ki-api.md) | Transkription in zwei Durchgängen, optional über eine KI-API | Angenommen |
| [0021](0021-guided-recovery-and-encrypted-deployment-backups.md) | Guided recovery and encrypted deployment backups | Accepted |
| [0022](0022-active-call-identities-and-live-previews.md) | Active call identities and local transcript previews | Accepted |
| [0023](0023-device-codec-profiles.md) | Codec profiles per device and transcoding in FreeSWITCH | Accepted |
| [0024](0024-transcription-engines-per-purpose.md) | Transcription engines per purpose, cloud connections and realtime previews | Accepted |
| [0025](0025-video-policy.md) | Video only on local calls and trunks with video | Accepted |
| [0026](0026-door-station-vendors.md) | Door station vendors and SIP password formats | Accepted |
| [0027](0027-shared-phonebook-numbers-in-active-calls.md) | Shared phonebook numbers in active calls | Accepted |
| [0028](0028-device-media-settings.md) | Media settings per device and door call ringing | Accepted |
| [0029](0029-roles-and-permissions.md) | Roles with fine-grained permissions | Accepted |
| [0030](0030-transcription-jobs-control.md) | Transcription jobs: progress, pause, resume and manual runs | Accepted |
| [0031](0031-backup-targets-with-rclone.md) | Backup targets with rclone | Accepted; storage parts superseded by 0036 |
| [0032](0032-automatic-deployment-capture.md) | Automatic deployment capture and Portainer API | Accepted |
| [0034](0034-mail-design-and-templates.md) | Mail design and editable mail templates | Accepted |
| [0035](0035-mail-notifications.md) | Mail notifications for operations events | Accepted |
| [0036](0036-storage-targets.md) | Storage targets as a shared area | Accepted |
| [0037](0037-audit-export-and-retention.md) | Audit log filters, export and retention | Accepted |
| [0038](0038-vendor-provisioning-profiles.md) | Vendor provisioning profiles and device transport overrides | Accepted |
| [0039](0039-source-derived-multi-vendor-provisioning.md) | Source-derived manufacturer provisioning and device copies | Accepted |
| [0040](0040-fax-over-sip-and-mail-input.md) | Fax over SIP and mailbox input | Accepted |
| [0041](0041-wifi-voice-survey-and-unifi-correlation.md) | Wi-Fi voice survey and read-only UniFi correlation | Accepted |
| [0042](0042-unifi-voice-ssid-write-access.md) | Changes to the UniFi voice SSID through the official API | Accepted |
| [0043](0043-wifi-handset-band-and-roaming.md) | Wi-Fi handset roaming threshold and band protection | Accepted |


Vorlage: [`template.md`](template.md)

| Number | Title | Status |
| --- | --- | --- |
| [0020](0020-softphone-identities-and-lifecycle.md) | Softphone identities and connection lifetime | Accepted |
