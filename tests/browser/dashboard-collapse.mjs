// Dashboard lists against a production preview with API fixtures: calls in
// progress and recent calls show three entries, expand on request, keep a
// ringing call visible and remember the open state after a reload.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const now = new Date().toISOString();
const party = (name, number, extension_id = null) => ({ name, number, extension_id, ambiguous: false });
const active = Array.from({ length: 5 }, (_, i) => ({
  uuid: `a${i}`, direction: 'inbound', caller: party(`Anrufer ${i + 1}`, `+49954312410${i}`), called: party('Empfang', '101', 'e1'),
  started_at: now, answered_at: i === 4 ? null : now, state: i === 4 ? 'ringing' : 'talking', transcript_available: false
}));
const recent = Array.from({ length: 8 }, (_, i) => ({
  id: `c${i}`, call_uuid: `c${i}`, direction: i % 2 ? 'outbound' : 'inbound', caller_number: '+4995431241000', caller_name: `Kontakt ${i + 1}`,
  destination: '101', extension_id: 'e1', started_at: now, answered_at: now, ended_at: now, duration_secs: 61, billsec: 60,
  hangup_cause: 'NORMAL_CLEARING', recording_id: null, number_id: null, via_number: null,
  caller: party(`Kontakt ${i + 1}`, '+4995431241000'), called: party('Empfang', '101', 'e1')
}));
const problems = [];
try {
  for (const width of [390, 1280]) {
    const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width, height: 1000 } });
    await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
    await context.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      let value = [];
      if (path === '/api/v1/auth/me') value = { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
      else if (path === '/api/v1/setup') value = { needs_setup: false };
      else if (path === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
      else if (path === '/api/v1/telephony/status') value = { connected: true, registrations: [], gateways: {}, updated_at: now };
      else if (path === '/api/v1/stats/calls') value = { range: '1d', buckets: [], total: 8, missed: 0, avg_talk_secs: 60, answer_rate: 100, inbound: 4, outbound: 4, internal: 0 };
      else if (path === '/api/v1/backups') value = { settings: { last_run_at: null } };
      else if (path === '/api/v1/telephony/calls') value = active;
      else if (path === '/api/v1/calls') value = recent;
      await route.fulfill({ status: 200, json: value });
    });
    const page = await context.newPage();
    page.on('pageerror', (e) => problems.push(`${width}: ${e.message}`));
    await page.goto(`${BASE}/`);
    const activeList = page.locator('#dash-active-list > li');
    const recentList = page.locator('#dash-recent-list > li');
    await recentList.first().waitFor();
    await activeList.first().waitFor();
    // Three calls plus the ringing fifth call.
    assert.equal(await activeList.count(), 4);
    assert.match(await activeList.nth(3).textContent(), /Anrufer 5/);
    assert.equal(await recentList.count(), 3);
    const moreActive = page.locator('[aria-controls="dash-active-list"]');
    const moreRecent = page.locator('[aria-controls="dash-recent-list"]');
    assert.equal((await moreActive.textContent()).trim(), '1 weitere anzeigen');
    assert.equal((await moreRecent.textContent()).trim(), '5 weitere anzeigen');
    await moreRecent.click();
    assert.equal(await recentList.count(), 8);
    assert.equal(await moreRecent.getAttribute('aria-expanded'), 'true');
    assert.equal((await moreRecent.textContent()).trim(), 'Weniger anzeigen');
    await mkdir('test-results', { recursive: true });
    await page.screenshot({ path: `test-results/dashboard-collapse-${width}.png`, fullPage: true });
    await page.reload();
    await recentList.first().waitFor();
    assert.equal(await recentList.count(), 8, 'open state kept after reload');
    assert.equal(await activeList.count(), 4, 'active list stays collapsed');
    await moreRecent.click();
    assert.equal(await recentList.count(), 3);
    await context.close();
  }
  assert.deepEqual(problems, []);
  console.log('dashboard collapse browser checks passed');
} finally {
  await browser.close();
}
