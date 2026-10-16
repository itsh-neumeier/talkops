// Browser checks of the role editor and the permission-based menu against a
// production preview with API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const catalog = [
  ['calls.active', 'calls', false], ['calls.view_all', 'calls', false],
  ['phonebook.edit', 'phonebook', false], ['users.view', 'users', false], ['users.manage', 'users', true],
  ['audit.view', 'system', false], ['backups.manage', 'system', true]
].map(([key, area, sensitive]) => ({ key, area, sensitive }));
const at = '2026-10-15T09:00:00Z';
const roles = () => [
  { id: 'r-admin', builtin: 'admin', name: 'Administrator', description: '', permissions: [], directory_group: '', users: 1, updated_at: at },
  { id: 'r-op', builtin: 'operator', name: 'Operator', description: '', permissions: ['calls.active', 'calls.view_all', 'phonebook.edit', 'users.view'], directory_group: '', users: 0, updated_at: at },
  { id: 'r-user', builtin: 'user', name: 'Default user', description: '', permissions: [], directory_group: '', users: 2, updated_at: at },
  { id: 'r-rec', builtin: null, name: 'Empfang', description: 'Front desk', permissions: ['calls.active'], directory_group: 'pbx-reception', users: 0, updated_at: at }
];

async function open(me) {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const puts = [];
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    let value = [];
    if (url.pathname === '/api/v1/auth/me') value = me;
    else if (url.pathname === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (url.pathname === '/api/v1/settings') value = { emergency_numbers: [] };
    else if (url.pathname === '/api/v1/roles') value = { permissions: catalog, roles: roles() };
    else if (url.pathname.startsWith('/api/v1/roles/') && method === 'PUT') { puts.push([url.pathname, route.request().postDataJSON()]); value = roles()[1]; }
    else if (url.pathname === '/api/v1/users') value = [
      { id: 'u1', username: 'admin', display_name: 'Admin', email: null, role: 'admin', role_id: null, enabled: true, auth_source: 'local', last_login_at: null, totp_enabled: false },
      { id: 'u2', username: 'anna', display_name: 'Anna', email: null, role: 'user', role_id: 'r-rec', enabled: true, auth_source: 'local', last_login_at: null, totp_enabled: false }
    ];
    await route.fulfill({ status: 200, json: value });
  });
  return { context, page, errors, puts };
}

try {
  const admin = { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', role_id: null, enabled: true }, csrf_token: 'x', permissions: catalog.map(c => c.key) };
  const { context, page, errors, puts } = await open(admin);
  await page.goto(`${BASE}/users`);
  await page.getByRole('cell', { name: 'Empfang' }).waitFor({ timeout: 20_000 });
  await page.getByRole('tab', { name: 'Rollen' }).click();
  const matrix = page.locator('table.role-matrix');
  await matrix.waitFor();
  // Admin column: checked and locked.
  const adminBox = page.getByRole('checkbox', { name: 'Änderungsprotokoll: Administrator' });
  assert.equal(await adminBox.isChecked(), true);
  assert.equal(await adminBox.isDisabled(), true);
  // Sensitive permissions are marked.
  assert.ok((await matrix.getByText('Kritisch').count()) >= 2);
  // Change two cells of the default user role, see the save bar, save.
  await page.getByRole('checkbox', { name: 'Änderungsprotokoll: Standardbenutzer' }).check();
  await page.getByRole('checkbox', { name: 'Telefonbuch bearbeiten: Standardbenutzer' }).check();
  await page.getByText('Rollen mit ungespeicherten Änderungen: 1').waitFor();
  // The area box of "Anrufe" for the operator: all checked.
  assert.equal(await page.getByRole('checkbox', { name: 'Alle in „Anrufe“ für Operator' }).isChecked(), true);
  await page.getByRole('button', { name: 'Speichern' }).click();
  await page.getByText('Rollen gespeichert.').waitFor();
  assert.equal(puts.length, 1);
  assert.equal(puts[0][0], '/api/v1/roles/r-user');
  assert.deepEqual(puts[0][1].permissions, ['phonebook.edit', 'audit.view'], 'catalog order');
  // Own role in the user list and as a choice.
  await page.getByRole('tab', { name: 'Benutzer' }).click();
  await page.getByRole('cell', { name: 'Empfang' }).waitFor();
  // Narrow screen: the matrix scrolls inside its card, not the page.
  await page.getByRole('tab', { name: 'Rollen' }).click();
  await page.setViewportSize({ width: 360, height: 780 });
  const width = await page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(width[0] <= width[1], JSON.stringify(width));
  assert.deepEqual(errors, []);
  await context.close();

  // A user with "users.view" sees the roles but cannot change them; the
  // settings menu follows the permissions.
  const viewer = { user: { id: 'u3', username: 'vera', display_name: 'Vera', role: 'user', role_id: 'r-rec', enabled: true }, csrf_token: 'x', permissions: ['users.view', 'audit.view'] };
  const second = await open(viewer);
  await second.page.goto(`${BASE}/users#roles`);
  await second.page.locator('table.role-matrix').waitFor({ timeout: 20_000 });
  assert.equal(await second.page.getByRole('checkbox', { name: 'Änderungsprotokoll: Operator' }).isDisabled(), true);
  assert.equal(await second.page.getByRole('button', { name: 'Neue Rolle' }).count(), 0);
  assert.equal(await second.page.getByRole('button', { name: 'Neuer Benutzer' }).count(), 0);
  const nav = second.page.locator('nav').filter({ hasText: 'Benutzer' }).first();
  assert.equal(await nav.getByRole('link', { name: /Änderungsprotokoll|Audit/ }).count(), 1);
  assert.equal(await nav.getByRole('link', { name: /Nebenstellen/ }).count(), 0);
  assert.deepEqual(second.errors, []);
  await second.context.close();
  console.log('role editor browser checks passed');
} finally {
  await browser.close();
}
