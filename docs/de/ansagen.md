# Ansagen und Audio

[Handbuch](index.md) · [English](../en/prompts.md)

TalkOps nutzt Audio für Voicemail-Begrüßungen, Schritte im Smart Attendant,
Warteschlangenbegrüßungen und Wartemusik. Die Audioauswahl steht innerhalb der
jeweiligen Funktion. Eine eigene globale Ansagenbibliothek gibt es nicht.

## Voraussetzungen

Nutze einen unterstützten Browser. Mikrofonaufnahmen benötigen Berechtigung und
einen sicheren Browserkontext, normalerweise HTTPS. Lokale Spracherzeugung
benötigt laufenden Media-Worker und Sprachmodelle. Eigene Voicemail-Begrüßungen
folgen dem Mailboxzugriff; Routing-Audio benötigt `routing.manage`, globale
Wartemusik `settings.manage` und System-Voicemail-Texte `voicemail.settings`.

## Sprache erzeugen

1. Öffne die Begrüßung oder Audioauswahl in der Zielfunktion.
2. Wähle **Generieren**, danach Deutsch oder Englisch und eine Stimme.
3. Trage den Text ein und setze, soweit angeboten, das Sprechtempo.
4. Erzeuge die Audiodatei und warte, bis sie bereit ist.
5. Höre sie im Player an und speichere das umgebende Formular.

Es stehen zwei Stimmen je Sprache zur Verfügung. `1.0` ist normales Tempo;
der unterstützte Bereich ist `0.5` bis `2.0`. Eine Textänderung ersetzt einen
bestehenden Clip erst nach erneuter Erzeugung. Tempo und Inline-Tags beschreibt
[Voicemail](voicemail.md#sprechgeschwindigkeit).

## Aufnehmen oder hochladen

1. Wähle **Aufnehmen** und erlaube Mikrofonzugriff oder wähle **Hochladen**.
2. Stoppe die Aufnahme oder wähle eine vom Browser lesbare Audiodatei.
3. Höre das Ergebnis an und prüfe Lautstärke sowie Anfang und Ende.
4. Speichere das umgebende Formular.

Der Browser wandelt Aufnahmen und Uploads in PCM-WAV mit 16 kHz, Mono und
16 Bit um. Die Dateiauswahl belegt nicht, dass der Browser den Codec lesen kann.
Exportiere die Quelle bei Ladefehlern als WAV und versuche es erneut.
**Standard** und **Keine** erscheinen nur, wenn die Zielfunktion sie unterstützt.

## Verwendungszweck wählen

| Zweck | Einrichtung |
| --- | --- |
| Persönliche Mailboxbegrüßung | Voicemail; siehe [Voicemail](voicemail.md). |
| Systemstimme und Voicemail-Texte | Einstellungen → Voicemail. Erforderliche Platzhalter erhalten. |
| Sprachmenü-Begrüßung oder gesprochener Schritt | Einstellungen → Anrufsteuerung → Smart Attendant. |
| Begrüßung und Wartemusik der Warteschlange | Warteschlangeneditor; siehe [Anrufsteuerung](anrufsteuerung.md). |
| Globale Wartemusik | Einstellungen → Anrufbehandlung. Vorhandenen Titel oder eigene Audiodatei wählen. |
| Telefonklingelton oder Hintergrundbild | Einstellungen → Telefone; siehe [Yealink](yealink.md). |

## Prüfen und Fehler beheben

Rufe das betroffene Testziel an und höre die gespeicherte Ansage. Prüfe Sprache,
Lautstärke und die folgende Routingaktion. Browserwiedergabe allein prüft nicht
die Zuordnung zum gewünschten Anrufablauf.

- Erzeugung wartet oder scheitert: Worker-Status und Logs prüfen; Modelldownload abwarten.
- Mikrofon gesperrt: HTTPS, Browserberechtigung und Eingang prüfen.
- Alter Text hörbar: erneut erzeugen und umgebendes Formular speichern.
- Vorhandene Wartemusik fehlt: Start von FreeSWITCH und Bereitstellung der Titel prüfen.

Dienstprüfungen stehen unter [Fehlerbehebung](fehlerbehebung.md), die Aktion
nach der Ansage unter [Anrufsteuerung](anrufsteuerung.md).
