// Browser checks of the settings sections "System" (sub-tabs with deep links)
// and "Backup" against a production preview with API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const at = '2026-10-16T09:00:00Z';
const capture = { running: false, level: null, sip_trace: false, started_at: null, until: null, lines: 0, last_seq: 0 };
const fixtures = {
  '/api/v1/setup': { needs_setup: false },
  '/api/v1/status': { version: 'test', database: { ok: true }, freeswitch: { ok: true } },
  '/api/v1/settings': { emergency_numbers: [] },
  '/api/v1/diagnostics': { version: '1.7.14', server_started_at: at, freeswitch: 'UP 0 years, 1 days, 2 hours, 3 minutes', capture, channels: [] },
  '/api/v1/diagnostics/log': { lines: [], capture },
  '/api/v1/telephony/status': { connected: true, registrations: [{ user: '101', network_ip: '192.0.2.10', network_port: '5060', transport: 'udp', user_agent: 'Yealink T54W', expires: 60 }], gateways: {}, updated_at: at },
  '/api/v1/trunks': [{ id: 't1', name: 'Telekom', preset: 'telekom', overrides: {}, enabled: true, video_enabled: false, numbers: [], accounts: [{ id: 'a1', trunk_id: 't1', username: '+4995431241000', auth_username: 'x', enabled: true, destination_type: 'none', destination_id: null, gateway: 'gw1', state: { name: 'gw1', state: 'FAIL_WAIT', status: 'DOWN', last_error: '403 Forbidden' } }] }],
  '/api/v1/backups': { settings: { enabled: true, hour: 3, keep: 7, include_recordings: true, last_run_at: null, last_file: null, last_error: null }, files: [], running: false, configuration: null, configuration_error: null, encryption_configured: false, mounted: false, auto_discovery: true, portainer: null },
  '/api/v1/backup-targets': { targets: [], roots: [], encryption_configured: false }
};

async function open(permissions) {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const me = { user: { id: 'u1', username: 'ops', display_name: 'Ops', role: 'user', role_id: 'r1', enabled: true }, csrf_token: 'x', permissions };
  await page.route('**/api/**', route => {
    const path = new URL(route.request().url()).pathname;
    return route.fulfill({ status: 200, json: path === '/api/v1/auth/me' ? me : fixtures[path] ?? [] });
  });
  return { context, page, errors };
}

try {
  // Diagnostics only: "System" with four tabs, no "Backup" section.
  const a = await open(['diagnostics.manage', 'calls.active', 'trunks.view']);
  await a.page.goto(`${BASE}/settings?section=system`);
  const tabs = a.page.getByRole('tab');
  await tabs.first().waitFor({ timeout: 20_000 });
  assert.deepEqual(await tabs.allTextContents(), ['Status', 'Diagnose', 'Sitzungen', 'Wartung']);
  const nav = a.page.getByRole('navigation', { name: 'Einstellungen' });
  assert.equal(await nav.getByRole('button', { name: /Backup/ }).count(), 0);
  await a.page.getByText('403 Forbidden').waitFor();
  await a.page.getByText('Yealink T54W').waitFor();
  await a.page.getByRole('tab', { name: 'Diagnose' }).click();
  await a.page.getByTestId('log').waitFor();
  assert.match(a.page.url(), /\?section=system&tab=diagnostics$/);
  await a.page.getByRole('tab', { name: 'Diagnose' }).press('ArrowRight');
  await a.page.getByRole('button', { name: 'Alle Sitzungen trennen' }).waitFor();
  assert.match(a.page.url(), /tab=sessions$/);
  // Deep link straight to a tab.
  await a.page.goto(`${BASE}/settings?section=system&tab=maintenance`);
  await a.page.getByRole('button', { name: 'FreeSWITCH neu starten' }).waitFor({ timeout: 20_000 });
  assert.equal(await a.page.getByRole('tab', { selected: true }).textContent(), 'Wartung');
  await a.page.setViewportSize({ width: 360, height: 780 });
  const width = await a.page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(width[0] <= width[1], JSON.stringify(width));
  assert.deepEqual(a.errors, []);
  await a.context.close();

  // Backups only: an old "#system" link opens the "Backup" section.
  const b = await open(['backups.manage']);
  await b.page.goto(`${BASE}/settings#system`);
  await b.page.getByRole('heading', { name: 'Sicherungsziele' }).waitFor({ timeout: 20_000 });
  const navB = b.page.getByRole('navigation', { name: 'Einstellungen' });
  assert.equal(await navB.getByRole('button', { name: /System/ }).count(), 0);
  assert.equal(await navB.getByRole('button', { name: /Backup/ }).getAttribute('aria-current'), 'page');
  assert.equal(await b.page.getByRole('tab').count(), 0);
  assert.deepEqual(b.errors, []);
  await b.context.close();
  console.log('settings system browser checks passed');
} finally {
  await browser.close();
}
