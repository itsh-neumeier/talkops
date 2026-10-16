// Browser checks of the audit log page: filters and sort go to the API,
// paging, download links with the filter, and the retention and export tab
// (audit.manage only), against a production preview with API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const at = '2026-10-16T09:00:00Z';
const entry = (id, action, entity) => ({ id, user_id: 'u1', username: 'admin', action, entity_type: entity, entity_id: `${entity}-${id}`, details: { name: `n${id}` }, ip: '192.0.2.1', created_at: at });
const settings = {
  retention_days: 365, export_enabled: true, storage_target_id: 's2', folder: 'audit', format: 'json',
  schedule: { weekdays: 127, hour: 2, minute: 15, every_hours: 0 },
  retention: { keep_last: 30, keep_daily: 0, keep_weekly: 0, keep_monthly: 0, keep_yearly: 0 },
  last_exported_id: 120, last_run_at: at, last_success_at: at, last_error: null, created_at: at,
  running: false, next_run_at: at, transfers_available: true,
  files: [{ id: 1, storage_target_id: 's2', file: 'talkops-audit-20261016-021500-1-120.jsonl', first_id: 1, last_id: 120, entries: 120, bytes: 40960, sha256: 'ab'.repeat(32), created_at: at }]
};

async function open(permissions) {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const queries = []; const puts = [];
  const me = { user: { id: 'u1', username: 'ops', display_name: 'Ops', role: 'user', role_id: 'r1', enabled: true }, csrf_token: 'x', permissions };
  await page.route('**/api/**', route => {
    const url = new URL(route.request().url());
    const path = url.pathname;
    if (path === '/api/v1/auth/me') return route.fulfill({ json: me });
    if (path === '/api/v1/setup') return route.fulfill({ json: { needs_setup: false } });
    if (path === '/api/v1/audit') {
      queries.push(url.searchParams);
      const offset = Number(url.searchParams.get('offset') ?? 0);
      const rows = Array.from({ length: 50 }, (_, i) => entry(200 - offset - i, i % 2 ? 'update' : 'create', 'extension'));
      return route.fulfill({ json: rows, headers: { 'X-Total-Count': '120', 'Access-Control-Expose-Headers': 'X-Total-Count' } });
    }
    if (path === '/api/v1/audit/facets') return route.fulfill({ json: { actions: ['create', 'update'], entity_types: ['extension', 'trunk'], users: [{ id: 'u1', username: 'admin' }] } });
    if (path === '/api/v1/audit/settings') {
      if (route.request().method() === 'PUT') { puts.push(route.request().postDataJSON()); }
      return route.fulfill({ json: settings });
    }
    if (path === '/api/v1/storage-targets') return route.fulfill({ json: { targets: [{ id: 's2', name: 'USB', config: { kind: 'local' }, path: '/var/lib/talkops/targets/usb', allow_insecure: false, bwlimit_kbps: null, has_secret: false, last_test_at: null, last_test_ok: null, last_test_error: null, created_at: at, updated_at: at, used_by: [] }], roots: [], encryption_configured: false } });
    return route.fulfill({ json: [] });
  });
  return { context, page, errors, queries, puts };
}

try {
  // audit.view only: entries, no settings tab.
  const a = await open(['audit.view']);
  await a.page.goto(`${BASE}/audit`);
  await a.page.getByText('120 Einträge').waitFor({ timeout: 20_000 });
  assert.equal(await a.page.getByRole('tab').count(), 0);
  assert.equal(await a.page.locator('tbody tr').count(), 50);
  await a.page.getByText('Seite 1 von 3').waitFor();
  await a.page.getByRole('button', { name: 'Weiter' }).click();
  await a.page.getByText('Seite 2 von 3').waitFor();
  assert.equal(a.queries.at(-1).get('offset'), '50');

  await a.page.getByLabel('Aktion').selectOption('update');
  await a.page.getByText('Seite 1 von 3').waitFor();
  let q = a.queries.at(-1);
  assert.equal(q.get('action'), 'update');
  assert.equal(q.get('offset'), '0');
  await a.page.getByRole('button', { name: /Benutzer/ }).click();
  await a.page.waitForFunction(() => document.querySelector('th[aria-sort="ascending"]'));
  q = a.queries.at(-1);
  assert.equal(q.get('sort'), 'user');
  assert.equal(q.get('order'), 'asc');
  await a.page.getByLabel('Suche').fill('ext-1');
  await a.page.waitForTimeout(500);
  assert.equal(a.queries.at(-1).get('search'), 'ext-1');
  const csv = await a.page.getByRole('link', { name: /CSV/ }).getAttribute('href');
  const params = new URL(csv, BASE).searchParams;
  assert.equal(params.get('format'), 'csv');
  assert.equal(params.get('action'), 'update');
  assert.equal(params.get('search'), 'ext-1');
  await a.page.setViewportSize({ width: 360, height: 780 });
  const w = await a.page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(w[0] <= w[1], JSON.stringify(w));
  assert.deepEqual(a.errors, []);
  await a.context.close();

  // audit.manage: retention and export tab.
  const b = await open(['audit.view', 'audit.manage']);
  await b.page.goto(`${BASE}/audit`);
  await b.page.getByRole('tab', { name: 'Aufbewahrung und Export' }).click();
  await b.page.getByText('talkops-audit-20261016-021500-1-120.jsonl').waitFor({ timeout: 20_000 });
  assert.equal(await b.page.getByLabel('Einträge behalten (Tage)').inputValue(), '365');
  assert.equal(await b.page.getByLabel('Speicherziel', { exact: true }).inputValue(), 's2');
  await b.page.getByLabel('Einträge behalten (Tage)').fill('90');
  await b.page.getByRole('button', { name: 'Speichern' }).click();
  await b.page.getByText('Gespeichert.').waitFor();
  assert.equal(b.puts[0].retention_days, 90);
  assert.equal(b.puts[0].storage_target_id, 's2');
  assert.equal(b.puts[0].format, 'json');
  await b.page.setViewportSize({ width: 360, height: 780 });
  const w2 = await b.page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(w2[0] <= w2[1], JSON.stringify(w2));
  assert.deepEqual(b.errors, []);
  await b.context.close();
  console.log('audit browser checks passed');
} finally {
  await browser.close();
}
