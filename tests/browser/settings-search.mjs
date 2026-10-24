// Settings search against a production preview with API fixtures: results
// in both languages, permission filter, keyboard use, the jump to a section
// with the marked heading and a setup page tab.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const now = new Date().toISOString();
const settings = { country_code: '49', area_code: '9543', national_prefix: '0', international_prefix: '00', emergency_numbers: ['110', '112'], external_ip: '', default_language: 'de', default_number_id: null, timezone: 'Europe/Berlin', record_inbound: false, record_outbound: false, record_internal: false, recording_announcement: false, recording_retention_days: 30, transcription_enabled: false, transcription_quality: 'fast', transcription_refine: '', sip_uri_dialing: false, transcription_vocabulary: '', hold_music: '', hold_music_clip_id: null };
async function context(width, role) {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  await context.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    let value = [];
    if (path === '/api/v1/auth/me') value = role === 'admin'
      ? { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' }
      : { user: { id: 'u2', username: 'user', display_name: 'User', role: 'user', enabled: true, permissions: [] }, csrf_token: 'fixture' };
    else if (path === '/api/v1/setup') value = { needs_setup: false };
    else if (path === '/api/v1/settings') value = settings;
    else if (path === '/api/v1/settings/smtp') value = { host: '', port: 587, security: 'starttls', username: '', has_password: false, from: '' };
    else if (path === '/api/v1/unifi/connection') value = { connection: null, has_api_key: false };
    await route.fulfill({ status: 200, json: value });
  });
  return context;
}
try {
  for (const width of [390, 1280]) {
    const ctx = await context(width, 'admin');
    const page = await ctx.newPage();
    await page.goto(`${BASE}/settings?section=account`);
    const box = page.getByRole('combobox', { name: 'Einstellungen durchsuchen' });
    await box.waitFor();

    // German synonym, then Enter opens the section and marks the heading.
    await box.fill('wartemusik');
    const options = page.locator('#settings-search-results [role=option]');
    await options.first().waitFor();
    assert.match(await options.first().textContent(), /Anrufbehandlung/);
    await mkdir('test-results', { recursive: true });
    await page.screenshot({ path: `test-results/settings-search-${width}.png` });
    await box.press('Enter');
    await page.waitForURL(/section=calls/);
    await page.locator('.search-hit').waitFor();
    assert.match(await page.locator('.search-hit h2').first().textContent(), /Wartemusik/i);

    // The keyboard shortcut and the arrow keys.
    await page.locator('body').click({ position: { x: 5, y: 300 } });
    await page.keyboard.press('/');
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'settings-search');
    await page.keyboard.type('zwei faktor');
    await options.first().waitFor();
    await page.keyboard.press('Enter');
    await page.waitForURL(/section=account/);
    await page.locator('.search-hit').waitFor();

    // A system tab and a setup page tab.
    await box.fill('sip trace');
    await options.first().waitFor();
    await box.press('Enter');
    await page.waitForURL(/section=system&tab=diagnostics/);
    await box.fill('ubiquiti');
    await options.first().waitFor();
    await box.press('Enter');
    await page.waitForURL(/\/wifi-surveys\?tab=unifi/);

    await box.fill('xyzzy');
    await page.getByText('Keine Einstellung gefunden.').waitFor();
    await box.press('Escape');
    assert.equal(await box.inputValue(), '');
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    assert.ok(overflow <= 0, `${width}: page scrolls sideways by ${overflow}px`);
    await ctx.close();
  }

  // A user without permissions finds only the own account.
  const ctx = await context(1280, 'user');
  const page = await ctx.newPage();
  await page.goto(`${BASE}/settings?section=account`);
  const box = page.getByRole('combobox', { name: 'Einstellungen durchsuchen' });
  await box.fill('ldap');
  await page.getByText('Keine Einstellung gefunden.').waitFor();
  await box.fill('passwort');
  await page.locator('#settings-search-results [role=option]').first().waitFor();
  assert.match(await page.locator('#settings-search-results [role=option]').first().textContent(), /Mein Konto/);
  await ctx.close();
  console.log('settings search browser checks passed');
} finally {
  await browser.close();
}
