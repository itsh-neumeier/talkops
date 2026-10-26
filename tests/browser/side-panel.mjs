// Side panels (UniFi Talk style) against a production preview with API
// fixtures: phones (overview, manage, reassign dialog with search),
// extensions (devices) and users (edit), the `?panel=` deep link, Escape
// and the mobile layout.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const now = new Date().toISOString();
const ext = (id, number, name, user_id = null) => ({ id, number, display_name: name, user_id, outbound_number_id: null, hide_caller_id: false, ring_timeout_secs: 30, enabled: true, dnd: id === 'e2', forward_all: null, record_calls: 'inherit', video_enabled: false });
const extensions = [ext('e1', '101', 'Empfang', 'u1'), ext('e2', '102', 'Lager'), ext('e3', '103', 'Büro Müller')];
const phone = { id: 'p1', mac: '80:5e:c0:00:00:01', model: 't54w', name: 'Touch C4F3', line_keys: [], last_seen_at: now, last_ip: '10.0.20.31', last_firmware: '96.86.0.70', ringtone_id: null, wallpaper_id: null, phonebook_sections: [], settings: {} };
const users = [{ id: 'u1', username: 'ingo', display_name: 'Ingo Heinicke', email: 'ingo@example.com', role: 'user', role_id: null, enabled: true, auth_source: 'local', last_login_at: now, totp_enabled: true }];
const calls = [];
async function context(width) {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  await context.route('**/api/**', async (route) => {
    const req = route.request();
    const path = new URL(req.url()).pathname;
    if (req.method() !== 'GET') calls.push(`${req.method()} ${path}`);
    let value = [];
    if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (path === '/api/v1/setup') value = { needs_setup: false };
    else if (path === '/api/v1/phones') value = [{ ...phone, extensions: [{ number: '101', display_name: 'Empfang' }] }];
    else if (path === '/api/v1/phones/p1') value = { ...phone, accounts: [{ account_index: 1, device_id: 'd1', extension_id: 'e1', extension_number: '101', display_name: 'Empfang', phone_label: '', phone_display_name: '' }] };
    else if (path === '/api/v1/phone-models') value = [{ id: 't54w', name: 'Yealink T54W', family: 'desk', accounts: 1, provisioning_capabilities: { native: true, adaptation: '', device_validation: '', resync: true } }];
    else if (path === '/api/v1/phones/network') value = { state: 'disabled', error: null, read_at: null, phones: [] };
    else if (path === '/api/v1/phone-settings') value = { catalog: [], values: {} };
    else if (path === '/api/v1/provisioning/options') value = { endpoint: 'https://pbx.example/provisioning', sip_host: '192.168.1.10' };
    else if (path === '/api/v1/extensions') value = extensions;
    else if (path === '/api/v1/extensions/e1') value = { ...extensions[0], devices: [{ id: 'd1', extension_id: 'e1', browser_user_id: null, name: 'Touch C4F3', kind: 'desk', sip_username: 'u101a', phone_id: 'p1', account_index: 1, phone_label: '', phone_display_name: '', codec_profile: null, video: null }] };
    else if (path === '/api/v1/extensions/e2') value = { ...extensions[1], devices: [] };
    else if (path === '/api/v1/users') value = users;
    else if (path === '/api/v1/roles') value = { roles: [] };
    else if (path === '/api/v1/phones/p1/resync') value = { notified: 1 };
    else if (path.endsWith('/devices') && req.method() === 'POST') value = { id: 'd2', sip_username: 'u102a', sip_password: 'x' };
    await route.fulfill({ status: 200, json: value });
  });
  return context;
}
const problems = [];
try {
  for (const width of [1280, 390]) {
    calls.length = 0;
    const ctx = await context(width);
    const page = await ctx.newPage();
    page.on('pageerror', (e) => problems.push(`${width}: ${e.message}`));

    await page.goto(`${BASE}/phones`);
    await page.getByRole('link', { name: 'Touch C4F3' }).click();
    const panel = page.getByTestId('side-panel');
    await panel.waitFor();
    assert.match(page.url(), /\?panel=p1/);
    assert.match(await panel.textContent(), /80:5e:c0:00:00:01/);
    await panel.getByRole('tab', { name: 'Einstellungen' }).click();
    await panel.getByRole('button', { name: 'Neu zuweisen' }).click();
    const dialog = page.getByRole('dialog', { name: 'Gerät neu zuweisen' });
    await dialog.waitFor();
    assert.match(await dialog.textContent(), /Touch C4F3 ist 101 Empfang zugewiesen/);
    assert.match(await dialog.textContent(), /Alle Konten dieses Telefons sind belegt/);
    const combo = dialog.getByRole('combobox', { name: 'Zuweisen an' });
    await combo.fill('mull');
    const options = dialog.getByRole('option');
    assert.equal(await options.count(), 1, 'search folds umlauts');
    await combo.fill('lag');
    await combo.press('Enter');
    await mkdir('test-results', { recursive: true });
    await page.screenshot({ path: `test-results/side-panel-reassign-${width}.png` });
    await dialog.getByRole('button', { name: 'Zuweisen' }).click();
    await panel.getByText('Zuweisung geändert.').waitFor();
    assert.deepEqual(calls, ['DELETE /api/v1/devices/d1', 'POST /api/v1/extensions/e2/devices', 'POST /api/v1/phones/p1/resync']);
    await page.screenshot({ path: `test-results/side-panel-phone-${width}.png` });
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    assert.ok(overflow <= 0, `${width}: page scrolls sideways by ${overflow}px`);
    await page.keyboard.press('Escape');
    await panel.waitFor({ state: 'detached' });
    assert.doesNotMatch(page.url(), /panel=/);

    // Deep link and the extension panel.
    await page.goto(`${BASE}/extensions?panel=e1`);
    await panel.waitFor();
    await panel.getByRole('tab', { name: 'Geräte' }).click();
    await panel.getByText('u101a', { exact: false }).waitFor();
    // On a phone the panel covers the list: close it first.
    if (width < 1024) {
      await page.keyboard.press('Escape');
      await panel.waitFor({ state: 'detached' });
    }
    await page.getByRole('row').filter({ hasText: 'Lager' }).click();
    assert.match(page.url(), /panel=e2/);
    await panel.getByRole('tab', { name: 'Geräte' }).click();
    await panel.getByText('Keine Geräte.').waitFor();
    await panel.getByRole('tab', { name: 'Übersicht' }).click();
    assert.match(await panel.textContent(), /Nicht stören/);

    // The user panel uses the edit dialog of the page.
    await page.goto(`${BASE}/users`);
    await page.getByRole('button', { name: 'ingo' }).click();
    await panel.waitFor();
    await panel.getByRole('tab', { name: 'Einstellungen' }).click();
    await panel.getByRole('button', { name: 'Bearbeiten' }).click();
    await page.getByRole('dialog', { name: 'Bearbeiten' }).waitFor();
    await page.screenshot({ path: `test-results/side-panel-user-${width}.png` });
    await ctx.close();
  }
  // An open panel must not squeeze the list into the centred content
  // column: the content moves left beside the panel, and on a very wide
  // screen the list keeps its full width.
  for (const width of [1280, 2000]) {
    const ctx = await context(width);
    const page = await ctx.newPage();
    for (const path of ['/extensions', '/phones', '/users']) {
      await page.goto(`${BASE}${path}`);
      // Wait for the loaded list, not the layout's temporary skeleton card.
      const link = page.locator('main [data-panel-link]').first();
      await link.waitFor();
      const card = page.locator('main .card').filter({ has: page.locator('[data-panel-link]') }).first();
      await card.waitFor();
      const before = await card.boundingBox();
      assert.ok(before, `${width} ${path}: loaded list has no bounding box`);
      await link.click();
      const panel = page.getByTestId('side-panel');
      await panel.waitFor();
      await page.waitForTimeout(50);
      const after = await card.boundingBox();
      const side = await panel.boundingBox();
      assert.ok(after.x + after.width <= side.x + 1, `${width} ${path}: list under the panel`);
      if (width === 2000) assert.ok(after.width >= before.width - 1, `${width} ${path}: list ${before.width}px → ${after.width}px`);
    }
    await ctx.close();
  }
  assert.deepEqual(problems, []);
  console.log('side panel browser checks passed');
} finally {
  await browser.close();
}
