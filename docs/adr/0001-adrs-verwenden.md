# 0001 – Architekturentscheidungen als ADRs festhalten

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

TalkOps kombiniert mehrere ausgereifte Fremdkomponenten (FreeSWITCH,
PostgreSQL, Piper, whisper.cpp) mit eigener Steuerlogik. Viele Entscheidungen
(z. B. warum FreeSWITCH „dumm“ bleibt) sind ohne Begründung schwer
nachvollziehbar – für Menschen wie für KI-Assistenten, die am Code arbeiten.

## Entscheidung

Wesentliche Architekturentscheidungen werden als kurze ADRs unter `docs/adr/`
festgehalten (Kontext, Optionen, Entscheidung, Konsequenzen). ADRs sind
unveränderlich; Änderungen erfolgen über einen neuen ADR, der den alten ersetzt.

## Konsequenzen

- Entscheidungen sind im Repository versioniert und per Pull Request diskutierbar.
- `CLAUDE.md` verweist auf die ADRs; Änderungen, die einem ADR widersprechen,
  brauchen einen neuen ADR.
