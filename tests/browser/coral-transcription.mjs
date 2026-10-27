// Browser checks for Coral speech recognition (ADR 0046) with API fixtures:
// the connection kind "Coral (this server)", its TPU status, Coral models,
// the hybrid button, the local fallback, and the live preview with a
// fallback note and detected languages.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
try {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  page.on('dialog', dialog => dialog.accept());
  const local = (purpose, model) => ({ purpose, engine: purpose.endsWith('_refine') ? 'off' : 'local', local_model: model, provider_id: null, model: '', realtime: false, price_per_minute: null, list_price: null, provider_kind: null });
  let providers = [];
  let profiles = [local('live', 'base'), local('recording', 'fast'), local('recording_refine', 'fast'), local('voicemail', 'fast'), local('voicemail_refine', 'fast')];
  const saved = { providers: [], profiles: null, tests: 0 };
  const settings = {
    country_code: '49', area_code: '9543', national_prefix: '0', international_prefix: '00', emergency_numbers: ['110', '112'],
    external_ip: '', default_language: 'de', default_number_id: null, timezone: 'Europe/Berlin', record_inbound: true,
    record_outbound: true, record_internal: false, recording_announcement: true, recording_retention_days: 90,
    transcription_enabled: true, sip_uri_dialing: true, transcription_vocabulary: '', hold_music: '', hold_music_clip_id: null
  };
  const health = {
    status: 'degraded', version: '1.8.10', backend: 'tpu',
    tpus: [
      { id: 'usb:0', kind: 'usb', node: '/dev/bus/usb/002/007', state: 'ready', busy: false, load: 0.12, inferences: 40, models: ['coral-ctc-de', 'coral-ctc-en', 'langid'], error: null },
      { id: 'pci:0', kind: 'pcie', node: '/dev/apex_0', state: 'error', busy: false, load: 0, inferences: 0, models: [], error: 'device not found' }
    ],
    queue: { length: 1, capacity: 16, sessions: 2, max_sessions: 4 },
    models: ['coral-ctc-de', 'coral-ctc-en', 'coral-auto']
  };
  await page.route('**/api/**', async route => {
    const req = route.request();
    const path = new URL(req.url()).pathname;
    let value = [];
    if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (path === '/api/v1/settings') value = settings;
    else if (path === '/api/v1/settings/smtp') value = { host: '', port: 587, security: 'starttls', username: '', has_password: false, from: '' };
    else if (path === '/api/v1/audio/music') value = [];
    else if (path === '/api/v1/settings/transcription') value = { providers, profiles, usage: [], openai_prices: [{ model: 'gpt-transcribe', price_per_minute: 0.0045 }] };
    else if (path === '/api/v1/settings/transcription/providers' && req.method() === 'POST') {
      const body = req.postDataJSON(); saved.providers.push(body);
      value = { id: 'coral1', name: body.name, kind: body.kind, url: body.url, has_key: !!body.key };
      providers = [value];
      await route.fulfill({ status: 201, json: value }); return;
    } else if (path === '/api/v1/settings/transcription/providers/coral1/test') {
      saved.tests++;
      value = { missing_models: [], speech_models: ['coral-auto', 'coral-ctc-de', 'coral-ctc-en'], coral: health };
    } else if (path === '/api/v1/settings/transcription/profiles') {
      saved.profiles = req.postDataJSON();
      profiles = saved.profiles.map(p => ({ ...p, provider_kind: p.provider_id === 'coral1' ? 'coral' : null }));
      value = profiles;
    }
    await route.fulfill({ status: 200, json: value });
  });
  await page.goto(`${BASE}/settings#calls`);
  const box = page.getByTestId('transcription-settings');
  await box.waitFor({ timeout: 20_000 });
  assert.equal(await box.getByTestId('stt-hybrid').count(), 0, 'no hybrid button without Coral');

  // Add the Coral connection: the URL is preset to loopback, no key.
  await box.getByRole('button', { name: 'Verbindung hinzufügen' }).click();
  await page.locator('#stt-preset').selectOption('coral');
  assert.equal(await page.locator('#stt-preset option[value="coral"]').textContent(), 'Coral (dieser Server)');
  assert.equal(await page.locator('#stt-url').inputValue(), 'http://127.0.0.1:8095/v1');
  await box.getByText('Nur Loopback-Adressen', { exact: false }).waitFor();
  await box.getByRole('button', { name: 'Verbindung speichern' }).click();
  await box.getByTestId('stt-provider').getByText('Coral', { exact: true }).first().waitFor();
  assert.deepEqual(saved.providers, [{ name: 'Coral', kind: 'coral', url: 'http://127.0.0.1:8095/v1', key: null }]);

  // Connection test with the TPU status.
  await box.getByTestId('stt-provider').getByRole('button', { name: 'Verbindung testen' }).click();
  const status = box.getByTestId('stt-coral-status');
  await status.waitFor();
  const text = await status.textContent();
  assert.match(text, /eingeschränkt/);
  assert.match(text, /usb:0 \(USB\): ready, Last 12 %, Modelle: coral-ctc-de, coral-ctc-en, langid/);
  assert.match(text, /pci:0 \(PCIE\): error \(device not found\)/);
  assert.match(text, /Warteschlange 1\/16, Live-Sitzungen 2\/4/);
  assert.equal(saved.tests, 1);

  // Hybrid: Coral Realtime live, Coral first passes, local second passes.
  await box.getByTestId('stt-hybrid').click();
  await page.getByTestId('stt-info').getByText('Hybrid-Einstellung übernommen', { exact: false }).waitFor();
  const live = page.getByTestId('stt-live');
  assert.equal(await live.getByLabel('Modell').inputValue(), 'coral-auto');
  assert.equal(await live.getByText('Live streamen (Coral Realtime)').count(), 1);
  assert.equal(await live.getByRole('checkbox').first().isChecked(), true);
  await live.getByTestId('stt-coral-stable-preview').check();
  await live.getByTestId('stt-live-fallback').waitFor();
  assert.match(await live.getByTestId('stt-live-fallback').textContent(), /Whisper Base/);
  const recording = page.getByTestId('stt-recording');
  assert.equal(await recording.getByLabel('Preis pro Minute').count(), 0, 'Coral has no price');
  await recording.getByTestId('stt-recording-fallback').selectOption('accurate');
  const models = await live.locator('datalist option').evaluateAll(o => o.map(x => x.value));
  assert.deepEqual(models, ['coral-auto', 'coral-ctc-de', 'coral-ctc-en']);
  assert.doesNotMatch(await box.textContent(), /gehen Gesprächsaufnahmen und Sprachnachrichten an den Anbieter/, 'no cloud privacy note for Coral');
  await box.getByRole('button', { name: 'Engines speichern' }).click();
  await page.getByTestId('stt-info').getByText('Gespeichert', { exact: false }).waitFor();
  const byPurpose = Object.fromEntries(saved.profiles.map(p => [p.purpose, p]));
  assert.equal(byPurpose.live.coral_stable_preview, true, 'complete-word preview is saved in the live profile');
  assert.deepEqual(
    { engine: byPurpose.live.engine, provider: byPurpose.live.provider_id, model: byPurpose.live.model, realtime: byPurpose.live.realtime, fallback: byPurpose.live.local_model },
    { engine: 'cloud', provider: 'coral1', model: 'coral-auto', realtime: true, fallback: 'base' });
  for (const purpose of ['recording', 'voicemail']) {
    assert.deepEqual(
      { engine: byPurpose[purpose].engine, provider: byPurpose[purpose].provider_id, model: byPurpose[purpose].model, realtime: byPurpose[purpose].realtime, price: byPurpose[purpose].price_per_minute },
      { engine: 'cloud', provider: 'coral1', model: 'coral-auto', realtime: false, price: null }, purpose);
  }
  assert.equal(byPurpose.recording.local_model, 'accurate');
  assert.equal(byPurpose.voicemail.local_model, 'fast');
  for (const purpose of ['recording_refine', 'voicemail_refine']) {
    assert.deepEqual({ engine: byPurpose[purpose].engine, model: byPurpose[purpose].local_model }, { engine: 'local', model: 'fast' }, purpose);
  }

  // Mobile: no horizontal overflow with the TPU status.
  await page.setViewportSize({ width: 320, height: 780 });
  await page.waitForTimeout(300);
  let geometry = await page.evaluate(() => ({ width: innerWidth, scroll: document.documentElement.scrollWidth }));
  assert.ok(geometry.scroll <= geometry.width, JSON.stringify(geometry));
  if (process.env.SCREENSHOT_DIR) { await mkdir(process.env.SCREENSHOT_DIR, { recursive: true }); await page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/coral-settings-mobile.png`, fullPage: true }); }

  // Live preview: Coral is down, the local fallback runs; then Coral again
  // with detected languages.
  await page.setViewportSize({ width: 1280, height: 900 });
  const started = new Date(Date.now() - 120_000).toISOString();
  const caller = { name: 'Familie Zweig', number: '+4995431241', extension_id: 'local', ambiguous: false };
  const called = { name: 'Support', number: '+4930123456', extension_id: null, ambiguous: false };
  let fallback = true;
  await page.unroute('**/api/**');
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    let value = [];
    if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
    else if (path === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (path === '/api/v1/telephony/status') value = { connected: true, registrations: [], gateways: {} };
    else if (path === '/api/v1/stats/calls') value = { range: '1d', buckets: [], total: 0, missed: 0, avg_talk_secs: 0, answer_rate: null, inbound: 0, outbound: 0, internal: 0 };
    else if (path === '/api/v1/backups') value = { settings: { last_run_at: null } };
    else if (path === '/api/v1/telephony/calls') value = [{ uuid: 'c1', direction: 'outbound', caller, called, started_at: started, answered_at: started, state: 'talking', transcript_available: true }];
    else if (path.endsWith('/transcript')) {
      if (route.request().method() === 'DELETE') { await route.fulfill({ status: 204 }); return; }
      value = fallback
        ? { id: 'p', started_at: started, processed_secs: 8, status: 'running', engine: 'whisper:base', fallback_from: 'Coral/coral-auto',
            segments: [{ start: 1, end: 3, speaker: 'caller', text: 'Guten Tag, hier ist die Familie Zweig.' }] }
        : { id: 'p', started_at: started, processed_secs: 12, status: 'running', engine: 'api:Coral/coral-auto', fallback_from: null,
            segments: [
              { start: 1, end: 3, speaker: 'caller', text: 'Guten Tag, hier ist die Familie Zweig.', language: 'de' },
              { start: 4, end: 6, speaker: 'called', text: 'Hello, how can I help?', language: 'en' }
            ] };
    }
    await route.fulfill({ status: 200, json: value });
  });
  await page.goto(BASE);
  const section = page.locator('section').filter({ has: page.getByRole('heading', { name: 'Laufende Gespräche', exact: true }) });
  await section.getByRole('button', { name: 'Live-Transkript', exact: true }).first().click({ timeout: 20_000 });
  const dialog = page.getByRole('dialog');
  await dialog.getByTestId('live-fallback').waitFor({ timeout: 8_000 });
  assert.match(await dialog.getByTestId('live-fallback').textContent(), /lokales Whisper/);
  assert.equal(await dialog.getByRole('alert').count(), 0, 'a fallback is no error');
  fallback = false;
  await dialog.getByText('Hello, how can I help?', { exact: true }).first().waitFor({ timeout: 8_000 });
  await dialog.getByTestId('live-fallback').waitFor({ state: 'detached', timeout: 8_000 });
  const languages = await dialog.getByTestId('live-language').allTextContents();
  assert.deepEqual(languages.map(l => l.trim()), ['de', 'en']);
  await dialog.getByRole('button', { name: 'Schließen', exact: true }).click();
  assert.deepEqual(errors, []);
  await context.close();
  console.log('PASS: Coral connection, TPU status, hybrid setup, local fallback, live fallback note and detected languages');
} finally { await browser.close(); }
