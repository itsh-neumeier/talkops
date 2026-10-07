# Datensicherung und Wiederherstellung

Eine TalkOps-Sicherung ist eine einzelne Datei
`talkops-JJJJMMTT-HHMMSS.tar.gz` im Volume `backups`. Sie enthält:

- die Datenbank (Konfiguration, Benutzer, Anrufliste, Metadaten von
  Voicemail und Aufzeichnungen, Transkripte, Audit-Log, verschlüsselte
  SIP-/Trunk-Passwörter),
- die Volumes `voicemail`, `sounds`, `snapshots`, `provisioning` und, sofern
  nicht abgeschaltet, `recordings`.

Die Whisper-Modelle und die Laufzeitdatenbank von FreeSWITCH werden nicht
gesichert; sie werden automatisch neu geladen bzw. aufgebaut.

> **`TALKOPS_SECRET_KEY` sicher und getrennt von den Sicherungen aufbewahren.**
> Die SIP-, Trunk-, LDAP- und Türsprechstellen-Passwörter in der Sicherung
> sind damit verschlüsselt. Ohne den Schlüssel gelingt die Wiederherstellung
> zwar, aber all diese Passwörter müssen neu eingegeben werden. Auch die
> `.env`-Datei aufbewahren.

## Geplante Sicherungen

**Einstellungen → Datensicherung** (Admins):

- **Tägliche Sicherung** zur angegebenen Stunde (Zeitzone des Containers,
  `TZ` in der `.env`; Standard 03:00 Uhr).
- **Aufbewahrte Sicherungen**: ältere Archive werden nach jeder Sicherung
  gelöscht (Standard 7).
- **Gesprächsaufzeichnungen einschließen**: abschalten, wenn die
  Aufzeichnungen groß sind und anderweitig gesichert werden.
- **Jetzt sichern** startet sofort eine Sicherung; **Herunterladen**
  speichert ein Archiv auf dem eigenen Rechner.

Eine Sicherung auf derselben Platte übersteht keinen Plattenausfall. Archive
regelmäßig woandershin kopieren, z. B. herunterladen oder eine NAS-Freigabe
als Volume `backups` einbinden:

```yaml
# docker-compose.override.yml
volumes:
  backups:
    driver: local
    driver_opts:
      type: nfs
      o: addr=nas.local,rw,nfsvers=4
      device: ":/volume1/talkops-backups"
```

Die Freigabe muss für UID/GID 10001 beschreibbar sein.

## Sicherung auf der Kommandozeile

```sh
docker compose exec talkops talkops backup
# gibt /var/lib/talkops/backups/talkops-20261012-030000.tar.gz aus
docker compose cp talkops:/var/lib/talkops/backups/talkops-20261012-030000.tar.gz .
```

`--without-recordings` lässt die Aufzeichnungen weg.

## Wiederherstellung

Eine Wiederherstellung **ersetzt** die Datenbank und den Inhalt der Volumes
im Archiv. Ein Archiv einer älteren TalkOps-Version lässt sich in einer
neueren wiederherstellen (die Datenbank wird danach migriert), nicht
umgekehrt.

1. TalkOps und den Media-Worker stoppen (FreeSWITCH und Postgres laufen weiter):

   ```sh
   docker compose stop talkops media-worker
   ```

2. Wiederherstellen. Für ein Archiv im Volume `backups`:

   ```sh
   docker compose run --rm talkops restore --yes \
     /var/lib/talkops/backups/talkops-20261012-030000.tar.gz
   ```

   Für ein heruntergeladenes Archiv (z. B. auf einem neuen Server nach
   `docker compose up -d` mit derselben `.env`):

   ```sh
   docker compose stop talkops media-worker
   docker compose run --rm -v "$PWD/talkops-20261012-030000.tar.gz:/restore.tar.gz:ro" \
     talkops restore --yes /restore.tar.gz
   ```

3. Alles wieder starten; die Telefone registrieren sich von selbst neu:

   ```sh
   docker compose up -d
   ```

In Portainer: die Container `talkops` und `media-worker` stoppen und die
obigen Befehle auf dem Host ausführen.
