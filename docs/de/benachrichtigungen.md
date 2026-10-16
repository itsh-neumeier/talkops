# E-Mail-Benachrichtigungen

TalkOps schickt eine E-Mail, wenn an der Telefonanlage etwas Wichtiges
passiert: Ein Trunk verliert die Registrierung, FreeSWITCH ist nicht
erreichbar, eine Datensicherung schlägt fehl, der Speicher ist fast voll oder
jemand ändert die Konfiguration.

Du richtest die Benachrichtigungen unter **Einstellungen →
Benachrichtigungen** ein. Dafür brauchst du die Berechtigung
„Benachrichtigungen“ (Administratoren haben sie). Der Mailserver muss unter
**Einstellungen → E-Mail** eingerichtet sein.

## Standard

Ohne Änderung bekommen die Benutzer der eingebauten Administrator-Rolle mit
E-Mail-Adresse die Mails. Die meisten Ereignisse sind an. Aus sind
Ereignisse, die oft vorkommen können: Registrierung von Geräten,
Konfigurationsänderungen, erfolgreiche Datensicherungen und angezeigte
Geheimnisse.

## Ereignisse

| Bereich | Ereignisse |
| --- | --- |
| Trunks | Registrierung verloren, Registrierung abgelehnt (falsche Zugangsdaten), Trunk-Konfiguration geändert |
| FreeSWITCH | Verbindung verloren, FreeSWITCH neu gestartet |
| Geräte | Gerät nicht mehr registriert |
| Sicherheit | Adresse vom SIP-Schutz gesperrt, wiederholt fehlgeschlagene Anmeldungen, Administrator von neuer Adresse angemeldet, Zwei-Faktor-Anmeldung geändert, Passwort geändert, API-Token erstellt oder widerrufen, Geheimnis angezeigt |
| Sicherung und Speicher | Datensicherung fehlgeschlagen oder erstellt, Sicherungsziel fehlgeschlagen, seit zwei Tagen keine Datensicherung, wenig Speicherplatz (unter 10 % oder 2 GB) |
| Dienste | Medien-Aufträge schlagen fehl, Medien-Worker arbeitet nicht, Voicemail-Mail fehlgeschlagen |
| Konfigurationsänderungen | Benutzer und Rollen, Rufnummern und Routing, Nebenstellen und Geräte, System-Einstellungen, Datensicherung wiederhergestellt, Neustart ausgelöst |

Ereignisse mit „Geplant“ stehen in der Liste, TalkOps erkennt sie aber noch
nicht.

## Regeln

Klicke bei einem Ereignis auf **Bearbeiten**:

- **Mindest-Schweregrad**: Nur Ereignisse mit diesem oder einem höheren
  Schweregrad senden eine Mail.
- **Empfänger**: Rollen, einzelne Benutzer und weitere E-Mail-Adressen.
  Benutzer ohne E-Mail-Adresse bekommen keine Mail.
- **Wartezeit vor dem Alarm** (nur Probleme): TalkOps wartet diese Zeit. Ist
  das Problem in dieser Zeit behoben, geht keine Mail raus. Beispiel: Ein
  Trunk, der beim Neustart des Providers eine Minute offline ist, sendet
  keine Mail (Standard 2 Minuten).
- **Mail senden, wenn das Problem behoben ist** (nur Probleme): Du bekommst
  eine zweite Mail, wenn der Trunk wieder registriert ist, die Verbindung
  wieder steht oder wieder genug Speicher frei ist.
- **Zu einer Zusammenfassung bündeln**: TalkOps sammelt die Ereignisse
  dieser Zeit und schickt eine Mail mit einer Liste.
  Konfigurationsänderungen nutzen standardmäßig 5 Minuten.
- **Mails pro Stunde**: Weitere Mails für dieses Ereignis gehen nicht raus;
  das Protokoll zeigt „Limit erreicht“.

**Test senden** schickt eine Beispiel-Mail an die Empfänger der gespeicherten
Regel. **Auf Standard zurücksetzen** stellt die Standardwerte wieder her.

## Protokoll

Die Karte **Benachrichtigungsprotokoll** zeigt die letzten 100 Ereignisse und
die letzten 100 Mails mit ihrem Status. Einträge, die älter als 30 Tage
sind, werden gelöscht.

## Text der Mail

Die Mail-Art „Benachrichtigung“ findest du unter **Einstellungen → E-Mail →
E-Mail-Texte**. Betreff und Text änderst du wie bei der Voicemail-Mail.
Platzhalter: `event.title`, `event.category`, `event.severity`,
`event.status`, `event.time`, `event.details`, `event.count`,
`event.recovery` (gesetzt bei einer Entwarnung) und `event.digest` (Liste
einer Zusammenfassung). Die Mail nutzt die Standardsprache aus den
Telefonie-Einstellungen.
