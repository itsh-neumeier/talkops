# Call origin map (number geolocation, map view, dashboard widget)

## Task state

- Task ID: 20261023-claude-call-map
- Date: 2026-10-23, Europe/Berlin
- Tool: Claude Code
- Status: Completed (3 PRs open; the release manager integrates them)
- Branch: claude/affectionate-lamport-v5iw3a (PR 1),
  claude/affectionate-lamport-v5iw3a-map-view (PR 2, stacked on PR 1),
  claude/affectionate-lamport-v5iw3a-map-widget (PR 3, stacked on PR 2)
- Base commit: a2478c8 (origin/main, TalkOps 1.8.7)
- Related commits or PR: [itsh-neumeier/talkops#65](https://github.com/itsh-neumeier/talkops/pull/65) (PR 1), [itsh-neumeier/talkops#66](https://github.com/itsh-neumeier/talkops/pull/66) (PR 2); PR 3 pending

## Goal

The call log (`/calls`) gets a list/map switch. The map shows where external
calls come from (inbound) and go to (outbound), down to the local network
area (DACH) and as precise as possible elsewhere (at minimum the country).
Ranges: 1 h, 1 day, 1 week, 1 month (30 days). The dashboard gets a compact
map widget (default 24 h). The user fixed the decisions in the development
prompt "Anrufkarte". Three PRs, one after the other:

1. Geo data, backend lookup, ADR 0044.
2. `GET /api/v1/calls/map` and the map view in the call log.
3. Dashboard widget and an optional tile URL in the system settings.

## Result

- PR 1: implemented, tested, pushed, PR #65 open.
- PR 2: implemented, tested, pushed, PR #66 open.
- PR 3: implemented and tested locally (see Changes, PR 3).

## Changes

- `tools/geo/fetch.sh`, `tools/geo/generate.py`, `tools/geo/requirements.txt`,
  `tools/geo/README.md`: reproducible generator. Raw data is not in Git.
- `presets/geo/*.tsv`: data files. `ch.tsv` and `service.tsv` by hand, the
  others generated. `presets/geo/README.md` has the format.
- `crates/talkops-core/src/geo.rs`: `GeoIndex` (load, longest-prefix match),
  `GeoArea`, `GeoKind`, `GeoSource`, `GeoError`. Unit tests with sample data
  and with the shipped data.
- `crates/talkops-api`: `TALKOPS_GEO_DIR` (`config.rs`), `AppState::geo`,
  `with_geo` (`lib.rs`), `load_geo` (`main.rs`), `Directory::locate`
  (`active_calls.rs`, tenant normalization before the lookup).
- `docker/talkops/Dockerfile`, `docker-compose.yml`, `.github/workflows/ci.yml`,
  `CLAUDE.md`: `TALKOPS_GEO_DIR`.
- `docs/adr/0044-call-origin-map-and-number-geolocation.md`, ADR index,
  `THIRD_PARTY_NOTICES.md`.

### PR 2

- `crates/talkops-api/src/routes/call_map.rs`: `GET /api/v1/calls/map`
  (`range`, `categories`), `Place`/`place()` (anonymous, area, no location),
  `area_call_ids()` for the list filter. Aggregation in Rust after the lookup.
- `crates/talkops-core/src/cdr.rs`: `list_window` (limit `WINDOW_LIMIT` =
  50,000), `CdrScope::ids`, `CdrQuery::area`.
- `crates/talkops-api/src/routes/settings.rs`: `own_extensions()` shared by
  the list and the map; `GET /api/v1/calls?area=…`.
- `crates/talkops-api/tests/call_map.rs`: scope, categories, ranges,
  internal calls, no location, anonymous, area filter, missing geo data.
- Web: `web/src/lib/calls/map.ts` (URL state, bubble size and segments),
  `filters.ts` (`range`, `area`, `place`), `CallMap.svelte` (Leaflet, lazy),
  `CallMapView.svelte`, `CallMapPopup.svelte`, `CountryFlag.svelte`,
  `routes/calls/+page.svelte` (tabs list/map, `afterNavigate`), `app.css`,
  i18n keys `calls.view*`, `calls.areaFilter`, `calls.rangeFilter`, `callMap.*`.
- `web/static/map/countries.geojson` (Natural Earth 1:50m, simplified, 0.48 MB).
- Tests: `web/tests/callMap.test.ts`, `tests/browser/call-map.mjs` (in CI).
- Docs: `docs/en/call-log.md`, `docs/de/anrufliste.md` (section Map/Karte, API).

### PR 3

- `migrations/20261109000001_call_map_tiles.sql`: `tenant_settings.map_tile_url`,
  `map_tile_attribution` (checks: `https://`, length).
- `crates/talkops-core/src/settings.rs`: `MapTiles`, `get_map_tiles`, `set_map_tiles`.
- `crates/talkops-api/src/routes/call_map.rs`: `GET/PUT /api/v1/settings/call-map`
  (PUT: `settings.manage`, audit `call_map_tiles`), `tile_source()` validation.
- `crates/talkops-api/src/lib.rs`: `map_tile_csp` middleware adds the tile host
  to `img-src` of the CSP meta tag in `index.html`.
- Web: `CallMapWidget.svelte` (dashboard, lazy, 1 h / 1 day, top 5),
  `CallMapSettings.svelte` (settings → call handling), `#lib/calls/tiles.ts`,
  `tileUrlProblem`, `topAreas` in `map.ts`, tile layer in `CallMap.svelte`.
- Tests: Rust `tile_server_setting_is_validated_audited_and_allowed_by_the_page_policy`,
  unit test `validates_tile_urls_and_derives_the_csp_source`, web unit tests,
  browser checks for the widget and the tile layer in `call-map.mjs`.
- Docs: `docs/en/system-settings.md`, `docs/de/systemeinstellungen.md`,
  call log docs (dashboard, tiles), ADR 0044 (tile setting and CSP).

## Decisions

- Data sources: BNetzA ONB GIS (2026-08-10) and area code list (DE), RTR
  `rnOrtsnetze` (AT), BAKOM by hand (CH), libphonenumber v9.0.42 geocoding
  and carrier data (other countries, mobile ranges), GeoNames, Natural Earth.
- Static TSV files, no database table. A missing directory gives a warning
  and an empty index; invalid data stops the start.
- Geocoding prefixes inside mobile ranges are ignored (China publishes the
  home region of mobile ranges; this is not a location).
- libphonenumber has no English geocoding for France and some other
  countries; these numbers resolve to the country.
- Areas of kind `mobile`, `service` and `country` form one "mobile / no
  location" bubble per country (PR 2).
- PRs are stacked branches because the user asked for three PRs.
- Map bubbles are Leaflet HTML markers (`divIcon` with a `conic-gradient`),
  not canvas: they get keyboard focus, Enter opens the details panel.
- The details are a panel below the map, not a Leaflet popup (keyboard,
  Esc, mobile layout).
- The popup link uses a rolling `range` and an `area` ID. The API resolves
  `area` to call IDs (same window limit) and filters the list with them.
- Validation errors return 422, as everywhere in the API.
- The tile setting has its own endpoint, so `PUT /api/v1/settings` cannot
  clear it. It is in the settings section "Call handling" (`settings.manage`),
  because the section "System" needs `diagnostics.manage`.
- The CSP is a meta tag; the server patches `index.html` because a header
  cannot extend the policy. Conditional requests are ignored for this
  response so a cached page does not keep the old policy.
- A tile server needs an attribution text.

## Checks

- Passed: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test -p talkops-core --lib geo` (5 tests), `cargo test -p talkops-api --lib active_calls`.
- PR 2 passed: clippy, `cargo test -p talkops-api --test call_map --test call_blocking`,
  `cargo test -p talkops-core --test domain`, `npm run lint/check/build/test`,
  browser tests `call-map`, `call-list`, `layout`, `call-presentation`, `motion`,
  `active-calls`, `call-blocking` against `vite preview` (Playwright 1.56.1).
- PR 2 bundle (uncompressed, measured in Chromium): `/calls` list view +13.5 KB JS,
  +2.4 KB CSS; dashboard +7 KB JS (i18n). The map view loads +179 KB JS and +15 KB CSS
  lazily, plus the 0.48 MB base map.
- PR 3 passed: clippy, `cargo test -p talkops-api --test call_map`, `--lib call_map`,
  web lint/check/test/build, browser tests `call-map` (widget, tile layer), `call-list`,
  `layout`, `call-presentation`, `motion`, `active-calls`, `call-blocking`,
  `settings-system`, `roles`.
- Measured: data 2.7 MB, 19,584 areas, 81,687 prefixes; load 78 ms (release), 260 ms (debug).
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- No migration in PR 1.
- Migration in PR 3: `20261109000001_call_map_tiles.sql`.
- Merge order: #65, then PR 2, then PR 3 (stacked branches).

## Next action

Release manager: review and integrate the three PRs in order.

## References

- `docs/ai-memory/20261015-claude-call-log-filters.md`
- `docs/ai-memory/20261016-claude-dashboard-recent-calls.md`
- `docs/adr/0044-call-origin-map-and-number-geolocation.md`
