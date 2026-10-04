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

Vorlage: [`template.md`](template.md)
