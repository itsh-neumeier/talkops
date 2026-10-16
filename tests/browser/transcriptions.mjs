// Browser checks of the transcription page and the "transcribe again"
// dialog against a production preview with API fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const at = '2026-10-15T09:02:00Z';
const job = (id, status, extra) => ({ id, status, priority: 0, progress: null, pass: 1, engine: null, engine_key: 'local:fast', recording_id: 'rec-1', voicemail_id: null, audio_secs: 125, caller_number: '+4995436999124', destination: '101', call_started_at: at, requested_by: null, attempts: 1, last_error: null, control: null, created_at: at, started_at: null, run_started_at: null, finished_at: null, eta_secs: null, resumable: false, ...extra });
try {
  const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  page.on('dialog', d => d.accept());
  const actions = [];
  let started = null;
  let jobs = [
    job('j-run', 'running', { progress: 0.46, eta_secs: 170, run_started_at: at, started_at: at }),
    job('j-paused', 'paused', { progress: 0.3, resumable: true, engine: { kind: 'local', model: 'accurate' }, engine_key: 'local:accurate', requested_by: 'Admin' }),
    job('j-queued', 'queued', { eta_secs: 400, pass: 2 })
  ];
  const maria = { name: 'Familie Vollmeier (Maria)', number: '+4995436999124', extension_id: null, contact_id: 'c-maria', ambiguous: false, matches: [] };
  const zweig = { name: 'Familie Zweig', number: '101', extension_id: 'e-101', contact_id: null, ambiguous: false, matches: [] };
  const bob = { name: 'Bob', number: '102', extension_id: 'e-102', contact_id: null, ambiguous: false, matches: [] };
  const history = [
    job('j-fail', 'failed', { direction: 'outbound', engine_key: 'local:accurate', caller_number: '102', destination: '+499112193553', started_at: '2026-10-14T17:59:30Z', finished_at: '2026-10-14T18:00:00Z', created_at: '2026-10-14T17:59:00Z', last_error: 'boom', caller: bob, called: { name: '', number: '+499112193553', extension_id: null, contact_id: null, ambiguous: false, matches: [] } }),
    job('j-old', 'done', { direction: 'inbound', progress: 1, started_at: at, finished_at: '2026-10-15T09:05:10Z', caller: maria, called: zweig, via_number: '+4995431241' }),
    job('j-vm', 'cancelled', { direction: 'internal', recording_id: null, voicemail_id: 'vm-1', caller_number: '102', caller_name: 'Bob', destination: '101', created_at: '2026-10-15T12:00:00Z', caller: bob, called: zweig })
  ];
  let historyRequests = 0;
  const me = { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', role_id: null, enabled: true }, csrf_token: 'x', permissions: ['transcriptions.view', 'transcriptions.manage', 'recordings.retranscribe', 'recordings.delete'] };
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    let value = [];
    const headers = {};
    if (url.pathname === '/api/v1/auth/me') value = me;
    else if (url.pathname === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
    else if (url.pathname === '/api/v1/transcription-jobs') {
      if (url.searchParams.get('history')) historyRequests++;
      value = url.searchParams.get('history') ? history : jobs;
    }
    else if (url.pathname.startsWith('/api/v1/transcription-jobs/') && method === 'POST') {
      const action = route.request().postDataJSON().action;
      actions.push([url.pathname.split('/').pop(), action]);
      if (action === 'resume') jobs = jobs.map(j => j.id === 'j-paused' ? { ...j, status: 'queued', eta_secs: 600 } : j);
      await route.fulfill({ status: 204 }); return;
    }
    else if (url.pathname === '/api/v1/transcription-workers') value = [{ worker_id: 'media-1', cpu_percent: 312, cpus: 4, load1: 3.4, rss_bytes: 2.1 * 1024 ** 3, mem_used_bytes: 5 * 1024 ** 3, mem_total_bytes: 8 * 1024 ** 3, job_id: 'j-run', updated_at: at }];
    else if (url.pathname === '/api/v1/calls') { value = [{ id: 'c1', call_uuid: 'c1', direction: 'inbound', caller_number: '+4995436999124', caller_name: '', destination: '101', extension_id: null, started_at: at, answered_at: at, ended_at: at, duration_secs: 125, billsec: 125, hangup_cause: 'NORMAL_CLEARING', recording_id: 'rec-1' }]; headers['X-Total-Count'] = '1'; }
    else if (url.pathname === '/api/v1/recordings/rec-1') value = { id: 'rec-1', call_uuid: 'c1', cdr_id: null, duration_secs: 125, size_bytes: 1, transcript_status: 'done', created_at: at };
    else if (url.pathname === '/api/v1/recordings/rec-1/transcript') value = { id: 't', recording_id: 'rec-1', voicemail_id: null, language: 'de', text: 'Hallo', segments: [{ start: 1, end: 2, speaker: 'caller', text: 'Hallo' }], engine: 'whisper:large-v3-turbo-q5_0', is_final: true, created_at: at };
    else if (url.pathname === '/api/v1/recordings/rec-1/transcribe' && method === 'GET') value = { audio_secs: 125, queue_secs: 570, current_engine: 'whisper:large-v3-turbo-q5_0', options: [
      { choice: { kind: 'local', model: 'fast' }, provider: '', eta_secs: 125, measured: true, cost_usd: null, default: true },
      { choice: { kind: 'local', model: 'accurate' }, provider: '', eta_secs: 375, measured: false, cost_usd: null, default: false },
      { choice: { kind: 'cloud', provider_id: 'p1', model: 'gpt-transcribe' }, provider: 'OpenAI', eta_secs: 19, measured: false, cost_usd: 0.009, default: false }
    ] };
    else if (url.pathname === '/api/v1/recordings/rec-1/transcribe') { started = route.request().postDataJSON(); value = { job_id: 'j-new' }; }
    await route.fulfill({ status: 200, json: value, headers });
  });

  // Menu entry and page.
  await page.goto(`${BASE}/transcriptions`);
  await page.getByRole('link', { name: 'Transkriptionen' }).waitFor({ timeout: 20_000 });
  const cards = page.locator('main ul > li');
  await cards.first().waitFor();
  assert.equal(await cards.count(), 3);
  assert.match(await cards.nth(0).textContent(), /läuft[\s\S]*46 %[\s\S]*noch ca\. 3 min/);
  assert.equal(await cards.nth(0).getByRole('progressbar').getAttribute('aria-valuenow'), '46');
  assert.match(await cards.nth(1).textContent(), /pausiert[\s\S]*setzt an der Pausenstelle fort/);
  assert.match(await cards.nth(1).textContent(), /manuell[\s\S]*von Admin/);
  assert.match(await cards.nth(2).textContent(), /wartet[\s\S]*2\. Durchgang[\s\S]*fertig in ca\. 7 min/);
  // Resources of the worker.
  assert.match(await page.locator('main').textContent(), /media-1[\s\S]*transkribiert[\s\S]*312 % · 4 Kerne/);
  assert.equal(await page.getByRole('meter', { name: 'CPU' }).getAttribute('aria-valuenow'), '78');
  // Actions.
  await cards.nth(1).getByRole('button', { name: /Fortsetzen/ }).click();
  await page.waitForFunction(() => document.querySelectorAll('main ul > li')[1]?.textContent?.includes('wartet'));
  await cards.nth(0).getByRole('button', { name: /Pausieren/ }).click();
  await cards.nth(2).getByRole('button', { name: /Stoppen/ }).click();
  for (let i = 0; i < 50 && actions.length < 3; i++) await page.waitForTimeout(100);
  assert.deepEqual(actions, [['j-paused', 'resume'], ['j-run', 'pause'], ['j-queued', 'stop']]);
  // History.
  await page.getByRole('tab', { name: /Verlauf/ }).click();
  await page.getByRole('cell', { name: 'fertig' }).waitFor();
  await page.waitForFunction(() => location.search === '?tab=history');
  const rows = page.locator('table tbody tr');
  const ids = async () => (await rows.allInnerTexts()).map(r => r.includes('Voicemail') ? 'vm' : r.includes('fehlgeschlagen') ? 'fail' : r.includes('fertig') ? 'old' : '?');
  assert.deepEqual(await ids(), ['vm', 'old', 'fail'], 'newest first');
  assert.match(await rows.nth(1).textContent(), /3:10/);
  // Parties through the phone book, like the call log.
  const old = rows.nth(1);
  assert.equal(await old.getByRole('link', { name: 'Familie Vollmeier (Maria)' }).getAttribute('href'), '/phonebook?contact=c-maria');
  assert.match(await old.textContent(), /Familie Zweig\s*\[101\][\s\S]*über\s*\+4995431241/);
  // Sort by duration (descending; jobs that did not run last), then by status.
  await page.getByRole('button', { name: 'Dauer', exact: true }).click();
  await page.waitForFunction(() => location.search.includes('sort=duration'));
  assert.deepEqual(await ids(), ['old', 'fail', 'vm']);
  await page.getByRole('button', { name: 'Ergebnis', exact: true }).click();
  assert.deepEqual(await ids(), ['vm', 'fail', 'old']);
  await page.getByRole('button', { name: 'Ergebnis', exact: true }).click();
  await page.waitForFunction(() => location.search.includes('order=asc'));
  assert.deepEqual(await ids(), ['old', 'fail', 'vm']);
  assert.equal(await page.locator('th[aria-sort="ascending"]').count(), 1);
  // Column filters: status, model, direction; the address keeps them.
  await page.getByRole('button', { name: 'Ergebnis filtern', exact: true }).click();
  await page.getByRole('dialog', { name: 'Filter: Ergebnis' }).getByLabel('fehlgeschlagen').check();
  await page.keyboard.press('Escape');
  assert.deepEqual(await ids(), ['fail']);
  await page.waitForFunction(() => location.search.includes('status=failed'));
  await page.reload();
  await page.getByText('1 von 3 Transkriptionen').waitFor({ timeout: 20_000 });
  await page.getByRole('button', { name: 'Alle Filter zurücksetzen' }).first().click();
  await page.getByText('3 von 3 Transkriptionen').waitFor();
  await page.getByRole('button', { name: 'Modell filtern', exact: true }).click();
  const models = page.getByRole('dialog', { name: 'Filter: Modell' });
  assert.equal(await models.getByRole('checkbox').count(), 2, 'models of the history');
  await models.getByRole('checkbox').first().check();
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => location.search.includes('engine=local'));
  await page.getByRole('button', { name: 'Alle Filter zurücksetzen' }).first().click();
  await page.getByRole('button', { name: 'Richtung filtern', exact: true }).click();
  await page.getByRole('dialog', { name: 'Filter: Richtung' }).getByLabel('intern').check();
  await page.keyboard.press('Escape');
  assert.deepEqual(await ids(), ['vm']);
  await page.getByRole('button', { name: 'Alle Filter zurücksetzen' }).first().click();
  // Free text on the parties (phone book name or digits).
  await page.getByRole('searchbox', { name: 'Suchen' }).fill('vollmeier');
  assert.deepEqual(await ids(), ['old']);
  await page.getByRole('searchbox', { name: 'Suchen' }).fill('0911 2193');
  assert.deepEqual(await ids(), ['fail']);
  // A date range from a link.
  await page.goto(`${BASE}/transcriptions?tab=history&since=2026-10-14&until=2026-10-14`);
  await page.getByText('1 von 3 Transkriptionen').waitFor({ timeout: 20_000 });
  assert.deepEqual(await ids(), ['fail']);
  assert.equal(await page.getByRole('button', { name: 'Zeit filtern (aktiv)' }).count(), 1);
  // The history reloads at most every 30 s (the active list every 2 s).
  const before = historyRequests;
  await page.waitForTimeout(4500);
  assert.ok(historyRequests - before <= 1, `history requests: ${historyRequests - before}`);

  // The dialog in the call log.
  await page.goto(`${BASE}/calls`);
  await page.getByRole('button', { name: 'Aufnahme', exact: true }).click();
  await page.getByRole('button', { name: 'Erneut transkribieren …' }).click();
  const dialog = page.getByRole('dialog', { name: 'Erneut transkribieren' });
  await dialog.getByText('Aktuelles Transkript: whisper:large-v3-turbo-q5_0').waitFor();
  assert.equal(await dialog.getByRole('radio').first().isChecked(), true, 'the default model');
  assert.match(await dialog.textContent(), /Standard[\s\S]*≈ 2 min \(gemessen\)/);
  assert.match(await dialog.textContent(), /OpenAI · gpt-transcribe[\s\S]*≈ 19 s \(geschätzt\)[\s\S]*ca\. \$0\.009/);
  assert.match(await dialog.textContent(), /Vorher in der Warteschlange: ca\. 10 min/);
  await dialog.getByRole('radio', { name: /Genau/ }).check();
  await dialog.getByRole('button', { name: 'Starten' }).click();
  await dialog.getByText('Transkription gestartet.').waitFor();
  assert.deepEqual(started, { engine: { kind: 'local', model: 'accurate' } });
  assert.equal(await dialog.getByRole('link', { name: 'Fortschritt ansehen' }).getAttribute('href'), '/transcriptions');

  // Narrow screen.
  await page.goto(`${BASE}/transcriptions`);
  await cards.first().waitFor();
  await page.setViewportSize({ width: 360, height: 780 });
  const width = await page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
  assert.ok(width[0] <= width[1], JSON.stringify(width));
  assert.deepEqual(errors, []);
  console.log('transcription page browser checks passed');
} finally {
  await browser.close();
}
