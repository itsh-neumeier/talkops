// Browser checks of loading states and motion: delayed skeletons, the page
// fade and the dialog transitions, with and without reduced motion.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const me = { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', role_id: null, enabled: true }, csrf_token: 'x', permissions: ['users.view', 'users.manage'] };
const users = [{ id: 'u1', username: 'admin', display_name: 'Admin', email: null, role: 'admin', role_id: null, enabled: true, auth_source: 'local', last_login_at: null, totp_enabled: false }];

async function open(reducedMotion, delayMs) {
  const context = await browser.newContext({ locale: 'de-DE', reducedMotion, viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/v1/users') await new Promise(r => setTimeout(r, delayMs));
    const value = path === '/api/v1/auth/me' ? me : path === '/api/v1/users' ? users : path === '/api/v1/roles' ? { permissions: [], roles: [] } : [];
    await route.fulfill({ status: 200, json: value });
  });
  return { context, page, errors };
}

const opacity = (page) => page.evaluate(() => {
  const el = document.querySelector('main [data-testid="skeleton"]');
  return el ? Number(getComputedStyle(el).opacity) : null;
});

try {
  // Normal motion, a slow answer: the skeleton is invisible at first, then
  // fades in, and the list replaces it.
  {
    const { context, page, errors } = await open('no-preference', 1500);
    await page.goto(`${BASE}/settings`);
    await page.locator('main h1').first().waitFor({ timeout: 20_000 });
    await page.getByRole('link', { name: 'Benutzer' }).first().click();
    await page.locator('main [data-testid="skeleton"]').first().waitFor({ state: 'attached' });
    const early = await opacity(page);
    assert.ok(early !== null && early < 0.3, `early skeleton opacity ${early}`);
    await page.waitForTimeout(700);
    const later = await opacity(page);
    assert.ok(later !== null && later > 0.9, `skeleton opacity after the delay ${later}`);
    const sweep = await page.evaluate(() => getComputedStyle(document.querySelector('main .skeleton-bar')).animationName);
    assert.equal(sweep, 'skeleton-sweep');
    await page.getByRole('cell', { name: 'admin', exact: true }).waitFor();
    assert.equal(await page.locator('main [data-testid="skeleton"]').count(), 0);
    // The page content has the fade-in wrapper.
    assert.equal(await page.locator('main > .page-in').count(), 1);

    // Dialog: opens with a transition and closes with Escape.
    await page.getByRole('button', { name: 'Neuer Benutzer' }).click();
    const dialog = page.getByRole('dialog', { name: 'Neuer Benutzer' });
    await dialog.waitFor();
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'detached', timeout: 2000 });
    // Buttons give press feedback through a transition.
    const transition = await page.evaluate(() => getComputedStyle(document.querySelector('.btn')).transitionProperty);
    assert.match(transition, /background-color/);
    assert.deepEqual(errors, []);
    await context.close();
  }
  // Reduced motion: no waiting skeleton delay and no running sweep.
  {
    const { context, page, errors } = await open('reduce', 1500);
    await page.goto(`${BASE}/users`);
    await page.locator('main [data-testid="skeleton"]').first().waitFor({ state: 'attached', timeout: 20_000 });
    await page.waitForTimeout(100);
    const shown = await opacity(page);
    assert.ok(shown !== null && shown > 0.9, `reduced motion shows the skeleton at once (${shown})`);
    const iterations = await page.evaluate(() => getComputedStyle(document.querySelector('main .skeleton-bar')).animationIterationCount);
    assert.equal(iterations, '1');
    await page.getByRole('cell', { name: 'admin', exact: true }).waitFor();
    assert.deepEqual(errors, []);
    await context.close();
  }
  console.log('motion browser checks passed');
} finally {
  await browser.close();
}
