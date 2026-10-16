// Browser checks of the phone book (sort order, column filters, address state,
// contact link) against a production preview with API fixtures.
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
  const family = 's-family';
  const office = 's-office';
  const contact = (id, name, extra) => ({ id, name, company: '', phone_work: '', phone_mobile: '', phone_other: '', section_id: null, ...extra });
  const contacts = [
    contact('c-bert', 'Bert', { company: 'Zeta AG', phone_work: '0951 100', section_id: office }),
    contact('c-anna', 'anna', { phone_mobile: '+49 151 2000' }),
    contact('c-carl', 'Carl', { company: 'Alpha GmbH', phone_work: '0951 300', phone_mobile: '+49 151 3000', section_id: office }),
    contact('c-dora', 'Dora', { phone_other: '0951 400', section_id: family })
  ];
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url());
    let value = [];
    if (url.pathname === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (url.pathname === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (url.pathname === '/api/v1/contacts') value = contacts;
    else if (url.pathname === '/api/v1/phonebook-sections') value = [{ id: family, name: 'Familie' }, { id: office, name: 'Büro' }];
    await route.fulfill({ status: 200, json: value });
  });
  const base = process.env.BASE || 'http://127.0.0.1:5184';
  const names = () => page.locator('tbody tr td:first-child').allInnerTexts().then(t => t.map(x => x.split('·')[0].trim()));
  await page.goto(`${base}/phonebook`);
  await page.getByText('4 von 4 Kontakten').waitFor({ timeout: 20_000 });
  assert.deepEqual(await names(), ['anna', 'Bert', 'Carl', 'Dora'], 'name, ascending by default');
  assert.equal(await page.locator('th[aria-sort="ascending"]').count(), 1);

  // Name descending, then company (contacts without a company last).
  await page.getByRole('button', { name: 'Name', exact: true }).click();
  await page.waitForFunction(() => location.search.includes('order=desc'));
  assert.deepEqual(await names(), ['Dora', 'Carl', 'Bert', 'anna']);
  assert.equal(await page.locator('th[aria-sort="descending"]').count(), 1);
  await page.getByRole('button', { name: 'Firma', exact: true }).click();
  await page.waitForFunction(() => location.search === '?sort=company');
  assert.deepEqual(await names(), ['Carl', 'Bert', 'anna', 'Dora']);
  await page.getByRole('button', { name: 'Bereich', exact: true }).click();
  assert.deepEqual(await names(), ['anna', 'Bert', 'Carl', 'Dora'], 'global, then sections by name');

  // Column filters: section "Büro" and "with mobile number".
  await page.getByRole('button', { name: 'Bereich filtern', exact: true }).click();
  const panel = page.getByRole('dialog', { name: 'Filter: Bereich' });
  await panel.getByLabel('Büro').check();
  await page.keyboard.press('Escape');
  await panel.waitFor({ state: 'detached' });
  assert.deepEqual(await names(), ['Bert', 'Carl']);
  await page.getByRole('button', { name: 'Mobil filtern', exact: true }).click();
  await page.getByRole('dialog', { name: 'Filter: Mobil' }).getByLabel('Mit Nummer').check();
  await page.keyboard.press('Escape');
  assert.deepEqual(await names(), ['Carl']);
  await page.waitForFunction(() => location.search.includes('mobile=with'));
  assert.ok(new URL(page.url()).searchParams.get('section') === office);

  // The address restores the state after a reload; free text also searches numbers.
  await page.reload();
  await page.getByText('1 von 4 Kontakten').waitFor({ timeout: 20_000 });
  assert.equal(await page.getByRole('button', { name: 'Bereich filtern (aktiv)' }).count(), 1);
  await page.getByRole('button', { name: 'Alle Filter zurücksetzen' }).first().click();
  await page.waitForFunction(() => !location.search.includes('mobile='));
  assert.ok(new URL(page.url()).search.includes('sort=section'), 'the sort order stays');
  await page.getByRole('searchbox', { name: 'Suchen' }).fill('0951400');
  assert.deepEqual(await names(), ['Dora']);
  await page.getByRole('searchbox', { name: 'Suchen' }).fill('xyz');
  await page.getByText('Keine Kontakte passen zu den Filtern.').waitFor();

  // A contact link shows the entry: filters that hide it are replaced by its section.
  await page.goto(`${base}/phonebook?section=${family}&contact=c-bert`);
  const target = page.locator('tr[aria-current="true"]');
  await target.waitFor({ timeout: 20_000 });
  assert.match(await target.textContent(), /Bert/);
  await page.waitForFunction(id => new URLSearchParams(location.search).get('section') === id, office);
  assert.equal(new URL(page.url()).searchParams.get('contact'), 'c-bert');

  // Narrow screen: no sideways page scroll.
  await page.setViewportSize({ width: 360, height: 780 });
  const width = await page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(width[0] <= width[1], JSON.stringify(width));
  assert.deepEqual(errors, []);
  console.log('phone book browser checks passed');
} finally {
  await browser.close();
}
