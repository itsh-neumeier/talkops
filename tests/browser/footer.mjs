// Fixed footer: copyright and version for every user, the system load only
// with diagnostics access. The footer must not cover the page content or the
// side panel.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const extensions = Array.from({ length: 40 }, (_, i) => ({ id: `e${i}`, number: String(100 + i), display_name: `Nebenstelle ${i}`, user_id: null, outbound_number_id: null, hide_caller_id: false, ring_timeout_secs: 30, enabled: true, dnd: false, forward_all: null, record_calls: 'inherit', video_enabled: false }));
const load = { cpu_percent: 12.4, cpus: 4, load1: 0.42, load5: 0.31, load15: 0.2, mem_used_bytes: 2 * 1024 ** 3, mem_total_bytes: 8 * 1024 ** 3, uptime_secs: 3 * 86_400 + 7_200 };
let loadCalls = 0;
async function context(width, role) {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width, height: 700 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  await context.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    let value = [];
    if (path === '/api/v1/auth/me') value = role === 'admin'
      ? { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' }
      : { user: { id: 'u2', username: 'user', display_name: 'User', role: 'user', enabled: true, permissions: ['extensions.view'] }, csrf_token: 'fixture' };
    else if (path === '/api/v1/setup') value = { needs_setup: false };
    else if (path === '/api/v1/status') value = { version: '9.9.9', database: { ok: true }, freeswitch: { ok: true } };
    else if (path === '/api/v1/system/load') { loadCalls += 1; value = load; }
    else if (path === '/api/v1/extensions') value = extensions;
    else if (path === '/api/v1/extensions/e0') value = { ...extensions[0], devices: [] };
    await route.fulfill({ status: 200, json: value });
  });
  return context;
}
const problems = [];
try {
  for (const width of [390, 1280]) {
    const ctx = await context(width, 'admin');
    const page = await ctx.newPage();
    page.on('pageerror', (e) => problems.push(`${width}: ${e.message}`));
    await page.goto(`${BASE}/extensions`);
    const footer = page.getByTestId('app-footer');
    await footer.getByText('TalkOps 9.9.9').waitFor();
    assert.match(await footer.textContent(), new RegExp(`© ${new Date().getFullYear()} ITSH Neumeier`));
    await footer.getByText('12 %').waitFor();
    if (width >= 640) assert.match(await footer.textContent(), /0,42 \/ 0,31 \/ 0,20/);
    assert.match(await footer.textContent(), /RAM\s*25 %/);

    // The footer stays at the bottom and the last row is not under it.
    await page.locator('tbody tr').last().scrollIntoViewIfNeeded();
    await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
    const box = await footer.boundingBox();
    assert.equal(Math.round(box.y + box.height), 700, `${width}: footer is not at the bottom`);
    const last = await page.locator('tbody tr').last().boundingBox();
    assert.ok(last.y + last.height <= box.y + 1, `${width}: footer covers the last row`);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    assert.ok(overflow <= 0, `${width}: page scrolls sideways by ${overflow}px`);

    // On a wide screen the side panel ends above the footer.
    if (width >= 1024) {
      await page.goto(`${BASE}/extensions?panel=e0`);
      const panel = await page.getByTestId('side-panel').boundingBox();
      const foot = await footer.boundingBox();
      assert.ok(panel.y + panel.height <= foot.y + 1, 'side panel covers the footer');
    }
    await mkdir('test-results', { recursive: true });
    await page.screenshot({ path: `test-results/footer-${width}.png` });
    await ctx.close();
  }

  // Without diagnostics access: no system load and no request for it.
  loadCalls = 0;
  const ctx = await context(1280, 'user');
  const page = await ctx.newPage();
  await page.goto(`${BASE}/`);
  const footer = page.getByTestId('app-footer');
  await footer.getByText('TalkOps 9.9.9').waitFor();
  assert.doesNotMatch(await footer.textContent(), /CPU|RAM/);
  assert.equal(loadCalls, 0);
  await ctx.close();

  assert.deepEqual(problems, []);
  console.log('footer browser checks passed');
} finally {
  await browser.close();
}
