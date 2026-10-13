// Browser checks for the transcription settings (ADR 0024) with API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
try {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  page.on('dialog', dialog => dialog.accept());
  const local = (purpose, model) => ({ purpose, engine: purpose.endsWith('_refine') ? 'off' : 'local', local_model: model, provider_id: null, model: '', realtime: false, price_per_minute: null, list_price: null });
  let providers = [];
  let profiles = [local('live', 'base'), local('recording', 'fast'), local('recording_refine', 'fast'), local('voicemail', 'fast'), local('voicemail_refine', 'fast')];
  const saved = { providers: [], profiles: null, deleted: [] };
  const settings = {
    country_code: '49', area_code: '9543', national_prefix: '0', international_prefix: '00', emergency_numbers: ['110', '112'],
    external_ip: '', default_language: 'de', default_number_id: null, timezone: 'Europe/Berlin', record_inbound: true,
    record_outbound: true, record_internal: false, recording_announcement: true, recording_retention_days: 90,
    transcription_enabled: true, sip_uri_dialing: true, transcription_vocabulary: '', hold_music: '', hold_music_clip_id: null
  };
  await page.route('**/api/**', async route => {
    const req = route.request();
    const path = new URL(req.url()).pathname;
    let value = [];
    if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (path === '/api/v1/settings') value = settings;
    else if (path === '/api/v1/settings/smtp') value = { host: '', port: 587, security: 'starttls', username: '', has_password: false, from: '' };
    else if (path === '/api/v1/audio/music') value = [];
    else if (path === '/api/v1/settings/transcription') value = {
      providers, profiles,
      usage: [
        { month: '2026-10-01', purpose: 'live', engine: 'OpenAI/gpt-live-transcribe', seconds: 1800, cost: 0.51, unpriced_seconds: 0 },
        { month: '2026-10-01', purpose: 'recording', engine: 'LAN/whisper', seconds: 600, cost: 0, unpriced_seconds: 600 }
      ],
      openai_prices: [{ model: 'gpt-transcribe', price_per_minute: 0.0045 }, { model: 'gpt-live-transcribe', price_per_minute: 0.017 }]
    };
    else if (path === '/api/v1/settings/transcription/providers' && req.method() === 'POST') {
      const body = req.postDataJSON(); saved.providers.push(body);
      value = { id: 'p1', name: body.name, kind: body.kind, url: 'https://api.openai.com/v1', has_key: !!body.key };
      providers = [value];
      await route.fulfill({ status: 201, json: value }); return;
    } else if (path === '/api/v1/settings/transcription/profiles') {
      saved.profiles = req.postDataJSON();
      profiles = saved.profiles.map(p => ({ ...p, list_price: p.model === 'gpt-live-transcribe' ? 0.017 : null }));
      value = profiles;
    } else if (path.startsWith('/api/v1/settings/transcription/providers/') && req.method() === 'DELETE') {
      saved.deleted.push(path); await route.fulfill({ status: 409, json: { error: 'conflict', message: 'connection is in use for: live' } }); return;
    }
    await route.fulfill({ status: 200, json: value });
  });
  await page.goto(`${process.env.BASE || 'http://127.0.0.1:5184'}/settings#calls`);
  const box = page.getByTestId('transcription-settings');
  await box.waitFor({ timeout: 20_000 });
  // Cloud cannot be selected without a connection.
  const live = page.getByTestId('stt-live');
  assert.equal(await live.locator('option[value="cloud"]').isDisabled(), true);
  assert.equal(await page.getByTestId('stt-recording').locator('option[value="off"]').count(), 0);
  assert.equal(await page.getByTestId('stt-recording_refine').locator('option[value="off"]').count(), 1);
  // Usage with estimate and a note on audio without a price.
  const usage = await page.getByTestId('stt-usage').textContent();
  assert.match(usage, /Live-Vorschau.*OpenAI\/gpt-live-transcribe.*30.*0,51/s);
  assert.match(usage, /\*/);

  // Add an OpenAI connection: the URL is fixed, the key is sent once.
  await box.getByRole('button', { name: 'Verbindung hinzufügen' }).click();
  assert.equal(await page.locator('#stt-url').count(), 0);
  await page.locator('#stt-key').fill('sk-test');
  await box.getByRole('button', { name: 'Verbindung speichern' }).click();
  await box.getByTestId('stt-provider').getByText('Schlüssel gespeichert', { exact: false }).waitFor();
  assert.deepEqual(saved.providers, [{ name: 'OpenAI', kind: 'openai', url: 'https://api.openai.com/v1', key: 'sk-test' }]);

  // Live over the cloud with streaming; second recording pass in the cloud.
  await live.locator('select').first().selectOption('cloud');
  await live.getByText('Live streamen (Realtime-API)').click();
  assert.equal(await live.getByLabel('Modell').inputValue(), 'gpt-live-transcribe');
  await live.getByText('Listenpreis', { exact: false }).waitFor();
  assert.match(await box.textContent(), /Die Live-Vorschau sendet Audio schon während des Gesprächs/);
  const refine = page.getByTestId('stt-recording_refine');
  await refine.locator('select').first().selectOption('cloud');
  await refine.getByLabel('Preis pro Minute').fill('0.004');
  await page.getByTestId('stt-voicemail').locator('select').nth(1).selectOption('german');
  await box.getByRole('button', { name: 'Engines speichern' }).click();
  await page.getByTestId('stt-info').waitFor();
  const byPurpose = Object.fromEntries(saved.profiles.map(p => [p.purpose, p]));
  assert.deepEqual(
    { engine: byPurpose.live.engine, provider: byPurpose.live.provider_id, model: byPurpose.live.model, realtime: byPurpose.live.realtime },
    { engine: 'cloud', provider: 'p1', model: 'gpt-live-transcribe', realtime: true });
  assert.deepEqual(
    { engine: byPurpose.recording_refine.engine, model: byPurpose.recording_refine.model, price: byPurpose.recording_refine.price_per_minute },
    { engine: 'cloud', model: 'gpt-transcribe', price: 0.004 });
  assert.equal(byPurpose.voicemail.local_model, 'german');
  assert.equal(byPurpose.recording.engine, 'local');

  // Deleting a connection in use shows the server's reason.
  await box.getByRole('button', { name: 'Löschen' }).click();
  await box.getByText('in use for: live', { exact: false }).waitFor();

  // Mobile: no horizontal overflow.
  await page.setViewportSize({ width: 320, height: 780 });
  await page.waitForTimeout(300);
  const geometry = await page.evaluate(() => ({ width: innerWidth, scroll: document.documentElement.scrollWidth }));
  assert.ok(geometry.scroll <= geometry.width, JSON.stringify(geometry));
  if (process.env.SCREENSHOT_DIR) { await mkdir(process.env.SCREENSHOT_DIR, { recursive: true }); await page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/transcription-settings-mobile.png`, fullPage: true }); }
  assert.deepEqual(errors, []);
  await context.close();
  console.log('PASS: transcription connections, engines per purpose, streaming, prices, usage, errors and mobile layout');
} finally { await browser.close(); }
