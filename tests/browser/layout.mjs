// Layout regression checks against a production preview with API fixtures:
// no page scrolls sideways at phone, tablet and desktop width, the stored
// theme survives a reload (no CSP block) and native widgets follow it.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const now = new Date().toISOString();
const long = 'Verkehrsausscheidungsziffer-Langname GmbH & Co. KG Niederlassung';
const ext = (i) => ({ id: `e${i}`, number: `${100 + i}`, display_name: `${long} ${i}`, user_id: 'u1', outbound_number_id: 'n1', hide_caller_id: i === 1, ring_timeout_secs: 30, enabled: i !== 2, dnd: false, forward_all: null, record_calls: 'inherit', video_enabled: false });
const number = { id: 'n1', trunk_id: 't1', account_id: null, e164: '+4995431241000', label: long, destination_type: 'extension', destination_id: 'e1', extra_extensions: [], enabled: true };
const trunk = { id: 't1', name: long, preset: 'telekom', overrides: {}, enabled: true, video_enabled: false, accounts: [{ id: 'a1', trunk_id: 't1', username: '+4995431241000', auth_username: 'anschluss-kennung-0123456789', enabled: true, destination_type: 'none', destination_id: null, gateway: 'gw1', state: { name: 'gw1', state: 'REGED', status: 'UP', last_error: null } }], numbers: [number] };
const fixtures = {
  '/api/v1/auth/me': { user: { id: 'u1', username: 'admin', display_name: 'Administrator Mustermann', email: null, role: 'admin', enabled: true, auth_source: 'local', last_login_at: now, totp_enabled: false }, csrf_token: 'fixture' },
  '/api/v1/setup': { needs_setup: false },
  '/api/v1/status': { version: 'test', database: { ok: true }, freeswitch: { ok: true } },
  '/api/v1/telephony/status': { connected: true, registrations: [], gateways: {}, updated_at: now },
  '/api/v1/stats/calls': { range: '1d', buckets: [], total: 0, missed: 0, avg_talk_secs: 0, answer_rate: null, inbound: 0, outbound: 0, internal: 0 },
  '/api/v1/backups': { settings: { last_run_at: null } },
  '/api/v1/settings': { country_code: '49', area_code: '9543', national_prefix: '0', international_prefix: '00', emergency_numbers: ['110', '112'], external_ip: '', default_language: 'de', default_number_id: null, timezone: 'Europe/Berlin', record_inbound: false, record_outbound: false, record_internal: false, recording_announcement: false, recording_retention_days: 30, transcription_enabled: false, transcription_quality: 'fast', transcription_refine: '', sip_uri_dialing: false, transcription_vocabulary: '', hold_music: '', hold_music_clip_id: null },
  '/api/v1/diagnostics': { version: 'test', server_started_at: now, freeswitch: 'UP 0 years, 1 days, 2 hours, 3 minutes', capture: { running: false, level: null, sip_trace: false, started_at: null, until: null, lines: 0, last_seq: 0 }, channels: [{ uuid: 'ch1', created: now, name: `sofia/external/${long}@sip.example.com`, state: 'CS_EXECUTE', callstate: 'ACTIVE', cid_name: long, cid_num: '+4995431241000', dest: '+4995431241000', application: 'bridge', application_data: long, read_codec: 'PCMA', write_codec: 'PCMA', secure: '' }] },
  '/api/v1/diagnostics/log': { lines: [{ seq: 1, text: `INVITE sip:${long}@sip.example.com SIP/2.0` }], capture: { running: false, level: null, sip_trace: false, started_at: null, until: null, lines: 1, last_seq: 1 } },
  '/api/v1/phone-settings': { catalog: [], values: {} },
  '/api/v1/settings/transcription': {
    providers: [{ id: 'p1', name: long, kind: 'openai', url: 'https://api.openai.com/v1', has_key: true }],
    profiles: ['live', 'recording', 'recording_refine', 'voicemail', 'voicemail_refine'].map(purpose => ({
      purpose, engine: purpose === 'live' ? 'cloud' : purpose.endsWith('_refine') ? 'off' : 'local',
      local_model: purpose === 'live' ? 'base' : 'fast', provider_id: purpose === 'live' ? 'p1' : null,
      model: purpose === 'live' ? 'gpt-live-transcribe' : '', realtime: purpose === 'live',
      price_per_minute: null, list_price: purpose === 'live' ? 0.017 : null
    })),
    usage: [{ month: '2026-10-01', purpose: 'live', engine: `${long}/gpt-live-transcribe`, seconds: 600, cost: 0.17, unpriced_seconds: 0 }],
    openai_prices: [{ model: 'gpt-live-transcribe', price_per_minute: 0.017 }]
  },
  '/api/v1/extensions': [ext(1), ext(2), ext(3)],
  '/api/v1/users': [{ id: 'u1', username: 'administrator.mustermann', display_name: long, email: null, role: 'admin', enabled: true, auth_source: 'local', last_login_at: now, totp_enabled: true }],
  '/api/v1/numbers': [number],
  '/api/v1/trunks': [trunk],
  '/api/v1/trunks/t1': trunk,
  '/api/v1/calls': [
    { id: 'c1', call_uuid: 'c1', direction: 'inbound', caller_number: '+4995431241000', caller_name: long, destination: '+4995431241000', extension_id: 'e1', started_at: now, answered_at: now, ended_at: now, duration_secs: 61, billsec: 60, hangup_cause: 'NORMAL_CLEARING', recording_id: null, number_id: 'n1', via_number: '+4995431241000', caller: { name: long, number: '+4995431241000', extension_id: null, contact_id: 'k1', ambiguous: true, matches: [{ contact_id: 'k1', name: long, section_id: null, section_name: '' }, { contact_id: 'k2', name: `${long} 2`, section_id: 's1', section_name: long }] }, called: { name: `${long} 1`, number: '101', extension_id: 'e1', ambiguous: false } },
    { id: 'c2', call_uuid: 'c2', direction: 'outbound', caller_number: '101', caller_name: long, destination: '+4995431241000', extension_id: 'e1', started_at: now, answered_at: null, ended_at: now, duration_secs: 5, billsec: 0, hangup_cause: 'NO_ANSWER', recording_id: null, number_id: 'n1', via_number: '+4995431241000', caller: { name: `${long} 1`, number: '101', extension_id: 'e1', ambiguous: false }, called: { name: '', number: '+4995431241000', extension_id: null, ambiguous: false } }
  ],
  '/api/v1/contacts': [{ id: 'k1', name: long, company: long, phone_work: '+4995431241000', phone_mobile: '+4915112345678', phone_other: '+4995431241999', section_id: null }],
  '/api/v1/audit': [{ id: 1, username: 'administrator.mustermann', action: 'extension.update', entity_type: 'extension', entity_id: 'e1', details: { display_name: long, number: '101' }, ip: '192.168.178.100', created_at: now }]
};
const routes = ['/', '/phone', '/calls', '/voicemail', '/doors', '/phonebook', '/search', '/settings', '/settings#calls', '/settings?section=system&tab=status', '/settings?section=system&tab=diagnostics', '/me', '/extensions', '/numbers', '/routing', '/trunks', '/trunks/t1', '/phones', '/users', '/audit', '/door-stations', '/login', '/setup'];
async function context(options, theme) {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', ...options });
  await context.addInitScript((stored) => {
    localStorage.setItem('talkops.locale', 'de');
    localStorage.setItem('talkops.gettingStarted.hidden', '1');
    if (stored) localStorage.setItem('talkops.theme', stored);
  }, theme);
  await context.route('**/api/**', route => route.fulfill({ status: 200, json: fixtures[new URL(route.request().url()).pathname] ?? [] }));
  return context;
}
const problems = [];
function watch(page) {
  page.on('pageerror', error => problems.push(`${page.url()}: ${error.message}`));
  page.on('console', message => { if (message.type() === 'error' && /Content Security Policy/.test(message.text())) problems.push(`${page.url()}: ${message.text()}`); });
}
try {
  // Theme: the stored choice wins over the OS preference and survives a reload.
  for (const [os, stored, dark] of [['light', 'dark', true], ['dark', 'light', false], ['dark', null, true], ['light', null, false]]) {
    const ctx = await context({ colorScheme: os }, stored);
    const page = await ctx.newPage(); watch(page);
    await page.goto(`${BASE}/login`); await page.getByRole('heading', { level: 1 }).waitFor();
    await page.reload(); await page.getByRole('heading', { level: 1 }).waitFor();
    const state = await page.evaluate(() => ({ dark: document.documentElement.classList.contains('dark'), scheme: getComputedStyle(document.documentElement).colorScheme }));
    assert.deepEqual(state, { dark, scheme: dark ? 'dark' : 'light' }, `OS ${os}, stored ${stored}`);
    await ctx.close();
  }
  // No page may scroll sideways, at any width and in both themes.
  for (const [width, height] of [[360, 780], [768, 1024], [1280, 900]]) for (const theme of ['light', 'dark']) {
    const ctx = await context({ viewport: { width, height } }, theme);
    const page = await ctx.newPage(); watch(page);
    for (const route of routes) {
      await page.goto(BASE + route); await page.locator('main h1').first().waitFor({ timeout: 20_000 });
      await page.waitForTimeout(300);
      const geometry = await page.evaluate(() => ({ width: innerWidth, scroll: document.documentElement.scrollWidth, title: document.querySelector('main h1')?.textContent?.trim(),
        overflow: [...document.querySelectorAll('main *')].filter(el => el.getBoundingClientRect().right > innerWidth + 1 && !el.closest('.scroll-x')).slice(0, 5).map(el => `${el.tagName}.${String(el.className).slice(0, 40)}`) }));
      assert.ok(geometry.scroll <= geometry.width, `${route} @${width} ${theme}: ${JSON.stringify(geometry)}`);
      assert.doesNotMatch(geometry.title ?? '', /schiefgelaufen|nicht gefunden/, `${route} @${width}: error page`);
      if (process.env.SCREENSHOT_DIR && theme === 'dark') { await mkdir(process.env.SCREENSHOT_DIR, { recursive: true }); await page.screenshot({ path: `${process.env.SCREENSHOT_DIR}/layout-${width}${route.replaceAll('/', '_') || '_'}.png`, fullPage: true }); }
    }
    // Below 1024 px the settings menu is a strip above the content, not a second column.
    await page.goto(`${BASE}/extensions`); await page.locator('main h1').waitFor();
    const nav = await page.evaluate(() => { const n = document.querySelector('nav[aria-label="Einstellungen"]'); const c = n?.nextElementSibling?.getBoundingClientRect(); return { direction: n && getComputedStyle(n).flexDirection, content: c?.width, main: document.querySelector('main')?.clientWidth }; });
    assert.equal(nav.direction, width < 1024 ? 'row' : 'column', `settings menu @${width}`);
    if (width < 1024) assert.ok(nav.content >= nav.main - 32, `content width @${width}: ${JSON.stringify(nav)}`);
    await ctx.close();
  }
  // Unknown addresses show the styled error page inside the layout.
  const ctx = await context({ viewport: { width: 360, height: 780 } }, 'dark');
  const page = await ctx.newPage(); watch(page);
  await page.goto(`${BASE}/gibt-es-nicht`);
  await page.getByRole('heading', { name: 'Seite nicht gefunden' }).waitFor({ timeout: 20_000 });
  await page.getByRole('link', { name: 'Zur Übersicht' }).waitFor();
  await ctx.close();
  assert.deepEqual(problems, []);
  console.log('layout browser checks passed');
} finally {
  await browser.close();
}
