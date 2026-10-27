// Review mockup only. These checks do not validate application behavior.
// Run with PLAYWRIGHT_MODULE, as for the existing browser checks.
import assert from 'node:assert/strict';
import { readFile, mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const fragment = await readFile(new URL('../../docs/mockups/call-map-emergency.html', import.meta.url), 'utf8');
assert.ok(Buffer.byteLength(fragment) < 1_000_000);
assert.ok(!/<!doctype|<html|<head|<body|\\"|\\n/.test(fragment), 'literal HTML fragment');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const shots = process.env.MOCKUP_SCREENSHOT_DIR;
if (shots) await mkdir(shots, { recursive: true });
let cases = 0;
try {
  for (const colorScheme of ['light', 'dark']) {
    for (const width of [1024, 736, 320]) {
      const context = await browser.newContext({ colorScheme, viewport: { width, height: 1100 }, reducedMotion: 'reduce' });
      const page = await context.newPage();
      const errors = [], requests = [];
      page.on('pageerror', e => errors.push(e.message));
      page.on('request', r => requests.push(r.url()));
      await page.setContent(fragment);
      for (const design of ['phone', 'tile']) {
        // The host owns the carousel. Reveal each independent design for QA.
        await page.locator('[data-design]').evaluateAll((nodes, chosen) => nodes.forEach(n => n.hidden = n.dataset.design !== chosen), design);
        const stage = page.locator(`[data-design=${design}]`);
        const preview = stage.locator('[data-preview]');
        const selected = stage.locator('[data-selected]');
        assert.match(await selected.innerText(), /Keine Auswahl/);
        await stage.locator('[data-key=bamberg]').hover();
        assert.match(await preview.innerText(), /Bamberg/);
        assert.match(await preview.innerText(), /60/);
        assert.match(await selected.innerText(), /Keine Auswahl/);
        await stage.locator('[data-key=mobile]').focus();
        assert.match(await preview.innerText(), /147/);
        assert.match(await preview.innerText(), /Kein Standort/);
        await stage.locator('[data-key=mobile]').press('Enter');
        assert.match(await selected.innerText(), /Mobilnummern/);
        await stage.locator('[data-key=bamberg]').hover();
        assert.match(await selected.innerText(), /Mobilnummern/);
        await stage.locator('[data-key=bamberg]').focus();
        await stage.locator('[data-key=bamberg]').press('Escape');
        assert.equal(await preview.isVisible(), false);
        await stage.locator('[data-key=other]').focus();
        assert.match(await preview.innerText(), /2 sonstige/);
        await preview.getByRole('button', { name: 'Details öffnen' }).click();
        assert.match(await selected.innerText(), /Ohne Ortsbezug/);
        await selected.getByRole('button', { name: 'Schließen' }).click();
        assert.match(await selected.innerText(), /Keine Auswahl/);
        await stage.locator('[data-key=mobile]').hover();
        assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `map overflow at ${width}`);
        if (shots && width === 1024) await page.locator('#talkops-review').screenshot({ path: resolve(shots, `map-${design}-${colorScheme}.png`) });
        cases++;
      }
      await page.getByRole('button', { name: 'Benachrichtigungen', exact: true }).click();
      assert.equal(await page.locator('[data-alert-enabled]').isChecked(), false);
      await page.locator('[data-alert-enabled]').check();
      await page.getByLabel('Benutzer', { exact: true }).check();
      await page.locator('[data-recipient]').selectOption('Ben Beispiel');
      assert.match(await page.locator('[data-mail-to]').innerText(), /Ben Beispiel/);
      await page.getByRole('button', { name: 'Auswahl vormerken' }).click();
      assert.match(await page.locator('[data-status]').innerText(), /Keine Produktänderung/);
      await page.getByRole('button', { name: 'Testvorschau', exact: true }).click();
      assert.match(await page.locator('[data-status]').innerText(), /Kein E-Mail-Versand und kein Anruf/);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `settings overflow at ${width}`);
      if (shots && width === 320) await page.locator('#talkops-review').screenshot({ path: resolve(shots, `alert-${colorScheme}-mobile.png`) });
      assert.deepEqual(errors, [], 'no JavaScript errors');
      assert.deepEqual(requests, [], 'no network requests');
      await context.close();
    }
  }
  const touchContext = await browser.newContext({ hasTouch: true, isMobile: true, viewport: { width: 390, height: 900 } });
  const touchPage = await touchContext.newPage();
  await touchPage.setContent('<meta name="viewport" content="width=device-width, initial-scale=1">' + fragment);
  const stage = touchPage.locator('[data-design=phone]');
  await stage.locator('[data-key=bamberg]').tap();
  assert.match(await stage.locator('[data-preview]').innerText(), /Bamberg/);
  assert.match(await stage.locator('[data-selected]').innerText(), /Keine Auswahl/);
  await stage.locator('[data-preview]').getByRole('button', { name: 'Details öffnen' }).tap();
  assert.match(await stage.locator('[data-selected]').innerText(), /Bamberg/);
  await touchContext.close();
  console.log(`Passed: ${cases} map variant/theme/width cases; 6 settings cases; touch preview and details; no network or runtime errors.`);
} finally {
  await browser.close();
}
