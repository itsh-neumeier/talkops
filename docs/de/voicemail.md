# Voicemail

Jede Nebenstelle kann eine Mailbox haben. Unbeantwortete, besetzte und
„Nicht stören“-Anrufe landen dort; neue Nachrichten zeigt die Telefonlampe
(MWI), auf Wunsch kommt eine E-Mail mit der Aufnahme.

## Einrichten

**Voicemail → Voicemail-Einstellungen** (eigene Nebenstellen) bzw. als Admin
auf der Seite der Nebenstelle:

- **Voicemail aktiv** einschalten.
- **PIN** (4–10 Ziffern) – nötig für den Abruf von einem fremden Telefon.
- **Sprache der Ansagen**: Deutsch oder Englisch (Standard aus den Einstellungen).
- **Ansage**:
  - *Standardansage* – „Ihr Gesprächspartner ist gerade nicht erreichbar …“
  - *Text* – wird von der Computerstimme (Piper, lokal) vorgelesen; nach dem
    Speichern dauert die Erzeugung einige Sekunden, danach lässt sie sich im
    Browser anhören.
  - *Eigene Aufnahme* – am Telefon `*97` wählen und die **5** drücken.
- **Per E-Mail senden** – an die E-Mail-Adresse des Benutzers der Nebenstelle,
  optional mit Aufnahme als WAV-Anhang.

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
