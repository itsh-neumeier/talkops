# 0017 – Audio-Clips und Smart Attendant

- Status: Angenommen
- Datum: 2026-10-13

## Kontext

Für 1.1 sollen Ansagen wie bei UniFi Talk entstehen: mit der Computerstimme
generieren (vor dem Speichern anhören, zwei Stimmen je Sprache), im Browser
aufnehmen oder hochladen. Bisher hatte jede Stelle (Voicemail-Box, Sprachmenü)
ihre eigene Logik für Text, Datei und Render-Status. Außerdem sollen
Sprachmenüs zu Abläufen werden (Tastenmenü, Telefone klingeln mit Rückfall,
Zeitplan, Voicemail für mehrere Empfänger, Parken, …), und Warteschlangen
brauchen Begrüßung, Wartemusik, Öffnungszeiten, Überlauf und Gruppen-Voicemail.

## Optionen

1. Jede Funktion behält eigene Ansage-Felder und Upload-Endpunkte.
2. Ein gemeinsames Objekt **Audio-Clip**, auf das alle Stellen per ID zeigen.
3. Für Abläufe: eigene Tabelle je Schritt-Art (relational) **oder** ein
   JSON-Baum je Ablauf.
4. „Telefone klingeln“ im ESL-Ablauf selbst (`bridge` über die Outbound-
   Verbindung) **oder** über den Dialplan mit Rücksprung in den Ablauf.

## Entscheidung

- **Audio-Clips** (`audio_clips`): Quelle `tts` (Text, Sprache, Stimme 1/2),
  `upload` oder `recording`; Datei `sounds/clips/<tenant>/<id>.wav`.
  Generierte Clips rendert der Media-Worker (Job `tts.clip`, Priorität 10);
  Uploads und Aufnahmen wandelt der Browser in 16 kHz Mono-WAV, der Server
  prüft sie mit `hound`. Stimmen: Thorsten/Kerstin (de), Linda/Joe (en) – nur
  frei nutzbare Modelle (CC0 bzw. ohne NC-Klausel). Nicht referenzierte Clips
  löscht die Retention nach einem Tag (Referenzliste in
  `talkops_core::audio::REFERENCES`).
- **Smart Attendant** ersetzt die Sprachmenüs in derselben Tabelle
  `ivr_menus` (Zielart `ivr` bleibt, damit alle Verweise gültig bleiben). Der
  Ablauf ist ein **JSON-Baum** (`flow`, Typ `talkops_core::attendant::Node`);
  ein leerer Zweig legt auf, „Gehe zu“ springt zu einer Schritt-ID. Die
  Prüfung (IDs, Tasten, Grenzen, Verweise) liegt in Rust; `clip_ids` wird
  beim Speichern gepflegt. Die Migration wandelt alte Menüs um, der Server
  verschiebt ihre Ansagedateien beim Start.
- **Ausführung** über ESL outbound (`attendant::run`), höchstens 50 Schritte
  je Anruf. „Telefone klingeln“ geht per `transfer attendant:<id>:<schritt>`
  in den Dialplan (gleiche Bridge-Logik wie Rufgruppen inkl. Aufzeichnung,
  Pickup, DND); ohne Annahme kehrt der Anruf mit `talkops_attendant_step` in
  den Ablauf zurück. Parken reserviert einen freien Platz über `valet_info`.
- **Warteschlangen** bekommen die zusätzlichen Einstellungen als Spalten;
  Wartemusik als `moh-sound` von mod_callcenter, „voll“ über die Zahl der
  wartenden Mitglieder (`callcenter_config queue list members`), Gruppen-
  Voicemail über die Outbound-Verbindung (`talkops_app=queue_vm`).

## Konsequenzen

- Eine Ansage-Komponente in der Oberfläche für alle Stellen; neue Funktionen
  verweisen nur auf eine Clip-ID.
- JSON-Abläufe sind einfach zu versionieren und als Ganzes zu speichern,
  Fremdschlüssel innerhalb des Baums gibt es aber nicht: gelöschte Ziele
  werden zur Laufzeit übersprungen (Klingeln) bzw. als „geschlossen“ (Zeitplan)
  oder Ende des Zweigs behandelt.
- Die API `/ivr-menus` entfällt zugunsten von `/attendants` (Breaking Change
  der REST-API, in der Weboberfläche transparent).
- Mehrere Empfänger einer Voicemail bekommen je eine Kopie (eigene
  Transkription und E-Mail).
