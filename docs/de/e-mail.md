# E-Mail: Design und Texte

TalkOps sendet Mails bei neuen Sprachnachrichten und für den SMTP-Test.
Jede Mail hat einen HTML-Teil und einen Textteil. Design und Texte legst du
unter **Einstellungen → E-Mail** fest. Dafür brauchst du das Recht
`voicemail.settings`.

## SMTP

1. Trage SMTP-Server, Port, Verschlüsselung, Benutzername und Passwort ein.
2. Trage die Absenderadresse ein, z. B. `pbx@example.com`.
3. Klicke auf **Speichern**.
4. Trage einen Test-Empfänger ein und klicke auf **Testmail senden**.

## E-Mail-Design

- **Name der Instanz**: der Name dieser Installation, z. B. „Büro
  Musterstadt“. Jeder Betreff beginnt dann mit `[Büro Musterstadt]`. Jede
  Mail zeigt den Namen in der Fußzeile.
- **Absendername**: der Anzeigename zur Absenderadresse.
- **Akzentfarbe**: die Farbe der Linie über der Mail und der Links. Ist die
  Farbe auf weißem Grund zu hell, nutzt TalkOps für Links dunkles Petrol.
- **Logo**: PNG, JPEG oder GIF, höchstens 256 KB. Die Mail zeigt das Logo
  höchstens 48 px hoch. Das Logo ist Teil der Mail (Inline-Bild); Mailprogramme
  zeigen es ohne Download aus dem Internet.
- **Kopfzeile** und **Fußzeile**: kurze Texte über und unter der Nachricht.
  Erlaubt sind `**fett**`, `*kursiv*`, Links und die Platzhalter der Instanz
  (`{{ instance.name }}` …).

Jede Fußzeile zeigt außerdem Instanzname, Hostname, öffentliche URL und die
Zeile „Powered by TalkOps vX.Y.Z“. Diese Zeile lässt sich nicht entfernen.
Die öffentliche URL ist die Einstellung **Öffentliche URL** unter
**Einstellungen → Anmeldung & Sicherheit**.

## E-Mail-Texte

1. Wähle die Mail-Art (z. B. **Voicemail**) und die Sprache.
2. Ändere Betreff und Text. Die Vorschau zeigt die Mail mit Beispielwerten.
3. Für einen Platzhalter setze den Cursor in Betreff oder Text und klicke auf
   den Platzhalter.
4. Klicke auf **Speichern**. TalkOps lehnt Texte mit unbekannten
   Platzhaltern oder Syntaxfehlern ab.

Das Textformat:

| Eingabe | Ergebnis |
| --- | --- |
| Leerzeile | Neuer Absatz |
| `**Text**` | **fett** |
| `*Text*` | *kursiv* |
| `[Öffnen](https://example.com)` | Link (nur `https://`, `http://`, `mailto:` oder `{{ instance.url }}`) |
| `- Punkt` oder `1. Punkt` | Liste |
| `{{ caller.name }}` | Wert eines Platzhalters |
| `{% if transcript %}…{% endif %}` | Text nur, wenn der Platzhalter einen Wert hat |
| `\*` | Ein einzelnes `*` |

Werte werden als Text eingefügt. Ein Anrufername kann also keine
Formatierung und keine Links in eine Mail bringen.

**Test senden** schickt den aktuellen Text mit Beispielwerten an eine
Adresse. **Auf Standard zurücksetzen** stellt den eingebauten Text dieser
Mail-Art und Sprache wieder her.

Die Sprache einer Voicemail-Mail ist die Sprache der Voicemail-Box.
