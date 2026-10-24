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
  const mapFiles = [];
  page.on('request', request => {
    const url = new URL(request.url());
    if (url.pathname.startsWith('/map/')) mapFiles.push(url.pathname);
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
    else if (url.pathname === '/api/v1/stats/calls') value = { range: '1d', buckets: [], total: 0, missed: 0, avg_talk_secs: 0, answer_rate: null, inbound: 0, outbound: 0, internal: 0 };
    else if (url.pathname === '/api/v1/telephony/status') value = { connected: true, registrations: [], gateways: {}, updated_at: at };
    else if (url.pathname === '/api/v1/backups') value = { settings: { last_run_at: null } };
    else if (url.pathname === '/api/v1/setup') value = { needs_setup: false };
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

  // Detail layers: state borders, water, places and local network areas load
  // by zoom level from the own server; the selected area gets an outline.
  const zoom = () => page.evaluate(() => Number(document.querySelector('.call-map').dataset.zoom));
  const zoomTo = async target => {
    while (await zoom() < target) {
      const before = await zoom();
      await page.locator('.leaflet-control-zoom-in').click();
      await page.waitForFunction(z => Number(document.querySelector('.call-map').dataset.zoom) > z, before);
    }
  };
  await page.getByRole('button', { name: /Bamberg \+49 951/ }).click();
  await page.locator('.leaflet-cm-onb-pane path').first().waitFor({ timeout: 20_000 });
  assert.ok(mapFiles.includes('/map/onb.geojson'), 'outline of the selected area');
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => !document.querySelector('.leaflet-cm-onb-pane path'));
  await zoomTo(5);
  await page.locator('.call-map-place > span').first().waitFor({ timeout: 20_000 });
  for (const file of ['/map/admin1.geojson', '/map/rivers.geojson', '/map/lakes.geojson', '/map/places-5.json']) {
    assert.ok(mapFiles.includes(file), `${file} at zoom ${await zoom()}`);
  }
  assert.ok(!mapFiles.includes('/map/places-10.json'), 'small places wait for zoom 10');
  await zoomTo(10);
  await page.waitForFunction(() => document.querySelectorAll('.call-map-place').length > 5);
  assert.ok(mapFiles.includes('/map/places-10.json'));
  assert.equal(await zoom(), 10);
  await zoomTo(11);
  assert.equal(await page.locator('.leaflet-control-zoom-in').getAttribute('aria-disabled'), 'true', 'offline maximum zoom 11');
  // Labels never overlap.
  const boxes = await page.locator('.call-map-place > span').evaluateAll(els => els.map(e => e.getBoundingClientRect().toJSON()));
  for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) {
    const a = boxes[i], b = boxes[j];
    assert.ok(a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top, 'labels overlap');
  }
  await page.reload();
  await bubbles.first().waitFor({ timeout: 20_000 });

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

  // Dashboard widget: compact map, top areas, link to the map view.
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto(`${base}/`);
  const widget = page.locator('section[aria-labelledby="dash-call-map"]');
  await widget.locator('.call-bubble').first().waitFor({ timeout: 20_000 });
  assert.match(await widget.textContent(), /Top-Ortsnetze.*Bamberg\s*\+49 951\s*3/s);
  // The widget zooms to the bubbles and shows the same detail layers.
  await widget.locator('.call-map-place > span').first().waitFor({ timeout: 20_000 });
  assert.ok(await widget.evaluate(el => Number(el.querySelector('.call-map').dataset.zoom)) >= 5, 'widget zooms in');
  assert.ok(await widget.locator('.leaflet-cm-admin-pane canvas').count() > 0, 'state borders in the widget');
  assert.equal(await widget.locator('.leaflet-control-zoom-in').count(), 1, 'zoom buttons in the widget');
  assert.equal(mapRequests.at(-1).get('range'), '1d', 'widget default 24 h');
  const before = mapRequests.length;
  await widget.getByRole('tab', { name: '1 Stunde' }).click();
  await page.waitForFunction(() => document.querySelector('section[aria-labelledby="dash-call-map"] a[href$="range=1h"]'));
  while (mapRequests.length === before) await page.waitForTimeout(50);
  assert.equal(mapRequests.at(-1).get('range'), '1h');
  assert.equal(await widget.getByRole('link', { name: 'Karte öffnen' }).getAttribute('href'), '/calls?view=map&range=1h');
  assert.deepEqual(foreign, [], 'the widget makes no external request either');

  // With a tile URL the tile layer loads from the tile server. The server
  // adds the tile host to the page policy; the preview does not, so this
  // check adds it to the page the same way.
  const tileRequests = [];
  const tiles = { tile_url: 'https://{s}.tiles.example.test/{z}/{x}/{y}.png', attribution: '© Example tiles' };
  await page.route('**/api/v1/settings/call-map', route => route.fulfill({ status: 200, json: tiles }));
  await page.route('https://*.tiles.example.test/**', route => {
    tileRequests.push(route.request().url());
    route.fulfill({ status: 200, contentType: 'image/png', body: Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64') });
  });
  await page.route(`${base}/calls*`, async route => {
    if (route.request().resourceType() !== 'document') return route.fallback();
    const res = await route.fetch();
    const allow = text => text.replace("img-src 'self'", "img-src 'self' https://*.tiles.example.test");
    // The preview sends the policy as a header, the TalkOps server as a meta tag.
    const headers = { ...res.headers() };
    if (headers['content-security-policy']) headers['content-security-policy'] = allow(headers['content-security-policy']);
    await route.fulfill({ response: res, headers, body: allow(await res.text()) });
  });
  await page.goto(`${base}/calls?view=map`);
  await bubbles.first().waitFor({ timeout: 20_000 });
  await page.locator('.leaflet-tile-loaded').first().waitFor({ timeout: 20_000 });
  assert.ok(tileRequests.length > 0, 'tiles requested');
  assert.ok(tileRequests.every(u => /^https:\/\/[abc]\.tiles\.example\.test\/\d+\/\d+\/\d+\.png$/.test(u)), tileRequests[0]);
  assert.equal(await page.locator('.call-map-country').count(), 0, 'no outline layer with tiles');
  assert.equal(await page.locator('.leaflet-cm-water-pane canvas').count(), 0, 'no offline water with tiles');
  assert.match(await page.locator('.leaflet-control-attribution').textContent(), /© Example tiles/);
  assert.deepEqual(errors, []);
  console.log('call map browser checks passed');
} finally {
  await browser.close();
}
