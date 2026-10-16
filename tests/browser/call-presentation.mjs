// Country flags and loading behavior against production assets and API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const local = { name: 'Office', number: '+4995431241', extension_id: 'e-101', ambiguous: false };
const german = { name: 'Maria', number: '+4995436999124', extension_id: null, contact_id: 'maria', ambiguous: false };
const canadian = { name: 'Toronto', number: '+14165551234', extension_id: null, ambiguous: false };
const hidden = { name: '', number: 'anonymous', extension_id: null, ambiguous: false };
const service = { name: '', number: '+18005551234', extension_id: null, ambiguous: false };
const at = '2026-10-16T09:00:00Z';
const record = (id, direction, caller, called) => ({ id, call_uuid: id, extension_id: 'e-101', direction,
  caller_number: caller.number, caller_name: caller.name, destination: called.number,
  caller, called, via_number: '+4995431241', started_at: at, answered_at: at, ended_at: at,
  duration_secs: 60, billsec: 60, hangup_cause: 'NORMAL_CLEARING', recording_id: null });
const rows = [record('in', 'inbound', german, local), record('out', 'outbound', local, canadian),
  record('internal', 'internal', local, local), record('hidden', 'inbound', hidden, local),
  record('service', 'outbound', local, service)];
// Detection must use the normalized party even when the visible record is national.
rows[0].caller_number = '095436999124';
const active = [
  { uuid: 'in', direction: 'inbound', caller: german, called: local, state: 'talking', started_at: at, answered_at: at, transcript_available: false },
  { uuid: 'out', direction: 'outbound', caller: local, called: canadian, state: 'talking', started_at: at, answered_at: at, transcript_available: false }
];
const section = (page, name) => page.locator('section').filter({ has: page.getByRole('heading', { name, exact: true }) });
const skeleton = parent => parent.locator('[data-testid="skeleton"]').first();

async function scenario({ reducedMotion = 'no-preference', dark = false, locale = 'de' } = {}) {
  const context = await browser.newContext({ locale: locale === 'de' ? 'de-DE' : 'en-GB', reducedMotion, viewport: { width: 1280, height: 900 } });
  await context.addInitScript(({ dark, locale }) => {
    localStorage.setItem('talkops.locale', locale);
    localStorage.setItem('talkops.theme', dark ? 'dark' : 'light');
    localStorage.setItem('talkops.gettingStarted.hidden', '1');
    window.skeletonTiming = [];
    let wasLoading = false;
    new MutationObserver(() => {
      const loading = !!document.querySelector('main table [data-testid="skeleton"]');
      if (wasLoading !== loading) window.skeletonTiming.push({ loading, at: performance.now() });
      wasLoading = loading;
    }).observe(document, { childList: true, subtree: true });
  }, { dark, locale });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const state = { rows, active, failed: new Set(), gates: new Map(), polls: 0 };
  const block = path => {
    let release, requested;
    const promise = new Promise(resolve => { release = resolve; });
    const seen = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`No request for ${path}`)), 15_000);
      timer.unref();
      requested = () => { clearTimeout(timer); resolve(); };
    });
    state.gates.set(path, { promise, requested });
    return { release: () => { state.gates.delete(path); release(); }, seen };
  };
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url()), path = url.pathname.replace('/api/v1', '');
    const gate = state.gates.get(path);
    if (gate) { gate.requested(); await gate.promise; }
    if (state.failed.has(path)) { await route.fulfill({ status: 503, json: { error: 'unavailable', message: 'Fixture unavailable' } }); return; }
    let value = [], headers = {};
    if (path === '/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (path === '/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (path === '/telephony/status') value = { connected: true, registrations: [], gateways: {} };
    else if (path === '/stats/calls') value = { range: '1d', buckets: [], total: 0, missed: 0, avg_talk_secs: 0, answer_rate: null, inbound: 0, outbound: 0, internal: 0 };
    else if (path === '/backups') value = { settings: { last_run_at: null } };
    else if (path === '/calls') {
      value = url.searchParams.get('offset') > 0 ? [record('more', 'inbound', german, local)] : state.rows;
      headers['X-Total-Count'] = '6';
    } else if (path === '/telephony/calls') { state.polls++; value = state.active; }
    else if (path === '/me/webrtc/extensions') value = [{ extension_id: 'e-101', extension_number: '101', display_name: 'Office', outbound_number: '+4995431241', uses_default_number: true, hide_caller_id: false }];
    else if (path === '/me/webrtc') { await route.fulfill({ status: 403, json: { error: 'disabled' } }); return; }
    else if (path === '/me/webrtc/calls') value = rows.slice(0, 2).map(r => ({ ...r, caller_number: r.caller?.number, caller: undefined, called: undefined }));
    await route.fulfill({ status: 200, json: value, headers });
  });
  return { context, page, state, block, errors };
}

async function flags(parent, expected) {
  const images = parent.locator('img[data-country]');
  assert.deepEqual(await images.evaluateAll(els => els.map(el => el.dataset.country)), expected);
  for (const img of await images.all()) {
    await img.evaluate(el => el.decode());
    assert.ok(await img.evaluate(el => el.naturalWidth > 0));
    assert.equal(new URL(await img.getAttribute('src'), BASE).origin, new URL(BASE).origin, 'local assets only');
  }
}

try {
  // Immediate skeleton, minimum duration for fast replies, fade, filters and pagination.
  {
    const { context, page, block, errors } = await scenario();
    const initial = block('/calls');
    await page.goto(`${BASE}/calls`);
    const table = page.locator('main table');
    await skeleton(table).waitFor();
    assert.equal(await skeleton(table).evaluate(el => getComputedStyle(el).opacity), '1');
    initial.release();
    await table.getByText('Maria', { exact: true }).waitFor();
    const timing = await page.evaluate(() => window.skeletonTiming);
    assert.ok(timing[1].at - timing[0].at >= 280, JSON.stringify(timing));
    assert.equal(await table.locator('tbody').evaluate(el => getComputedStyle(el).animationName), 'content-in');
    assert.equal(await table.locator('tbody').evaluate(el => getComputedStyle(el).animationDuration), '0.18s');
    await flags(table, ['DE', 'CA']);
    assert.equal(await table.locator('img').first().getAttribute('alt'), 'Länderkennzeichnung: Deutschland');
    await page.getByLabel('Rufnummern über Telefonbuch anzeigen').uncheck();
    await flags(table, ['DE', 'CA']);
    assert.match(await table.locator('tbody tr').first().textContent(), /095436999124/);

    const filter = block('/calls');
    await page.getByRole('button', { name: /^Dauer/ }).click();
    await filter.seen;
    assert.equal(await table.locator('tbody tr').count(), 5, 'retain the old result during filtering');
    await page.getByRole('status').waitFor();
    assert.equal(await table.locator('[data-testid="skeleton"]').count(), 0);
    filter.release();
    await page.getByRole('status').waitFor({ state: 'detached' });
    const more = block('/calls');
    await page.getByRole('button', { name: 'Weitere laden' }).click();
    await more.seen;
    await skeleton(table).waitFor();
    assert.equal(await table.locator('tbody tr:not([data-testid="skeleton"])').count(), 5);
    more.release();
    await page.getByText('6 von 6 Anrufen').waitFor();
    assert.equal(await table.locator('[data-testid="skeleton"]').count(), 0);
    // Navigation away during a pending load must not leave a late placeholder.
    const pending = block('/calls');
    await page.getByRole('button', { name: /^Dauer/ }).click();
    await pending.seen;
    await page.getByRole('link', { name: 'Übersicht', exact: true }).click();
    pending.release();
    await page.locator('[data-testid="active-content"]').waitFor();
    assert.deepEqual(errors, []);
    await context.close();
    console.log('call presentation scenario passed');
  }
  // Independent dashboard loads, slow answers, polling without replacement, narrow themes.
  for (const dark of [false, true]) {
    const { context, page, block, state, errors } = await scenario({ dark });
    const recent = block('/calls'), live = block('/telephony/calls');
    await page.clock.install();
    await page.goto(BASE);
    const a = section(page, 'Laufende Gespräche'), r = section(page, 'Letzte Anrufe');
    await skeleton(a).waitFor(); await skeleton(r).waitFor();
    assert.doesNotMatch(await a.textContent(), /Gerade keine Gespräche/);
    await page.waitForTimeout(350);
    live.release();
    await a.getByText('Toronto', { exact: true }).waitFor();
    await skeleton(r).waitFor();
    recent.release();
    await r.getByText('Toronto', { exact: true }).waitFor();
    await flags(a, ['DE', 'CA']); await flags(r, ['DE', 'CA']);
    assert.match(await a.textContent(), /\[\+4995436999124\]/, 'keep brackets');
    const polling = block('/telephony/calls');
    // Advance the polling clock after the initial content is visible.
    await page.clock.fastForward(10_100);
    await polling.seen;
    assert.equal(await a.locator('[data-testid="skeleton"]').count(), 0);
    await flags(a, ['DE', 'CA']);
    polling.release();
    assert.ok(state.polls >= 1);
    for (const width of [320, 768, 1280]) {
      await page.setViewportSize({ width, height: 900 });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `overflow at ${width}`);
    }
    if (process.env.SCREENSHOT_DIR) {
      await mkdir(process.env.SCREENSHOT_DIR, { recursive: true });
      await page.setViewportSize({ width: 1280, height: 900 });
      await page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/call-flags-${dark ? 'dark' : 'light'}.png`, fullPage: true });
    }
    assert.deepEqual(errors, []);
    await context.close();
    console.log('call presentation scenario passed');
  }
  // Empty and error outcomes terminate loading without a false empty message.
  for (const fail of [false, true]) {
    const { context, page, state, errors } = await scenario({ reducedMotion: 'reduce' });
    state.rows = []; state.active = [];
    if (fail) { state.failed.add('/calls'); state.failed.add('/telephony/calls'); }
    await page.goto(BASE);
    const a = section(page, 'Laufende Gespräche'), r = section(page, 'Letzte Anrufe');
    await a.locator('[data-testid="active-content"]').waitFor();
    await r.locator('[data-testid="recent-content"]').waitFor();
    assert.equal(await a.locator('[data-testid="skeleton"]').count(), 0);
    assert.equal(await r.locator('[data-testid="skeleton"]').count(), 0);
    if (fail) { assert.equal(await a.getByRole('alert').count(), 1); assert.equal(await r.getByRole('alert').count(), 1); }
    else { assert.match(await a.textContent(), /Gerade keine Gespräche/); assert.match(await r.textContent(), /Noch keine Anrufe/); }
    await page.goto(`${BASE}/calls`);
    if (fail) await page.getByText('Fixture unavailable').waitFor();
    else await page.getByText('Noch keine Anrufe.', { exact: true }).waitFor();
    await skeleton(page.locator('main table')).waitFor({ state: 'detached' });
    assert.deepEqual(errors, []);
    await context.close();
    console.log('call presentation scenario passed');
  }
  // Softphone raw international records, English labels and reduced motion.
  {
    const { context, page, block, errors } = await scenario({ reducedMotion: 'reduce', locale: 'en' });
    await page.goto(`${BASE}/phone`);
    const history = section(page, 'Recent calls');
    const pending = block('/me/webrtc/calls');
    await page.getByLabel('Call identity', { exact: true }).selectOption('e-101');
    await pending.seen;
    await skeleton(history).waitFor();
    assert.equal(await skeleton(history).locator('.skeleton-bar').first().evaluate(el => getComputedStyle(el).animationIterationCount), '1');
    pending.release();
    await history.getByRole('button', { name: '+14165551234', exact: true }).waitFor();
    await flags(history, ['DE', 'CA']);
    assert.equal(await history.locator('img').last().getAttribute('title'), 'Country code: Canada');
    // Selecting a callback keeps the actual number intact.
    await history.getByRole('button', { name: '+14165551234', exact: true }).click();
    assert.equal(await page.getByLabel('Number', { exact: true }).inputValue(), '+14165551234');
    assert.deepEqual(errors, []);
    await context.close();
    console.log('call presentation scenario passed');
  }
  console.log('call presentation browser checks passed');
} finally {
  await browser.close();
}
