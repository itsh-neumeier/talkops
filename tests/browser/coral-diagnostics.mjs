// Coral capture controls, explicit text consent and JSON download.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
try {
  const context = await browser.newContext({ locale: 'de-DE', reducedMotion: 'reduce', viewport: { width: 1100, height: 900 } });
  await context.addInitScript(() => localStorage.setItem('talkops.locale', 'de'));
  const page = await context.newPage();
  const errors = []; page.on('pageerror', e => errors.push(e.message));
  const requests = []; let running = false;
  await page.route('**/api/**', async route => {
    const req = route.request(); const url = new URL(req.url()); const path = url.pathname;
    let value = [];
    if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (path === '/api/v1/settings') value = { emergency_numbers: [] };
    else if (path === '/api/v1/diagnostics') value = { capture: { running: false, lines: 0 }, channels: [] };
    else if (path === '/api/v1/diagnostics/log') value = { capture: { running: false, lines: 0 }, lines: [] };
    else if (path === '/api/v1/diagnostics/coral') value = [{ id: 'coral1', name: 'Coral', kind: 'coral' }];
    else if (path === '/api/v1/diagnostics/coral/coral1') value = { capture: { running, events_kept: running ? 20 : 0, until_unix_ms: Date.now() + 300000 } };
    else if (path.endsWith('/coral1/capture')) { requests.push(req.method() === 'POST' ? req.postDataJSON() : 'stop'); running = req.method() === 'POST'; value = { running }; }
    else if (path.endsWith('/coral1/download')) { requests.push({ include_text: url.searchParams.get('include_text') }); value = { schema_version: 1, capture: { running } }; }
    await route.fulfill({ status: 200, json: value });
  });
  await page.goto(`${BASE}/settings?section=system&tab=diagnostics`);
  const card = page.getByTestId('coral-diagnostics');
  await card.getByRole('button', { name: 'Erfassung starten' }).waitFor();
  assert.equal(await card.getByRole('checkbox').isChecked(), false);
  await card.getByRole('button', { name: 'Erfassung starten' }).click();
  await card.getByRole('status').getByText('Erfassung aktiv bis', { exact: false }).waitFor();
  assert.deepEqual(requests[0], { minutes: 5, include_text: false });
  await card.getByRole('checkbox').check();
  const downloaded = page.waitForEvent('download');
  await card.getByTestId('coral-debug-download').click();
  assert.equal((await downloaded).suggestedFilename(), 'talkops-coral-diagnostics.json');
  assert.deepEqual(requests[1], { include_text: 'true' });
  await card.getByRole('button', { name: 'Stoppen', exact: true }).click();
  assert.equal(requests[2], 'stop');
  await mkdir('test-results', { recursive: true });
  await page.screenshot({ path: 'test-results/coral-diagnostics-desktop.png', fullPage: true });
  await page.setViewportSize({ width: 360, height: 800 });
  const width = await page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(width[0] <= width[1], JSON.stringify(width));
  await page.screenshot({ path: 'test-results/coral-diagnostics-mobile.png', fullPage: true });
  assert.deepEqual(errors, []);
} finally { await browser.close(); }
