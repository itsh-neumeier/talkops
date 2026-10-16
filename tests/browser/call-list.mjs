// Browser checks of the call list (filters, sort order, phone book names)
// against a production preview with API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
try {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const at = '2026-10-15T09:02:00Z';
  const zweig = { name: 'Familie Zweig', number: '101', extension_id: 'e-101', ambiguous: false };
  const maria = { name: 'Familie Vollmeier (Maria)', number: '+4995436999124', extension_id: null, contact_id: 'c-maria', ambiguous: false };
  const unknown = { name: '', number: '+499112193553', extension_id: null, ambiguous: false };
  const record = (id, extra) => ({ id, call_uuid: id, extension_id: 'e-101', started_at: at, answered_at: at, ended_at: at, duration_secs: 59, billsec: 59, hangup_cause: 'NORMAL_CLEARING', recording_id: null, ...extra });
  const rows = [
    record('in', { direction: 'inbound', caller_number: '+4995436999124', caller_name: '095436999124', destination: '101', caller: maria, called: zweig, via_number: '+4995431241' }),
    record('busy', { direction: 'inbound', caller_number: '+499112193553', caller_name: '', destination: '101', answered_at: null, billsec: 0, hangup_cause: 'USER_BUSY', caller: unknown, called: { name: '', number: '101', extension_id: null, ambiguous: false }, via_number: null }),
    record('out', { direction: 'outbound', caller_number: '101', caller_name: 'Familie Zweig', destination: '+4995436999124', caller: zweig, called: maria, via_number: null })
  ];
  const requests = [];
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url());
    let value = [];
    const headers = {};
    if (url.pathname === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (url.pathname === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (url.pathname === '/api/v1/calls') {
      requests.push(url.searchParams);
      value = url.searchParams.get('result') === 'busy' ? rows.filter(r => r.id === 'busy') : rows;
      headers['X-Total-Count'] = String(url.searchParams.get('result') === 'busy' ? 1 : 120);
    }
    await route.fulfill({ status: 200, json: value, headers });
  });
  await page.goto(`${process.env.BASE || 'http://127.0.0.1:5184'}/calls`);
  const table = page.locator('table');
  await table.getByText('Familie Vollmeier (Maria)').first().waitFor({ timeout: 20_000 });
  const first = table.locator('tbody tr').first();
  // Inbound: phone book name over the number; the answering extension via the dialed number.
  assert.match(await first.textContent(), /Familie Vollmeier \(Maria\)\s*\+4995436999124/);
  assert.match(await first.locator('td').nth(3).textContent(), /Nst\.\s*101\s*über\s*\+4995431241\s*Familie Zweig/);
  assert.match(await table.locator('tbody tr').nth(1).textContent(), /besetzt\/abgelehnt/);
  assert.match(await page.locator('main').textContent(), /3 von 120 Anrufen/);

  // Phone book names off: the number comes first, the name from the record below it.
  const toggle = page.getByLabel('Rufnummern über Telefonbuch anzeigen');
  assert.equal(await toggle.isChecked(), true, 'on by default');
  await toggle.uncheck();
  assert.match(await first.locator('td').nth(2).textContent(), /^\s*\+4995436999124\s*095436999124\s*$/);
  assert.equal(await page.evaluate(() => localStorage.getItem('talkops.calls.phonebook')), 'off');

  // Sort by duration, then ascending.
  await page.getByRole('button', { name: /^Dauer/ }).click();
  await page.waitForFunction(() => location.search.includes('sort=duration'));
  assert.equal(requests.at(-1).get('sort'), 'duration');
  assert.equal(requests.at(-1).get('order'), 'desc');
  await page.getByRole('button', { name: /^Dauer/ }).click();
  await page.waitForFunction(() => location.search.includes('order=asc'));
  assert.equal(await page.locator('th[aria-sort="ascending"]').count(), 1);

  // Column filter popup: result "busy".
  await page.getByRole('button', { name: 'Ergebnis filtern', exact: true }).click();
  const panel = page.getByRole('dialog', { name: 'Filter: Ergebnis' });
  await panel.waitFor();
  await panel.getByLabel('besetzt/abgelehnt').check();
  await page.waitForFunction(() => location.search.includes('result=busy'));
  await page.getByText('1 von 1 Anrufen').waitFor();
  assert.equal(await page.getByRole('button', { name: 'Ergebnis filtern (aktiv)' }).count(), 1);
  await page.keyboard.press('Escape');
  await panel.waitFor({ state: 'detached' });

  // The address restores the filters after a reload; "reset all" clears them.
  await page.reload();
  await page.getByText('1 von 1 Anrufen').waitFor({ timeout: 20_000 });
  assert.equal(requests.at(-1).get('result'), 'busy');
  await page.getByRole('button', { name: 'Alle Filter zurücksetzen' }).click();
  await page.waitForFunction(() => !location.search.includes('result='));
  assert.ok(new URL(page.url()).search.includes('sort=duration'), 'the sort order stays');

  // Narrow screen: no sideways page scroll.
  await page.setViewportSize({ width: 360, height: 780 });
  const width = await page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(width[0] <= width[1], JSON.stringify(width));
  assert.deepEqual(errors, []);
  console.log('call list browser checks passed');
} finally {
  await browser.close();
}
