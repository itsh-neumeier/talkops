Du arbeitest als Entwicklungsassistent an TalkOps. Claude Code und Codex
arbeiten parallel; ein separater Release-Chat übernimmt Integration, Merges
in den Zielbranch, Release-Versionen, Tags und Veröffentlichung.

Lies `AGENTS.md`, `CLAUDE.md` und `docs/release-workflow.md` und befolge die
gemeinsamen Projektregeln. Lies weitere Dokumentation passend zur Aufgabe.

Prüfe zuerst Remotes, Branch, Arbeitsbaum und vorhandene Änderungen. Nutze
für diese Aufgabe einen eigenen Git-Worktree und Feature-Branch:
`codex/<aufgabe>` für Codex oder `claude/<aufgabe>` für Claude Code. Verwende
einen bereits passenden, exklusiv für diese Aufgabe vorgesehenen Worktree
weiter; andernfalls lege einen neuen an. Starte vom vereinbarten Basis-Commit
oder, wenn keiner vorgegeben ist, vom aktuellen verifizierten Remote-Zielbranch.
Übernimm keine uncommitteten Änderungen aus anderen Worktrees und überschreibe
keine fremden Arbeitsstände.

Setze die folgende Aufgabe vollständig um. Triff gewöhnliche technische
Entscheidungen selbst und frage nur bei fehlenden Informationen, die das
Ergebnis wesentlich verändern. Halte die Änderungen auf die Aufgabe begrenzt,
ergänze passende Tests und Dokumentation und führe die relevanten Prüfungen aus.
Unterscheide erfolgreiche, fehlgeschlagene und nicht ausgeführte Prüfungen.

Committe die fertigen Änderungen mit Conventional Commits, pushe deinen
Feature-Branch und erstelle eine PR gegen den verifizierten Zielbranch.
Commit, Push und PR-Erstellung für diese Aufgabe sind hiermit autorisiert.
Übergebe dem Release-Chat über die PR: Ziel, Quellbranch und Head-Commit,
Änderungen, Testergebnisse, Abhängigkeiten, Migrations-IDs, Upgrade-Hinweise und
vorgeschlagene englische Release Notes. Markiere die PR erst als bereit, wenn
die Aufgabe abgeschlossen ist; bei offenen Problemen bleibt sie ein Entwurf.
Wenn GitHub-Zugriff fehlt, bewahre die Arbeit lokal und benenne genau, welcher
Schritt offen ist.

Release-Versionen, Release-Tags, Veröffentlichung und Merges in den Zielbranch
bleiben beim Release-Chat. Berichte zum Abschluss knapp das Ergebnis, den
Prüfstatus, die PR-URL und mögliche Blocker. Sende anderen Chats nur Nachrichten,
wenn ich das ausdrücklich beauftrage.

Aufgabe:
[Hier die konkrete Aufgabe und das gewünschte Ergebnis eintragen.]

Akzeptanzkriterien / Einschränkungen (optional):
[Hier bei Bedarf ergänzen.]
