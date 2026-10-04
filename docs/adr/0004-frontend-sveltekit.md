# 0004 – Frontend: SvelteKit-SPA, ausgeliefert vom Rust-Server

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

Das Web-UI umfasst Admin-Oberfläche, Self-Service-Portal, Live-Ansichten
(aktive Gespräche, Registrierungen über WebSocket), einen grafischen
IVR-Editor und ein WebRTC-Softphone mit Video. Anforderungen: TypeScript,
Dark/Light-Mode, Deutsch + Englisch, mobile-tauglich, kein eigener
Node-Server im Betrieb.

## Optionen

1. **SvelteKit (Svelte 5) mit `adapter-static`** – kleine Bundles, wenig
   Boilerplate, gute TypeScript-Unterstützung; als SPA statisch baubar.
2. **React (Vite) / Next.js** – größtes Ökosystem; Next.js bringt einen
   Node-Server mit, den wir nicht wollen; reines React mehr Boilerplate.
3. **Server-gerendert in Rust (askama + htmx)** – kein JS-Build, aber
   Softphone (SIP.js/JsSIP, WebRTC) und IVR-Editor brauchen ohnehin viel
   Client-Logik.

## Entscheidung

**SvelteKit + TypeScript + Tailwind CSS**, gebaut als **SPA** (`ssr = false`,
`fallback: index.html`). Der Rust-Server liefert `web/build` statisch aus und
fällt für unbekannte Pfade auf `index.html` zurück. Die API liegt unter
`/api/v1`, dadurch gleiche Origin (keine CORS-Konfiguration nötig).

- i18n: kleine eigene, typisierte Lösung (`web/src/lib/i18n`), Englisch ist
  der Referenzkatalog; der TypeScript-Typ erzwingt vollständige Übersetzungen.
- Dark Mode über `dark`-Klasse am `<html>`, Systemeinstellung als Default.
- Softphone ab Phase 7 mit SIP.js oder JsSIP (Auswahl dann per eigenem ADR).

## Konsequenzen

- Build-Zeit-Abhängigkeit auf Node.js; das Laufzeit-Image enthält nur statische Dateien.
- Es gibt kein SSR → SEO irrelevant, Auth läuft vollständig über die API (Cookies).
- SvelteKit 3 hat die Konfiguration in `vite.config.ts` verlagert und `$lib`
  durch `#lib` (package.json `imports`) ersetzt – siehe `CLAUDE.md`.
