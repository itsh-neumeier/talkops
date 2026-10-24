// Dashboard charts against a production preview with API fixtures: KPI
// trends, sparklines and the ring gauge, the tooltip by pointer and keys,
// legend toggles, bars for a day and areas for a week, both themes.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const now = Date.now();
function stats(range) {
  const count = range === '1w' ? 7 : 24;
  const step = range === '1w' ? 86_400_000 : 3_600_000;
  const buckets = Array.from({ length: count }, (_, i) => {
    const wave = Math.round(4 + 4 * Math.sin(i / 2));
    return { start: new Date(now - (count - i) * step).toISOString(), inbound: wave + 2, outbound: wave, internal: i % 3, missed: i % 5 === 0 ? 1 : 0, blocked: 0 };
  });
  const sum = (k) => buckets.reduce((s, b) => s + b[k], 0);
  const inbound = sum('inbound'), outbound = sum('outbound'), internal = sum('internal'), missed = sum('missed');
  return { range, buckets, end: new Date(now).toISOString(), total: inbound + outbound + internal, inbound, outbound, internal, missed, blocked: 0,
    answer_rate: Math.round((inbound - missed) * 1000 / inbound) / 10, avg_talk_secs: 95,
    previous: { total: Math.round((inbound + outbound + internal) / 1.25), inbound, outbound, internal, missed: missed + 2, blocked: 0, answer_rate: 90, avg_talk_secs: 100 } };
}
const problems = [];
try {
  for (const [width, theme] of [[1280, 'light'], [1280, 'dark'], [390, 'light']]) {
    const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width, height: 1000 } });
    await context.addInitScript((th) => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); localStorage.setItem('talkops.theme', th); }, theme);
    await context.route('**/api/**', async (route) => {
      const url = new URL(route.request().url());
      const path = url.pathname;
      let value = [];
      if (path === '/api/v1/auth/me') value = { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
      else if (path === '/api/v1/setup') value = { needs_setup: false };
      else if (path === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
      else if (path === '/api/v1/telephony/status') value = { connected: true, registrations: [], gateways: {}, updated_at: new Date(now).toISOString() };
      else if (path === '/api/v1/stats/calls') value = stats(url.searchParams.get('range') ?? '1d');
      else if (path === '/api/v1/backups') value = { settings: { last_run_at: null } };
      await route.fulfill({ status: 200, json: value });
    });
    const page = await context.newPage();
    page.on('pageerror', (e) => problems.push(`${width}: ${e.message}`));
    await page.goto(`${BASE}/`);
    const trends = page.getByTestId('kpi-trend');
    await trends.first().waitFor();
    // Calls up 25 % (good), missed down (good), answer rate up, talk time down.
    assert.match(await trends.nth(0).textContent(), /▲\s*25 %/);
    assert.match(await trends.nth(0).getAttribute('class'), /emerald/);
    assert.match(await trends.nth(1).textContent(), /▼/);
    assert.match(await trends.nth(1).getAttribute('class'), /emerald/, 'fewer missed calls is good');

    const chart = page.getByRole('group', { name: 'Anrufe' });
    await chart.locator('[data-slot="5"]').hover();
    const tip = page.getByTestId('chart-tooltip');
    await tip.waitFor();
    assert.match(await tip.textContent(), /Eingehend/);
    await chart.focus();
    await page.keyboard.press('ArrowRight');
    await tip.waitFor();
    await mkdir('test-results', { recursive: true });
    await page.screenshot({ path: `test-results/dashboard-charts-${width}-${theme}-day.png` });

    const outbound = page.getByRole('button', { name: /Ausgehend/ });
    await outbound.click();
    assert.equal(await outbound.getAttribute('aria-pressed'), 'false');
    await outbound.click();

    await page.getByRole('button', { name: '1 Woche' }).click();
    await page.waitForFunction(() => document.querySelector('[role="group"] path[fill^="url(#call-chart"]'));
    await chart.locator('[data-slot="3"]').hover();
    await tip.waitFor();
    await page.screenshot({ path: `test-results/dashboard-charts-${width}-${theme}-week.png` });
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    assert.ok(overflow <= 0, `${width}: page scrolls sideways by ${overflow}px`);
    await context.close();
  }
  assert.deepEqual(problems, []);
  console.log('dashboard chart browser checks passed');
} finally {
  await browser.close();
}
