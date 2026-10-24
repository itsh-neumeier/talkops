# Dashboard chart design

## Task state

- Task ID: 20261024-claude-chart-design
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: `claude/chart-design` (worktree `../talkops-charts`)
- Base commit: `9f69391`
- Related commits or PR: Pending

## Goal

PR 6 of 6 (in the agreed order: 1, 2, 4, 3, 6, then 5) from one user
request: a better design of the graphs. The user accepted: KPI trend against
the previous period (with a small backend change) and an area chart for 1
week and 1 month (automatic).

## Result

- KPI tiles: value, trend arrow with percent against the previous period
  (green = good; for missed calls lower is good), sparkline for calls and
  missed calls, ring gauge for the answer rate.
- Call chart: integer grid steps, top-rounded stacked bars (1h, 1d), smooth
  stacked areas with gradients (1w, 1m), hover/touch/arrow-key tooltip,
  column highlight, legend chips with totals that hide a series.
- Wi-Fi survey chart: gradient area under the RSSI line, round line joins,
  rounded loss bars.

## Changes

- `crates/talkops-core/src/stats.rs`: `PeriodTotals`, `period()`,
  `CallStats.previous` (period of the same length before the buckets);
  shared `answer_rate()`.
- `crates/talkops-api/tests/dashboard.rs`: checks `previous`.
- `web/src/lib/charts.ts` (+ `web/tests/charts.test.ts`): `niceScale`,
  `smoothPath` (monotone cubic), `areaPath`, `stack`, `trend`,
  `trendClass`, `sparkPoints`, `ringOffset`.
- `web/src/lib/components/CallChart.svelte` (rewrite),
  `KpiTile.svelte` (new), `WifiSurveyChart.svelte`, `routes/+page.svelte`,
  `api.ts` (`previous?`), i18n `dash.kpi.vsPrevious`, `dash.kpi.new`.
- `tests/browser/dashboard-charts.mjs` and a CI step.
- Docs: `docs/en/monitoring.md`, `docs/de/monitoring.md`.

## Decisions

- No chart library; plain SVG keeps the bundle small (as before).
- Monotone cubic curves: no overshoot below zero or above a peak.
- Gradients use `currentColor`, so Tailwind text colors work in both themes;
  gradient IDs use `$props.id()` for unique IDs.
- `previous` is optional in the TypeScript type, so an older server still
  works with the new UI (no trend).
- The average talk time trend is hidden when the period has no answered
  call.

## Checks

- Passed: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with PostgreSQL 16:
  506 passed, 1 ignored, 1 failed
  (`read_only_fax_directory_rejects_upload_without_queueing`; known, fails
  only as root, unrelated).
- Passed: `npm run lint`, `npm run check`, `npm test` (73), `npm run build`.
- Passed: browser `dashboard-charts.mjs` (1280 light/dark, 390),
  `active-calls.mjs`, `layout.mjs`; screenshots checked.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Not run: Docker stack, SIPp/E2E.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- No migration. PR #72 and PR #75 also change `web/src/routes/+page.svelte`
  (other cards); merge conflicts are possible but small.

## Next action

PR 5 (UniFi Talk style side panel for phones, extensions and users).

## References

- `docs/ai-memory/20261024-claude-dashboard-collapse.md`
