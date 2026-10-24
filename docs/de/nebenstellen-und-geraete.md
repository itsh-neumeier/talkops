# Nebenstellen und Geräte

[Handbuch](index.md) · [English](../en/extensions-and-devices.md)

Eine Nebenstelle enthält interne Nummer und Anrufeinstellungen. Ein Benutzer
kann Nebenstellen besitzen. Ein Gerät ist eine SIP-Identität einer Nebenstelle.
Ein provisioniertes Telefon kann mehrere Konten enthalten. Das Web-Passwort
ist kein SIP-Passwort.

## Voraussetzungen

Administratoren können alle Schritte ausführen. Andere Rollen benötigen
`extensions.view` und `extensions.manage` für Nebenstellen, `users.manage`
für Benutzeränderungen, `phones.manage` für Provisionierung und `trunks.manage`
für Änderungen eingehender Rufnummern. Eigene erlaubte Geräte und
Anrufeinstellungen verwalten Benutzer unter **Einstellungen → Meine Telefone**.
Siehe [Benutzer und Rollen](benutzer-und-rollen.md).

## Nebenstelle erstellen

1. Erstelle bei Bedarf den Benutzer unter **Einstellungen → Benutzer**.
2. Öffne **Einstellungen → Nebenstellen → Neue Nebenstelle**.
3. Trage eine freie interne Nummer und einen verständlichen Namen ein.
4. Weise den Benutzer zu und wähle Ausgangsrufnummer oder Systemstandard.
5. Setze Klingelzeit, Aufzeichnung und Videooptionen nach Bedarf.
6. Speichere und öffne die Nebenstelle.

Neue interne Nummern haben 3–8 Stellen, beginnen ab 100 und nicht mit null.
Notrufnummern, deren Präfixe und reservierte Servicenummern sind ausgeschlossen.
Das Formular meldet Konflikte. Bestehende kurze Nummern können nach Updates
erhalten bleiben. Die [ersten Schritte](erste-schritte.md) erklären Wahlregeln.

## Seitenpanel einer Nebenstelle

Eine Nebenstelle in der Liste auswählen, um ihr Seitenpanel zu öffnen:
**Übersicht** (Benutzer, ausgehende Rufnummer, Status mit Nicht stören und
Umleitung), **Geräte** (mit Link zum provisionierten Telefon) und
**Einstellungen** (**Alle Einstellungen öffnen**, **Bearbeiten** und
**Löschen**). Die Seite der Nebenstelle behält alle Editoren.

## Manuelles SIP-Gerät hinzufügen

1. Wähle **Gerät hinzufügen** an der Nebenstelle oder unter **Meine Telefone**.
2. Wähle den Gerätetyp und speichere.
3. Öffne **Zugangsdaten anzeigen** und übertrage SIP-Server, Benutzername und Passwort ins Gerät.
4. Prüfe die Registrierung und rufe eine zweite Nebenstelle an.
5. Prüfe Klingeln und Sprache in beide Richtungen.

Erwartetes Ergebnis: Das Gerät ist online und erreicht die andere Nebenstelle.
Alle Geräte einer Nebenstelle klingeln gemeinsam, sofern Anrufeinstellungen
dies nicht verhindern. Ein neuer SIP-Benutzername oder ein neu erzeugtes
Passwort muss auch am Gerät eingestellt werden. Zugangsdaten und exportierte
QR-Codes bleiben privat. Beachte angebotene Passwortformate und bei passenden
Geräten die [Dahua-Anleitung](tuersprechstelle-dahua.md).

Automatische Einrichtung beschreiben [Provisionierung](provisionierung.md)
und [Herstellereinstellungen](hersteller-provisionierung.md). Erstelle keine
zweite manuelle Identität für ein bereits durch Provisionierung verwaltetes
Konto. Audio- und Videoeinstellungen stehen unter [Codec-Profile](codec-profile.md).

## Nicht stören und Weiterleitung ändern

1. Öffne **Einstellungen → Meine Telefone** und suche die Nebenstelle.
2. Setze **Nicht stören** oder trage das Ziel für **Alle Anrufe umleiten an** ein.
3. Speichere und rufe die Nebenstelle von einem zweiten Telefon an.
4. Leere später das Weiterleitungsfeld oder deaktiviere Nicht stören.

Eine interne oder externe Rufnummer kann Weiterleitungsziel sein. Externe
Weiterleitung benötigt einen nutzbaren Trunk und kann Providerkosten erzeugen.
Die Codes `*78`/`*79` schalten Nicht stören, `*72<Nummer>`/`*73` die
Weiterleitung. Weitere Codes stehen in der [Funktionstabelle](yealink.md#4-kurzwahlen-am-telefon).

## Eingehende Rufnummern und häufige Fehler

Wähle im Nebenstellen-Bearbeitungsdialog **Eingehende Rufnummern, die hier
klingeln**. Prüfe das angezeigte aktuelle Ziel: Das Hinzufügen kann eine
bisherige Gruppe oder ein anderes Ziel ersetzen. Die Zuordnung erklären
[Trunks und Rufnummern](trunks-und-rufnummern.md).

- **Meine Telefone** leer: Benutzerzuweisung der Nebenstelle prüfen.
- Gerät offline: SIP-Zugangsdaten, Adresse, Port und SIP-Anmeldesperren prüfen.
- Online, aber kein Klingeln: Nicht stören, Weiterleitung, Klingelzeit und Route prüfen.
- Keine Medien oder kein Video: [Codecs](codec-profile.md) und [Netzwerkdiagnose](fehlerbehebung.md) prüfen.
