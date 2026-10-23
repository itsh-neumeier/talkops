// Browser checks of the call origin map (ADR 0044) against a production
// preview with API fixtures: view and range in the address, bubbles with
// keyboard access, details panel, link to the list, and no request to
// another host without a tile URL.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const base = process.env.BASE || 'http://127.0.0.1:5184';
try {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const foreign = [];
  page.on('request', request => {
    const url = new URL(request.url());
    if (url.origin !== new URL(base).origin && !url.protocol.startsWith('data') && !url.protocol.startsWith('blob')) foreign.push(request.url());
  });
  const at = '2026-10-23T09:02:00Z';
  const zweig = { name: 'Familie Zweig', number: '101', extension_id: 'e-101', ambiguous: false };
  const baker = { name: 'Bäckerei Bamberg', number: '+4995112345', extension_id: null, contact_id: 'c-1', ambiguous: false };
  const call = (id, extra) => ({ id, call_uuid: id, direction: 'inbound', caller_number: '+4995112345', caller_name: '', destination: '101', extension_id: 'e-101', started_at: at, answered_at: at, ended_at: at, duration_secs: 75, billsec: 75, hangup_cause: 'NORMAL_CLEARING', blocked_reason: null, recording_id: null, caller: baker, called: zweig, via_number: null, ...extra });
  const counts = (answered, missed, outbound, blocked) => ({ answered, missed, outbound, blocked });
  const map = range => ({
    range, since: at, until: at, truncated: false, limit: 50000,
    counts: counts(2, 1, 0, 1), total: 5, anonymous: counts(0, 0, 0, 1),
    areas: [
      { id: 'DE-951', kind: 'area', country: 'DE', name: 'Bamberg', prefix: '+49 951', lat: 49.89, lon: 10.89, counts: counts(2, 1, 0, 0), total: 3, billsec: 150, mobile: 0, service: 0, unresolved: 0,
        calls: [call('a'), call('b'), call('c', { answered_at: null, billsec: 0, hangup_cause: 'NO_ANSWER' })] },
      { id: 'DE-none', kind: 'none', country: 'DE', name: 'Germany', prefix: '+49', lat: 50.96, lon: 9.68, counts: counts(0, 0, 0, 1), total: 1, billsec: 0, mobile: 1, service: 0, unresolved: 0,
        calls: [call('m', { caller_number: '+491711234567', caller: { name: '', number: '+491711234567', extension_id: null, ambiguous: false }, answered_at: null, billsec: 0, blocked_reason: 'phoneblock:3' })] }
    ],
    sources: [{ file: 'de.tsv', title: 'Germany', source: 'BNetzA', license: '© Bundesnetzagentur', updated: '2026-10-23' }]
  });
  const mapRequests = [];
  const listRequests = [];
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url());
    let value = [];
    const headers = {};
    if (url.pathname === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (url.pathname === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (url.pathname === '/api/v1/calls/map') {
      mapRequests.push(url.searchParams);
      value = map(url.searchParams.get('range'));
    } else if (url.pathname === '/api/v1/calls') {
      listRequests.push(url.searchParams);
      value = [call('a')];
      headers['X-Total-Count'] = '1';
    }
    await route.fulfill({ status: 200, json: value, headers });
  });

  // The address selects the map view and the range.
  await page.goto(`${base}/calls?view=map&range=1w`);
  const bubbles = page.locator('.call-bubble');
  await bubbles.nth(1).waitFor({ timeout: 20_000 });
  assert.equal(await bubbles.count(), 2);
  assert.equal(mapRequests.at(-1).get('range'), '1w');
  assert.equal(listRequests.length, 0, 'the list does not load in the map view');
  assert.equal(await page.getByRole('tab', { name: '1 Woche' }).getAttribute('aria-selected'), 'true');
  assert.match(await page.locator('.leaflet-control-attribution').textContent(), /Bundesnetzagentur.*RTR-GmbH.*GeoNames.*libphonenumber.*Natural Earth/);
  assert.ok(await page.locator('.call-map-country').count() > 100, 'offline base map');

  // Keyboard: focus the Bamberg bubble, Enter opens the details, Esc closes them.
  const bamberg = page.getByRole('button', { name: /Bamberg \+49 951: 3 Anrufe/ });
  await bamberg.focus();
  await page.keyboard.press('Enter');
  const details = page.locator('section[aria-labelledby="call-map-popup-title"]');
  await details.waitFor();
  assert.match(await details.locator('h2').textContent(), /Bamberg\s*· \+49 951/);
  assert.match(await details.textContent(), /Deutschland/);
  assert.match(await details.textContent(), /Bäckerei Bamberg/);
  assert.match(await details.textContent(), /Gesamtdauer\s*2:30/);
  const link = details.getByRole('link', { name: 'Alle in der Anrufliste anzeigen' });
  const href = new URL(await link.getAttribute('href'), base);
  assert.equal(href.searchParams.get('range'), '1w');
  assert.equal(href.searchParams.get('area'), 'DE-951');
  await page.keyboard.press('Escape');
  await details.waitFor({ state: 'detached' });

  // Without location: the mobile bubble of Germany and the anonymous counter.
  const unlocated = page.locator('section[aria-labelledby="call-map-unlocated"]');
  assert.match(await unlocated.textContent(), /Deutschland.*Mobil: 1/s);
  assert.match(await unlocated.textContent(), /Anonym \(ohne Nummer\)\s*1/);

  // Category chips and range tabs update the address and the request.
  await page.getByRole('button', { name: /^Ausgehend/ }).click();
  await page.waitForFunction(() => location.search.includes('categories='));
  assert.equal(mapRequests.at(-1).get('categories'), 'answered,missed,blocked');
  await page.getByRole('tab', { name: '1 Stunde' }).click();
  await page.waitForFunction(() => location.search.includes('range=1h'));
  assert.equal(mapRequests.at(-1).get('range'), '1h');
  await page.reload();
  await bubbles.first().waitFor({ timeout: 20_000 });
  assert.equal(mapRequests.at(-1).get('range'), '1h', 'reload keeps the range');
  assert.equal(await page.getByRole('button', { name: /^Ausgehend/ }).getAttribute('aria-pressed'), 'false');

  // The details link opens the list with the area filter.
  await page.locator('.call-bubble').filter({ hasNot: page.locator('.call-bubble-none') }).first().click();
  await page.getByRole('link', { name: 'Alle in der Anrufliste anzeigen' }).click();
  await page.waitForFunction(() => location.search.includes('area=DE-951'));
  await page.getByText('Ortsnetz: Bamberg · +49 951').waitFor({ timeout: 20_000 });
  assert.equal(listRequests.at(-1).get('area'), 'DE-951');
  assert.ok(listRequests.at(-1).get('since'), 'rolling range as start time');

  // Narrow screen: no sideways scrolling in the map view.
  await page.setViewportSize({ width: 360, height: 800 });
  await page.goto(`${base}/calls?view=map`);
  await bubbles.first().waitFor({ timeout: 20_000 });
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1), 'no sideways scrolling');

  assert.deepEqual(foreign, [], 'no request to another host without a tile URL');
  assert.deepEqual(errors, []);
  console.log('call map browser checks passed');
} finally {
  await browser.close();
}
