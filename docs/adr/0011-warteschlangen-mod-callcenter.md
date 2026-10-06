# 0011 – Warteschlangen mit mod_callcenter, Konfiguration aus TalkOps

- Status: Angenommen
- Datum: 2026-10-06

## Kontext

Phase 4 bringt Warteschlangen (Anrufer warten mit Musik, Agenten bekommen
Anrufe nach einer Strategie). Die Konfiguration soll wie alles andere in
PostgreSQL liegen und im Web-UI gepflegt werden.

## Optionen

1. **Eigene Queue-Logik in Rust** über ESL (Parken, Originate, `uuid_bridge`)
   – volle Kontrolle, aber viel fehleranfällige Eigenentwicklung
   (Agentenzustände, Wrap-up, Abbrüche, Rennbedingungen).
2. **`mod_fifo`** – einfach, aber ohne Strategien und Agentenverwaltung.
3. **`mod_callcenter`** – ausgereift, Strategien, Agentenzustände, Wrap-up;
   hält Agenten und Tiers jedoch in einer eigenen Datenbank, die nur beim
   Laden des Moduls aus `callcenter.conf` gefüllt wird.

## Entscheidung

`mod_callcenter`, gesteuert von TalkOps:

- `callcenter.conf` (Queues, Agenten, Tiers) liefert `/fs/xml`; Queues werden
  bei Bedarf geladen.
- Ein Abgleich (`talkops_api::callcenter`) bringt Agenten und Tiers per
  `callcenter_config` auf den Soll-Stand – nach jeder Änderung und alle 30 s
  (DND, Geräte). Er ist diff-basiert und unterbricht laufende Gespräche nicht.
- Ein Agent je Nebenstelle (`a-<id>`), Kontakt = alle Geräte der Nebenstelle,
  Status `On Break` bei DND, deaktivierter Nebenstelle oder ohne Geräte.

## Konsequenzen

- Bewährte Verteil-Logik, wenig eigener Code; der Abgleich ist eine reine
  Funktion und unit-getestet, der Ablauf per E2E-Test mit echtem FreeSWITCH.
- Die mod_callcenter-Datenbank ist flüchtig (kein Volume nötig): nach einem
  Neustart stellt der Abgleich den Stand wieder her.
