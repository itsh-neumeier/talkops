# Fax

TalkOps versendet PDFs und Bilder als Fax über einen SIP-Trunk. Eingehende
Faxrufnummern liefern Dokumente in einen Eingang und optional per E-Mail.
Nach einem Upgrade ist Fax zunächst deaktiviert.

## Einrichten

1. **Einstellungen → Fax** öffnen und Fax aktivieren.
2. Bei Bedarf eine globale ausgehende Nummer auswählen.
3. Festlegen, ob immer diese globale Nummer verwendet werden soll.
4. Kopf-/Fußzeile, Aufbewahrung und Übertragungsmodus je Trunk einstellen.
5. Einstellungen speichern.

Verfügbar sind T.38 mit G.711-Rückfall, ausschließlich G.711 und zwingend
T.38. Anbieter und Gegenstelle müssen den gewählten Modus unterstützen.
Vor dem produktiven Einsatz mit dem eigenen Anbieter testen.

Für den Empfang ein Faxziel mit Eigentümer, Sendeberechtigten und
Leseberechtigten anlegen. Optional eine E-Mail-Adresse für empfangene PDFs
eintragen. Unter **Einstellungen → Rufnummern** eine eingehende Nummer
diesem Faxziel zuweisen. Eine bisherige Sprachroute wird erst durch diese
ausdrückliche Änderung ersetzt. Automatische Sprach-/Faxerkennung ist
nicht vorgesehen.

Unter **Einstellungen → Benutzer → Rollen** stehen diese Rechte bereit:

- `fax.send`: Faxe vorbereiten und versenden.
- `fax.mail_submit`: Aufträge von freigegebenen E-Mail-Adressen einreichen.
- `fax.templates.edit`: persönliche Kopf-/Fußzeile speichern.
- `fax.use_global`: globale Absendernummer verwenden.
- `fax.access_all`: alle Faxdokumente des Mandanten lesen.
- `fax.delete`: zugängliche Faxe löschen.
- `fax.manage`: Faxregeln, Ziele und Whitelist verwalten.

Als Absender stehen nur eigene, ausdrücklich freigegebene gemeinsame und
berechtigte globale Nummern zur Auswahl. Rechte und Rufnummer werden vor
dem Wählen erneut geprüft.

## Über die Oberfläche senden

1. **Fax → Senden** öffnen.
2. Zielnummer international eingeben, beispielsweise `+49123456`.
3. Absendernummer und Anhänge auswählen.
4. **Vorschau vorbereiten** wählen.
5. Seiten und Absender prüfen.
6. **Fax jetzt senden** wählen.

Unterstützt werden PDF, JPEG, PNG, TIFF, BMP und WebP, auch gemischt.
Die Dateien bilden in ihrer Reihenfolge ein Fax. Ein Bild ergibt eine
Seite; bei mehrseitigen TIFFs werden alle Seiten übernommen. Grenzen:
10 Dateien, 20 MiB entpackte Anhänge und 50 Ausgabeseiten. Zusätzlich gelten
Pixel- und Größenlimits für Bilder. Animierte Bilder, verschlüsselte PDFs
und beschädigte Dateien werden abgelehnt. Ein ungültiger Anhang verhindert
den gesamten Versand.
Pro Benutzer sind höchstens 20 offene Faxdokumente zulässig. Neue Uploads
benötigen mindestens 100 MiB freien Speicher im Faxverzeichnis.

Gespeicherte Texte für Web-Faxe unterstützen `{sender}`, `{recipient}`,
`{date}`, `{page}` und `{pages}`. Die Aufbereitung reserviert Ränder und
passt Dokumentseiten ohne Abschneiden ein. Persönliche Texte erfordern
das entsprechende Rollenrecht.

## Mail-to-Fax

Ein eigenes Postfach auf einem bestehenden Mailserver verwenden. Benutzer
senden per SMTP an dieses Postfach. TalkOps holt ungelesene Nachrichten
alle 30 Sekunden per IMAPS ab und öffnet selbst keinen SMTP-Eingangsport.

1. Ausgehendes SMTP unter **Einstellungen → E-Mail** einrichten.
2. Unter **Einstellungen → Fax** Fax-in-Adresse, IMAPS-Server, Port,
   Benutzername, Passwort und Ordner eintragen.
3. Den Mailserver so konfigurieren, dass dieses Postfach nur Nachrichten
   authentifizierter Absender mit verbindlich zugeordneter Absenderadresse
   annimmt. Eine bloße From-Adresse kann gefälscht werden.
4. Diese Einschränkung in den Faxeinstellungen bestätigen.
5. Vollständige Absenderadressen zur Whitelist hinzufügen und jeweils einem
   TalkOps-Benutzer zuordnen. Dieser benötigt Sende- und Mail-Einreichungsrecht.
6. Postfachabholung aktivieren und speichern.

Beispiel:

```text
Von: freigegeben@example.com
An: faxin@example.com
Betreff: +49123456
Anhänge: dokument.pdf, bild.jpg
```

Der Betreff enthält ausschließlich eine saubere internationale Nummer.
Zusatztext, `Re:`, Leerzeichen innerhalb der Nummer, mehrere Nummern oder
`^` sind ungültig. Absender werden anhand der vollständigen ASCII-Adresse
ohne Beachtung der Groß-/Kleinschreibung verglichen. Platzhalter sind nicht
erlaubt. Eine leere Whitelist erlaubt keinen Absender.

**Nur Anhänge werden gefaxt. Mailtext, HTML, Signaturen, zitierte Nachrichten
und eingebettete Bilder werden nicht übernommen.** Im Faxkopf steht die
freigegebene Absender-E-Mail-Adresse. Die gespeicherte Fax-Fußzeile kann
zusätzlich erscheinen; sie enthält keinen Mailinhalt.

Bei ungültigem Betreff oder nicht freigegebenem Absender erhält der Absender
eine Ablehnung. Fehlende oder ungültige Anhänge werden ebenfalls gemeldet.
Die Rückmeldung enthält weder ursprünglichen Mailtext noch Anhänge.
Automatische Antworten, Zustellberichte und Nachrichten ohne brauchbaren
Absender erzeugen keine weitere Antwort. Fehler beim Mailversand lösen
keinen erneuten Faxversand aus.

Das Postfach benötigt ein gültiges TLS-Zertifikat. Es gibt keinen unsicheren
TLS-Modus. Andere Mailprogramme dürfen Nachrichten im ausgewählten Ordner
nicht vorzeitig als gelesen markieren. Abgelehnte Nachrichten bleiben zur
Diagnose im Postfach.
Die Faxeinstellungen zeigen die letzten 100 Mail-Eingangsergebnisse und
Zustellversuche für Rückmeldungen.

## Ergebnis, Aufbewahrung und Betrieb

Der Ausgang zeigt Vorbereitung, Warteschlange, Übertragung und Ergebnis.
Erfolg setzt eine vollständige Faxbestätigung für alle Seiten voraus.
Absender von Mailaufträgen erhalten einen Bericht. Empfangene PDFs erscheinen
im Eingang und optional an der Empfangsadresse des Faxziels.

Besetzte oder unbeantwortete Anrufe ohne übertragene Seiten werden bis zu
dreimal versucht. Bei **Zustellung unbekannt** vor erneutem Versand beim
Empfänger nachfragen: Das Dokument könnte bereits angekommen sein.

Die Aufbewahrung entfernt alte abgeschlossene Dokumente einschließlich
Quelldateien. Aktive Aufträge bleiben erhalten. Die Sicherung enthält das
Volume `fax`, auch wenn Sprachaufzeichnungen ausgeschlossen sind.

Alle drei Container verwenden `/var/lib/talkops/fax` und UID/GID 10001.
Bei abweichendem `TALKOPS_FAX_DIR` müssen API, Worker, FreeSWITCH und Mounts
denselben Pfad verwenden. `TALKOPS_SECRET_KEY` wird zur Wiederherstellung
des verschlüsselten Postfachpassworts benötigt.

Migrations-IDs: `20261106000001` und `20261106000002`. Nach Einrichtung
eines Faxziels keine ältere Programmversion mit dieser Datenbank betreiben.

## Speicherfehler nach einem Update beheben

Ein Image-Update aktualisiert keine vorhandene Compose- oder Portainer-Stack-
Definition. Server und Worker haben ein schreibgeschütztes Root-Dateisystem.
Ein fehlender Fax-Mount kann beim Upload `Read-only file system (os error 30)`
auslösen.

Unter **Einstellungen → Fax → Fax-Speicher** auf **Fax-Speicher erneut prüfen**
klicken. Die Prüfung erstellt, schreibt und entfernt eine kleine Testdatei als
Serverbenutzer. Außerdem müssen mindestens 100 MiB frei sein. Ein positives
Ergebnis bestätigt nur den Serverzugriff. Worker und FreeSWITCH separat prüfen.

Fehlende Einträge in der bestehenden Stack-Definition ergänzen. Andere Volumes
und Einstellungen beibehalten. Alle drei Dienste müssen dasselbe Volume nutzen.
Vorhandene Faxdaten bleiben in ihrem bisherigen Volume.

```yaml
services:
  talkops:
    volumes:
      - fax:/var/lib/talkops/fax
  media-worker:
    volumes:
      - fax:/var/lib/talkops/fax
  freeswitch:
    volumes:
      - fax:/var/lib/talkops/fax
volumes:
  fax:
```

Liegen bereits Faxdaten in einem anderen Volume oder Bind-Mount, diese Quelle
für alle drei Dienste verwenden. Der Zielpfad muss dem konfigurierten Faxpfad
entsprechen. Bei eigenem `TALKOPS_FAX_DIR` denselben Pfad für API und Worker
setzen und das Volume bei allen drei Containern dort einhängen. Die API
übergibt diesen Pfad an FreeSWITCH.

Die geänderte Stack-Definition anwenden. Mit Compose im bestehenden
Deployment-Verzeichnis und mit den üblichen Umgebungs- und Override-Dateien:

```sh
docker compose up -d talkops media-worker freeswitch
```

In Portainer den bestehenden Stack bearbeiten und aktualisieren. Projekt und
Volume-Quellen beibehalten. Ein Neustart allein übernimmt keine geänderten
Mounts. `read_only: true` bei API und Worker beibehalten. Nur das Fax-Volume
benötigt Schreibzugriff.

Den Zugriff in jedem Container als konfigurierten Dienstbenutzer prüfen:

```sh
for service in talkops media-worker freeswitch; do
  docker compose exec -T "$service" sh -ec '
    id
    probe=$(mktemp /var/lib/talkops/fax/.fax-check-XXXXXX)
    trap '\''rm -f "$probe"'\'' EXIT
    printf "fax storage check\n" > "$probe"
    cat "$probe"
  '
done
```

Bei eigener Konfiguration den Zielpfad im Befehl anpassen. Alle Dienste müssen
mit UID/GID 10001 auf dasselbe Volume zugreifen. Bei einem Bind-Mount dieser
UID/GID Schreibzugriff auf das bestehende Faxverzeichnis geben.

Uploads liefern `fax_storage_unavailable` (HTTP 503) bei unzugänglichem Speicher
oder `fax_storage_full` (HTTP 507) bei zu wenig freiem Platz. Das Sendefenster
zeigt eine übersetzte Erklärung und behält ausgewählte Anhänge bei. Nach der
Korrektur die Vorschau erneut vorbereiten. Ein abgewiesener Upload erzeugt
keinen Faxauftrag. Das Serverlog enthält den betroffenen Vorgang und Pfad.

## Abgewiesene eingehende Anrufe prüfen

PhoneBlock und die lokalen Anrufsperren gelten auch für eingehende Faxe.
`inbound call blocked` mit `phoneblock:4` bedeutet, dass vier Spam-Meldungen
den eingestellten Sperrschwellwert erreicht haben. TalkOps weist diesen Anruf
vor dem Start des Fax-Empfängers ab. Deshalb können null Sekunden Dauer und
`CALL_REJECTED` ohne Faxdokument erscheinen.

Die Sperrentscheidung in den Einstellungen für Anrufsperren prüfen. Danach
den Empfang mit einem zugelassenen Anrufer testen. Diese Speicherkorrektur
ändert keine Spam-Regeln, Ausnahmen oder T.38-Einstellungen.
