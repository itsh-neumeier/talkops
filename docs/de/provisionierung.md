# Provisionierungsprofile und Vorlagen

[Handbuch](index.md) · [English](../en/provisioning.md)

## Voraussetzungen

Du benötigst: phones.manage, ein unterstütztes Gerät und eine erreichbare Provisionierungsadresse.

Die zusätzlichen Herstellerprofile und Kategorien sind unter
[Herstellereinstellungen und Gerätekopien](hersteller-provisionierung.md)
beschrieben.

## Adressen festlegen

Unter **Telefone → Übertragung der Provisionierung** lässt sich eine feste
Provisionierungsadresse getrennt von der Browseradresse festlegen. Der Pfad
lautet `/provisioning`. Der optionale SIP-Server wird ohne Schema und Port
angegeben. Leere Werte übernehmen wie bisher die jeweilige Abrufadresse.

Unter **Geräte-Provisionierung** kann jedes Telefon eine eigene Adresse nutzen.
Beispielsweise kann ein T41P `http://192.168.1.10:8095/provisioning` verwenden,
während das AX86R über die HTTPS-Standardadresse läuft. HTTP nur im
vertrauenswürdigen Telefon-LAN verwenden. Die Ausnahme gilt auch für
Telefonbücher, Firmware, Medien, Statusmeldungen und Softkey-Dateien.
TalkOps schaltet HTTPS nicht automatisch auf HTTP um und deaktiviert keine
Zertifikatsprüfung.

DHCP-Option 66 verteilt einen Serverwert. Auch wenn die DHCP-Oberfläche das Feld
„TFTP-Server“ nennt, wählt eine HTTP-/HTTPS-URL bei unterstützenden Telefonen
dieses Protokoll. TalkOps stellt keinen TFTP-Dienst bereit. Für DHCP die unter
**Autoprovisionierung** angezeigte URL mit Zugangsdaten verwenden. Bei manueller
Eingabe Benutzername und Passwort in die getrennten Felder des Telefons
eintragen. Die URL mit Zugangsdaten nicht veröffentlichen.

Das Telefon muss TalkOps zuerst erreichen, um eine Geräte-Ausnahme zu laden.
Scheitert bereits die erste HTTPS-Verbindung, trage die HTTP-Adresse und die
Provisionierungs-Zugangsdaten einmal am Telefon ein. Alternativ kann eine
gerätespezifische DHCP-Option 66 diese HTTP-Adresse verteilen. Nach dem ersten
erfolgreichen Download behält die Geräte-Konfiguration die gewählte Adresse.

## Tasten belegen

Der grafische Leitungstasten-Editor bleibt verfügbar. Je Modell sind zusätzliche
Funktionen wie Mailbox, gezieltes Heranholen, DTMF, Halten, Verbinden und
Konferenz möglich. Unter **Geräte-Provisionierung** gibt es getrennte Gruppen
für programmierbare Tasten, Erweiterungsmodule und Softkeys im Gespräch.

Das T41P hat 13 programmierbare Bedienelemente: vier Ruhe-Softkeys,
Richtungstasten, OK, Abbrechen, Stumm, Wahlwiederholung und Nachricht.
Die IDs lauten 1–10, 13, 17 und 18. Die letzten beiden sind nicht 11 und 12.
Für das T41P ist kein Erweiterungsmodul freigegeben.

1. Taste und Funktion wählen.
2. Die passenden Felder ausfüllen. Ein lokales Menü benötigt kein SIP-Konto.
3. Für Kurzwahl eine Kontaktrufnummer auswählen oder z. B. `**101` eintragen.
4. Für XML-Telefonbuch das interne, globale oder zugewiesene Buch auswählen.
5. **Übernehmen**, danach am Telefon **Speichern** oder **Speichern &
   synchronisieren** wählen.

**Geerbt / Werkseinstellung** entfernt eine Ausnahme. **Unbelegt** leert eine
Taste ausdrücklich. Beim Zurücksetzen wird der Profilstandard beim nächsten
Abruf übertragen; die alte Belegung bleibt dadurch nicht auf dem Gerät stehen.
Gesprächs-Layouts haben vier geordnete Aktionen je Zustand. **Standard**
entfernt die Ausnahme für diesen Zustand.

Das normale Profil verwendet Yealink-V83+-Funktionscodes. Das ausdrücklich
wählbare `fusionpbx-legacy`-Profil nutzt ältere Quellregister-Codes für
Verzeichnisse und Kontowechsel. Nur wählen, wenn die Firmware das benötigt.
Die importierten Kommentare und Register unterscheiden sich. Neue Belegungen
und Layouts müssen am Gerät geprüft werden. SMS, BroadSoft, gemeinsame
Leitungen und ACD werden durch Einträge in Quelldateien nicht zu verfügbaren
TalkOps-Diensten.

Eine direkte Pickup-Taste mit Ziel `101` wählt `*8*101`. Ein ausdrücklich
eingetragener Funktionscode bleibt erhalten. Bei einer umbelegten Stummtaste
deaktiviert TalkOps zusätzlich Yealinks Keep-Mute-Einstellung. Zurücksetzen
stellt diese Einstellung wieder her.

Die moderne XML-Telefonbuchaktion verwendet XML Group (Typ 22) und den
gewählten Telefonbuchindex. Das Legacy-Profil verwendet den älteren Typ 47
aus der Quellenregistrierung.

## Vorlagen anpassen

**Konfigurationsvorschau** zeigt die Geräteänderungen vor dem Speichern und
einen Parametervergleich. Erzeugte SIP-, Provisionierungs- und Geräte-Admin-
Passwörter werden vor dem Rendern ersetzt. Die bestehende rohe
Konfigurationsansicht ist eine getrennte, berechtigungspflichtige und
protokollierte Aktion mit echten Zugangsdaten.

Unter **Telefone → Provisionierungsvorlagen** sind alle 433 Quelldateien aus
70 FusionPBX-Yealink-Verzeichnissen eines festen Versionsstands lesbar.
Ein Vorlagenblock kann von einem anderen Modell stammen. Rohe Smarty-Dateien
dienen als Referenz und werden nicht ausgeführt.

1. **TalkOps-Vorlagen** öffnen, Name und Modell wählen und **Kopieren** anklicken.
2. `common.cfg` oder `mac.cfg` bearbeiten. Den Header in der ersten Zeile lassen.
3. Variablen und Schleifen aus der nativen Vorlage verwenden. Keine echten
   Passwörter in den Quelltext schreiben; Geheimnisse als Variablen belassen.
4. **Neue Version speichern** wählen. Die aktive Version bleibt unverändert.
5. **Prüfen und aktivieren** wählen. Bei einem Fehler bleibt die alte Version aktiv.
6. Die aktive Vorlage am passenden Telefon auswählen, speichern und synchronisieren.

Pro Quelldatei sind 64 KiB erlaubt. Rendern, Rekursion und Ausgabe sind begrenzt.
Vorlagen haben keinen Datei- oder Netzwerkzugriff und sind an Mandant und
Modell gebunden. Für eine Wiederherstellung eine ältere Version wählen und
erneut aktivieren. Eine Aktivierung sendet nicht automatisch Resync.

## Resync ohne Download untersuchen

Resync sendet SIP NOTIFY. Das Ergebnis bestätigt noch keinen Konfigurationsabruf.
Unter **Übertragung der Provisionierung → Letzte Konfigurationsabrufe**
aktualisieren und nach einem neuen MAC-CFG-Abruf des Telefons suchen.

- `200`: Datei übertragen. Die Anwendung am Telefon prüfen.
- `401`: Provisionierungs-Benutzername und Passwort am Telefon bzw. in DHCP prüfen.
  Eine erste Anmeldeaufforderung mit anschließendem `200` ist normale Basic-Authentifizierung.
- `404`: MAC, Modell, Dateiname und angelegtes Telefon prüfen.
- `429`: Zu viele fehlgeschlagene Anmeldungen.
- Kein neuer Abruf: Verbindung, DNS, Gerätezeit, TLS/Zertifikate und tatsächlich
  genutzte Adresse prüfen.

Beim gemeldeten T41P mit Firmware `36.83.0.120` funktioniert AX86R bereits über
HTTPS mit DHCP 66; T41P-Resync funktioniert über internes HTTP. Das belegt
weder einen allgemeinen HTTPS-Fehler noch fehlendes TFTP. Manuelles HTTPS und
HTTP am T41P vergleichen, danach die gerätespezifischen DHCP-Antworten prüfen.
Scheitert TLS vor HTTP, entsteht hier kein Abrufeintrag. Eine feste HTTP-
Geräteausnahme dient als kontrollierter LAN-Workaround. Die genaue Ursache
bei Firmware, Vertrauenskette, TLS oder Anmeldung bleibt separat zu prüfen.

## Aktualisierung

Migration `20261105000001_provisioning_profiles` ergänzt Geräteausnahmen,
Übertragungsoptionen, Vorlagenversionen und Abrufprotokolle. Bestehende
Zugangsdaten und Kontozuordnungen bleiben erhalten. Ältere API-Clients, die
`provisioning` nicht mitsenden, bewahren die gespeicherten Ausnahmen.
Nach Änderungen am Telefon synchronisieren. Ein alter „Zuletzt gesehen“-Wert
bestätigt keinen neuen erfolgreichen Resync.

## Ergebnis prüfen

Prüfe tatsächlichen Geräteabruf, Kontoregistrierung und einen Testanruf.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.
