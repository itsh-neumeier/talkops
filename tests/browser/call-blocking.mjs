// Call decisions, whitelist actions and the allowlist-only role against API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const at = '2026-10-19T08:59:00Z';
const phone = '+493012345600';
const own = '+493012345699';
const record = (id, reason, number = phone) => ({
  id, call_uuid: id, direction: 'inbound', caller_number: number, caller_name: '', destination: own,
  extension_id: null, started_at: at, answered_at: null, ended_at: at, billsec: 0, duration_secs: 0,
  hangup_cause: 'CALL_REJECTED', blocked_reason: reason, recording_id: null, via_number: own,
  caller: { number, name: '', extension_id: null, ambiguous: false },
  called: { number: own, name: '', extension_id: null, ambiguous: false }
});

async function open({ permissions = ['blocking.allowlist', 'calls.view_all'], locale = 'de', theme = 'dark', width = 1280 } = {}) {
  const context = await browser.newContext({ reducedMotion: 'reduce', viewport: { width, height: 1000 } });
  await context.addInitScript(({ locale, theme }) => {
    localStorage.setItem('talkops.locale', locale);
    localStorage.setItem('talkops.theme', theme);
    localStorage.setItem('talkops.gettingStarted.hidden', '1');
  }, { locale, theme });
  const page = await context.newPage();
  const errors = [], requests = [], posts = [];
  let failPost = false;
  let entries = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/api/**', async route => {
    const request = route.request(), url = new URL(request.url()), method = request.method();
    requests.push([method, url.pathname]);
    let value = [], status = 200, headers = {};
    if (url.pathname === '/api/v1/auth/me') value = { user: { id: 'u1', username: 'test', display_name: 'Test', role: 'user', enabled: true }, permissions, csrf_token: 'fixture' };
    else if (url.pathname === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (url.pathname === '/api/v1/calls') {
      value = [record('local', 'list:+4930*'), record('community', 'phoneblock:12'), record('hidden', 'anonymous', 'anonymous'), record('legacy', null)]
        .map(c => ({ ...c, allowlisted: permissions.includes('blocking.allowlist') && entries.some(e => e.number === c.caller_number) }));
      if (url.searchParams.get('result') === 'blocked') value = value.filter(c => c.blocked_reason);
      headers['X-Total-Count'] = String(value.length);
    } else if (url.pathname === '/api/v1/call-allowlist' && method === 'POST') {
      const input = request.postDataJSON(); posts.push(input);
      if (failPost) { status = 403; value = { error: 'forbidden', message: 'Permission revoked' }; }
      else { const entry = { id: 'a1', ...input, created_at: at }; entries = [entry]; value = entry; status = 201; }
    } else if (url.pathname === '/api/v1/call-allowlist') value = entries;
    else if (url.pathname.startsWith('/api/v1/call-allowlist/') && method === 'DELETE') { entries = []; status = 204; }
    if (status === 204) await route.fulfill({ status });
    else await route.fulfill({ status, json: value, headers });
  });
  return { context, page, errors, requests, posts, setFail: value => { failPost = value; } };
}

try {
  const f = await open(); const { page } = f;
  await page.goto(`${BASE}/calls`);
  const blocks = page.getByTestId('blocked-call');
  await blocks.first().waitFor();
  assert.equal(await blocks.count(), 3);
  assert.match(await blocks.nth(0).textContent(), /Blockiert · Sperrliste/);
  assert.match(await blocks.nth(1).textContent(), /Blockiert · PhoneBlock/);
  assert.match(await blocks.nth(2).textContent(), /Blockiert · Anonym/);
  assert.equal(await blocks.nth(2).getByRole('button').count(), 0);
  await blocks.nth(0).getByText('Blockierungsdetails').click();
  assert.match(await blocks.nth(0).textContent(), /Passende Regel: \+4930\*/);
  assert.match(await page.locator('tbody tr').last().textContent(), /besetzt\/abgelehnt/);
  await blocks.first().getByRole('button', { name: 'Zur Whitelist hinzufügen' }).click();
  const dialog = page.getByRole('dialog', { name: 'Zur Whitelist hinzufügen' });
  await dialog.waitFor();
  assert.match(await dialog.textContent(), /\+493012345600/);
  assert.match(await dialog.textContent(), /gesamten Mandanten/);
  await dialog.getByLabel('Notiz').fill('Trusted');
  f.setFail(true);
  await dialog.getByRole('button', { name: 'Nummer freigeben' }).click();
  await dialog.getByText('Permission revoked', { exact: false }).waitFor();
  assert.equal(await dialog.getByLabel('Notiz').inputValue(), 'Trusted');
  f.setFail(false);
  await dialog.getByRole('button', { name: 'Nummer freigeben' }).click();
  await dialog.waitFor({ state: 'detached' });
  assert.equal(await blocks.getByText('Auf Whitelist', { exact: true }).count(), 2);
  assert.match(await blocks.first().textContent(), /Blockiert · Sperrliste/);
  assert.deepEqual(f.posts.at(-1), { number: phone, label: 'Trusted' });
  // Keyboard: the result filter includes the new value and retains it in the address.
  await page.getByRole('button', { name: 'Ergebnis filtern', exact: true }).click();
  const panel = page.getByRole('dialog', { name: 'Filter: Ergebnis' });
  await panel.getByLabel('Blockiert', { exact: true }).check();
  await page.getByText('3 von 3 Anrufen').waitFor();
  assert.ok(page.url().includes('result=blocked'));
  await page.keyboard.press('Escape');
  await page.goto(BASE);
  await page.getByTestId('recent-content').getByTestId('blocked-call').first().waitFor();
  const firstRecent = page.getByTestId('recent-content').locator('li').first();
  assert.match(await firstRecent.textContent(), /Eingehender Anruf an \+493012345699/);
  assert.doesNotMatch(await firstRecent.textContent(), /Verpasst|Nst: Unbekannt/);
  assert.match(await firstRecent.textContent(), /Auf Whitelist/);
  // A role with only the whitelist permission can find and use its settings.
  await page.goto(`${BASE}/routing`);
  await page.getByRole('heading', { name: 'Whitelist', exact: true }).waitFor();
  assert.equal(await page.getByRole('tab').count(), 1);
  assert.equal(f.requests.filter(([, path]) => ['/api/v1/call-blocks', '/api/v1/call-blocks/settings', '/api/v1/settings', '/api/v1/extensions', '/api/v1/ring-groups'].includes(path)).length, 0);
  await page.getByRole('button', { name: 'Löschen', exact: true }).click();
  const remove = page.getByRole('dialog', { name: 'Whitelist-Ausnahme entfernen' });
  await remove.waitFor();
  assert.match(await remove.textContent(), /wieder.*blockiert/);
  await remove.getByRole('button', { name: 'Löschen' }).click();
  await page.getByText('Keine Whitelist-Einträge.').waitFor();
  await page.getByLabel('Internationale Rufnummer').fill('+4930888888');
  await page.getByRole('button', { name: 'Zur Whitelist hinzufügen' }).click();
  await page.getByRole('dialog', { name: 'Zur Whitelist hinzufügen' }).waitFor();
  await page.keyboard.press('Escape');
  assert.deepEqual(f.errors, []);
  await f.context.close();

  const no = await open({ permissions: ['calls.view_all'] });
  await no.page.goto(`${BASE}/calls`);
  await no.page.getByTestId('blocked-call').first().waitFor();
  assert.equal(await no.page.getByRole('button', { name: 'Zur Whitelist hinzufügen' }).count(), 0);
  assert.match(await no.page.getByTestId('blocked-call').first().textContent(), /Blockiert · Sperrliste/);
  assert.deepEqual(no.errors, []);
  await no.context.close();

  for (const locale of ['de', 'en']) for (const theme of ['dark', 'light']) {
    const f = await open({ locale, theme, width: 360 });
    for (const path of ['/calls', '/', '/routing']) {
      await f.page.goto(`${BASE}${path}`);
      await (path === '/routing' ? f.page.getByRole('heading', { name: 'Whitelist', exact: true }) : f.page.getByTestId('blocked-call').first()).waitFor();
      const width = await f.page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
      assert.ok(width[0] <= width[1], `${locale}/${theme}/${path}: ${width}`);
      if (path === '/calls' && process.env.SCREENSHOT_DIR) await f.page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/blocking-${locale}-${theme}.png`, fullPage: true });
    }
    assert.deepEqual(f.errors, []);
    await f.context.close();
  }
  console.log('call blocking browser checks passed');
} finally { await browser.close(); }
