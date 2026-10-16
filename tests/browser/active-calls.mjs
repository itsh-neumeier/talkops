// Browser regression checks against a production preview with API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
try {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const local = { name: 'Familie Zweig', number: '+4995431241', extension_id: 'local', ambiguous: false };
  const remote = { name: 'Familie Vollmeier (Maria)', number: '+4995458589', extension_id: null, contact_id: 'c-maria', ambiguous: false };
  const zweig = 's-zweig', neumeier = 's-neumeier';
  const shared = { name: 'Maxi', number: '+4995436999124', extension_id: null, contact_id: 'c-maxi', ambiguous: true, matches: [
    { contact_id: 'c-anna', name: 'Anna', section_id: null, section_name: '' },
    { contact_id: 'c-max', name: 'Max Mustermann (Mustermann GmbH)', section_id: neumeier, section_name: 'Familie Neumeier' },
    { contact_id: 'c-maxi', name: 'Maxi', section_id: zweig, section_name: 'Familie Zweig' }
  ] };
  const contacts = [
    { id: 'c-anna', name: 'Anna', company: '', phone_work: '+4995436999124', phone_mobile: '', phone_other: '', section_id: null },
    { id: 'c-maria', name: 'Familie Vollmeier', company: 'Maria', phone_work: '+4995458589', phone_mobile: '', phone_other: '', section_id: null },
    { id: 'c-max', name: 'Max Mustermann', company: 'Mustermann GmbH', phone_work: '+4995436999124', phone_mobile: '', phone_other: '', section_id: neumeier },
    { id: 'c-maxi', name: 'Maxi', company: '', phone_work: '+4995436999124', phone_mobile: '', phone_other: '', section_id: zweig }
  ];
  const started = new Date(Date.now() - 180_000).toISOString();
  let calls = [
    { uuid: 'outgoing', direction: 'outbound', caller: local, called: remote, started_at: started, answered_at: started, state: 'talking', transcript_available: true },
    { uuid: 'incoming', direction: 'inbound', caller: remote, called: local, started_at: started, answered_at: started, state: 'talking', transcript_available: true }
  ];
  let polls = 0, closes = 0, failing = false, extra = false;
  const newText = 'Dieser neue Gesprächsbeitrag erscheint schrittweise und bleibt nach der nächsten Aktualisierung vollständig sichtbar.';
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    let value = [];
    if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (path === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (path === '/api/v1/telephony/status') value = { connected: true, registrations: [], gateways: {} };
    else if (path === '/api/v1/stats/calls') value = { range: '1d', buckets: [], total: 0, missed: 0, avg_talk_secs: 0, answer_rate: null, inbound: 0, outbound: 0, internal: 0 };
    else if (path === '/api/v1/backups') value = { settings: { last_run_at: null } };
    else if (path === '/api/v1/telephony/calls') value = calls;
    else if (path === '/api/v1/contacts') value = contacts;
    else if (path === '/api/v1/phonebook-sections') value = [{ id: neumeier, name: 'Familie Neumeier' }, { id: zweig, name: 'Familie Zweig' }];
    else if (path.endsWith('/transcript')) {
      if (route.request().method() === 'DELETE') { closes++; await route.fulfill({ status: 204 }); return; }
      polls++;
      assert.equal(route.request().headers()['x-csrf-token'], 'fixture');
      if (failing) { await route.fulfill({ status: 503, json: { error: 'unavailable' } }); return; }
      value = { id: 'preview', started_at: started, processed_secs: 8, status: 'running', segments: polls < 2 ? [] : [
        { start: 1, end: 2, speaker: 'called', text: 'Hallo, hier ist Maria.' },
        { start: 3, end: 4, speaker: 'caller', text: 'Passt es morgen um zehn Uhr?' },
        ...(extra ? [{ start: 5, end: 6, speaker: 'called', text: newText }] : [])
      ] };
    }
    await route.fulfill({ status: 200, json: value });
  });
  await page.goto(process.env.BASE || 'http://127.0.0.1:5184');
  const section = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Laufende Gespräche', exact: true }) });
  await section.getByText('Ausgehender Anruf von Nst:', { exact: false }).waitFor({ timeout: 20_000 });
  const rows = section.locator('li');
  assert.match(await rows.nth(0).textContent(), /Familie Zweig\s*\[\+4995431241\]/);
  assert.match(await rows.nth(0).textContent(), /Familie Vollmeier \(Maria\)\s*\[\+4995458589\]/);
  assert.match(await rows.nth(1).textContent(), /Eingehender Anruf an Nst:/);
  await section.getByRole('button', { name: 'Live-Transkript', exact: true }).first().click();
  const dialog = page.getByRole('dialog');
  await dialog.waitFor(); await dialog.getByText('Hallo, hier ist Maria.', { exact: true }).first().waitFor({ timeout: 8_000 });
  assert.match(await dialog.textContent(), /Angerufene: Familie Vollmeier \(Maria\)/);
  assert.match(await dialog.textContent(), /Anrufer: Familie Zweig/);
  assert.match(await dialog.locator('time').first().textContent(), /^\d{2}:\d{2}:\d{2}$/);
  assert.equal(await dialog.locator('.sr-only').first().textContent(), 'Hallo, hier ist Maria.');
  await page.keyboard.press('Escape'); await dialog.waitFor({ state: 'detached' });
  const afterClose = polls; await page.waitForTimeout(3300); assert.equal(polls, afterClose); assert.ok(closes > 0);
  await page.setViewportSize({ width: 320, height: 780 });
  await section.getByRole('button', { name: 'Live-Transkript', exact: true }).last().click();
  await dialog.waitFor();
  await dialog.getByText('Angerufene: Familie Zweig', { exact: true }).waitFor();
  const geometry = await page.evaluate(() => ({ width: innerWidth, scroll: document.documentElement.scrollWidth,
    overflow: [...document.querySelectorAll('body *')].filter(el => el.getBoundingClientRect().right > innerWidth + 1).slice(0, 8).map(el => ({ tag: el.tagName, text: el.textContent?.slice(0, 60), right: el.getBoundingClientRect().right })) }));
  assert.ok(geometry.scroll <= geometry.width, JSON.stringify(geometry));
  assert.match(await dialog.textContent(), /Angerufene: Familie Zweig/);
  if (process.env.SCREENSHOT_DIR) { await mkdir(process.env.SCREENSHOT_DIR, { recursive: true }); await page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/active-calls-mobile.png`, fullPage: true }); }
  await dialog.getByRole('button', { name: 'Schließen', exact: true }).click();
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await section.getByRole('button', { name: 'Live-Transkript', exact: true }).first().click();
  await dialog.getByText('Hallo, hier ist Maria.', { exact: true }).first().waitFor();
  extra = true;
  await dialog.getByText(newText, { exact: true }).waitFor({ timeout: 8_000 });
  const animated = dialog.locator('li').last().locator('p > span[aria-hidden="true"]');
  assert.ok((await animated.textContent()).length < newText.length);
  await page.waitForTimeout(1600); assert.equal(await animated.textContent(), newText);
  await page.waitForTimeout(3000); assert.equal(await animated.textContent(), newText);
  await dialog.getByRole('button', { name: 'Schließen', exact: true }).click();
  failing = true;
  await section.getByRole('button', { name: 'Live-Transkript', exact: true }).first().click();
  await dialog.getByRole('alert').waitFor();
  await dialog.getByRole('button', { name: 'Schließen', exact: true }).click();
  assert.deepEqual(errors, []);
  failing = false;
  await section.getByRole('button', { name: 'Live-Transkript', exact: true }).first().click();
  await dialog.getByText('Hallo, hier ist Maria.', { exact: true }).first().waitFor();
  calls = calls.map(call => call.uuid === 'outgoing' ? { ...call, called: { ...remote, number: '+4995459999' } } : call);
  await dialog.getByText('Das Gespräch oder die Live-Vorschau ist beendet.', { exact: true }).waitFor({ timeout: 15_000 });
  const afterTransfer = polls; await page.waitForTimeout(3300); assert.equal(polls, afterTransfer);
  await dialog.getByRole('button', { name: 'Schließen', exact: true }).click();

  // Shared number: the own phone book name, a hover card with all entries, and jumps.
  calls = [{ uuid: 'shared', direction: 'inbound', caller: shared, called: local, started_at: started, answered_at: started, state: 'talking', transcript_available: false }];
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.reload();
  await section.getByText('Eingehender Anruf an Nst:', { exact: false }).waitFor({ timeout: 20_000 });
  const sharedRow = section.locator('li').first();
  assert.match(await sharedRow.textContent(), /von\s+Maxi\s*\[\+4995436999124\]\s*\+2 weitere/);
  assert.doesNotMatch(await sharedRow.textContent(), /Mehrere Telefonbucheinträge/);
  const more = sharedRow.getByRole('button', { name: 'Alle 3 Telefonbucheinträge zu dieser Nummer anzeigen' });
  assert.equal(await more.getAttribute('aria-expanded'), 'false');
  await more.hover();
  const card = page.getByRole('region', { name: 'Telefonbucheinträge für +4995436999124' });
  await card.waitFor();
  assert.deepEqual(await card.locator('thead th').allTextContents().then(h => h.map(x => x.trim())), ['Telefonbuch', 'Eintrag (Name)', 'Gehe zu Eintrag']);
  const cells = await card.locator('tbody tr').evaluateAll(rows => rows.map(r => [...r.cells].slice(0, 2).map(c => c.innerText.replace(/\s+/g, ' ').trim())));
  assert.deepEqual(cells, [['Global (alle Telefone)', 'Anna'], ['Familie Neumeier', 'Max Mustermann (Mustermann GmbH)'], ['Familie Zweig', 'Maxi angezeigt']]);
  const links = await card.getByRole('link', { name: /Gehe zu Eintrag/ }).evaluateAll(a => a.map(l => l.getAttribute('href')));
  assert.deepEqual(links, ['/phonebook?contact=c-anna', '/phonebook?contact=c-max', '/phonebook?contact=c-maxi']);
  // The pointer can move into the card; leaving both closes it.
  await card.hover(); await page.waitForTimeout(400); assert.ok(await card.isVisible());
  await page.mouse.move(5, 5); await card.waitFor({ state: 'detached' });
  // Keyboard: Enter opens, Escape closes and returns the focus.
  await more.focus(); await page.keyboard.press('Enter'); await card.waitFor();
  assert.equal(await more.getAttribute('aria-expanded'), 'true');
  await page.keyboard.press('Escape'); await card.waitFor({ state: 'detached' });
  assert.equal(await page.evaluate(() => document.activeElement?.textContent?.trim()), '+2 weitere');
  // Narrow screen: a tap opens the card inside the viewport.
  await page.setViewportSize({ width: 320, height: 780 });
  await more.click(); await card.waitFor();
  const box = await card.boundingBox();
  assert.ok(box.x >= 0 && box.x + box.width <= 320 + 1, JSON.stringify(box));
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/active-calls-shared-mobile.png` });
  await page.setViewportSize({ width: 1280, height: 900 });
  await card.getByRole('link', { name: /Gehe zu Eintrag/ }).nth(1).click();
  await page.waitForURL(url => url.pathname === '/phonebook' && url.searchParams.get('contact') === 'c-max');
  const target = page.locator('tr[aria-current="true"]');
  await target.waitFor();
  assert.match(await target.textContent(), /Max Mustermann/);
  // The entry is visible without filters, so the phone book filters stay unchanged.
  assert.ok(await target.isVisible());
  assert.equal(new URL(page.url()).searchParams.get('contact'), 'c-max');
  assert.equal(await page.getByRole('button', { name: 'Bereich filtern (aktiv)' }).count(), 0);
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/phonebook-highlight.png` });
  // A single entry: the name itself jumps to the entry, without a card.
  calls = [{ uuid: 'single', direction: 'inbound', caller: remote, called: local, started_at: started, answered_at: started, state: 'talking', transcript_available: false }];
  await page.goto(process.env.BASE || 'http://127.0.0.1:5184');
  await section.getByText('Eingehender Anruf an Nst:', { exact: false }).waitFor({ timeout: 20_000 });
  assert.equal(await section.getByRole('button', { name: /Telefonbucheinträge/ }).count(), 0);
  await section.getByRole('link', { name: 'Familie Vollmeier (Maria)', exact: true }).click();
  await page.waitForURL(url => url.pathname === '/phonebook' && url.searchParams.get('contact') === 'c-maria');
  assert.match(await page.locator('tr[aria-current="true"]').textContent(), /Familie Vollmeier/);
  assert.deepEqual(errors, []);
  await context.close();
  console.log('PASS: call brackets, shared numbers, hover card, phone book jumps, directions, speakers, times, popup, reduced motion, cleanup, mobile layout and errors');
} finally { await browser.close(); }
