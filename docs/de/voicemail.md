# Voicemail

Jede Nebenstelle kann eine Mailbox haben. Unbeantwortete, besetzte und
„Nicht stören“-Anrufe landen dort; neue Nachrichten zeigt die Telefonlampe
(MWI), auf Wunsch kommt eine E-Mail mit der Aufnahme.

## Einrichten

**Voicemail → Voicemail-Einstellungen** (eigene Nebenstellen) bzw. als Admin
auf der Seite der Nebenstelle:

- **Voicemail aktiv** einschalten.
- **Voicemail nimmt ab nach** … Sekunden (5–300, etwa 5 s je Klingeln) –
  gilt auch für die Weiterleitung bei Nichtmelden.
- **PIN** (4–10 Ziffern) – nötig für den Abruf von einem fremden Telefon.
- **Sprache der Ansagen**: Deutsch oder Englisch (Standard aus den Einstellungen).
- **Ansage**: *Standard*, *Generieren*, *Aufnehmen*, *Hochladen* oder *Keine*
  (Anrufer hören sofort den Signalton) – siehe unten. Aufnehmen geht auch am
  Telefon: `*97` wählen und die **5** drücken.
- **Per E-Mail senden** – an die E-Mail-Adresse des Benutzers der Nebenstelle,
  optional mit Aufnahme als WAV-Anhang. Bei eingeschalteter Transkription
  enthält die Mail auch den Text der Nachricht (siehe
  [Aufzeichnung & Transkription](aufzeichnung.md)).

## Ansagen und Audio

Überall, wo TalkOps etwas ansagt – Voicemail, Smart Attendant, Warteschlangen –
gibt es dieselbe Auswahl:

- **Generieren:** Text eingeben, Sprache und Stimme wählen (je Sprache zwei:
  Deutsch *Thorsten* / *Kerstin*, Englisch *Linda* / *Joe*), **Generieren**
  klicken und anhören. Die Computerstimme (Piper) läuft lokal im
  Media-Worker; nichts verlässt den Server. Wird der Text geändert und ohne
  erneutes Generieren gespeichert, erzeugt TalkOps ihn beim Speichern.
- **Aufnehmen:** direkt im Browser über das Mikrofon (bis 10 Minuten).
- **Hochladen:** WAV, MP3, OGG oder M4A; der Browser wandelt die Datei in
  16 kHz Mono um.

Der Player zeigt die Wellenform, springt per Klick und spielt mit 1×, 1,5×
oder 2×. Nicht mehr verwendete Audiodateien löscht TalkOps nach einem Tag.

## Am Telefon abhören

| Wahl | Funktion |
|---|---|
| `*97` | eigene Mailbox (vom eigenen Telefon, ohne PIN) |
| `*98` | beliebige Mailbox: Nebenstelle + `#`, PIN + `#` |

Hauptmenü: **1** Nachrichten abhören, **5** Ansage aufnehmen, **\*** Ende.
Während einer Nachricht: **1** wiederholen, **7** löschen, **9** speichern,
**#** nächste Nachricht. Beim Aufsprechen beendet **#** die Aufnahme.

## Im Browser

Unter **Voicemail** stehen alle Nachrichten mit Player, Download, „als gehört
markieren“ und Löschen. Das Abspielen markiert eine Nachricht als gehört und
aktualisiert die Lampe am Telefon.

## E-Mail (SMTP)

**Einstellungen → E-Mail (SMTP)** (Admin): Server, Port, Verschlüsselung
(STARTTLS 587 oder TLS 465), Zugangsdaten und Absender, z. B.
`TalkOps <telefon@example.com>`. Mit **Testmail senden** lässt sich die
Konfiguration prüfen. Das Passwort wird verschlüsselt gespeichert.

## Hinweise

- Ansagen erzeugt der Dienst `media-worker` beim ersten Start (ca. 20 s). Ohne
  ihn funktioniert die Mailbox trotzdem, nur ohne gesprochene Ansagen.
- Nachrichten unter einer Sekunde (aufgelegt während der Ansage) werden
  verworfen; die maximale Länge ist pro Mailbox einstellbar (Standard 3 min).
- Aufnahmen liegen im Volume `voicemail` – bitte in die Datensicherung
  aufnehmen.
