// Network data of provisioned phones (Ubiquiti Networks API) against a
// production preview with API fixtures: the switch in the connection
// settings, the network column of the phone list and its layout.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE?.replace(/index\.mjs$/, 'index.js') || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.BROWSER_EXECUTABLE ? { executablePath: process.env.BROWSER_EXECUTABLE } : {}) });
const BASE = process.env.BASE || 'http://127.0.0.1:5184';
const now = new Date().toISOString();
const phone = (id, mac, name, model) => ({ id, mac, model, name, line_keys: [], last_seen_at: now, last_ip: '10.0.20.31', last_firmware: '96.86.0.70', extensions: [{ number: '101', display_name: 'Empfang' }] });
const client = (extra) => ({ mac: '', wired: false, ip: '10.0.20.31', network: 'Voice', vlan: 20, ap_mac: null, essid: null, radio: null, channel: null, rssi: null, tx_rate_kbps: null, rx_rate_kbps: null, sw_mac: null, sw_port: null, wired_rate_mbps: null, satisfaction: 98, uptime_secs: 7500, tx_bytes: 1024, rx_bytes: 4096, ...extra });
let enabled = false;
let puts = 0;
const connection = () => ({ url: 'https://192.168.1.1', site: 'default', cert_sha256: '', version: '10.0.162', capabilities: { integration: true, classic: true }, last_check_at: now, last_error: null, voice_wifi_id: null, voice_wifi_name: null, write_enabled: false, phone_network: enabled });
const network = () => enabled
  ? { state: 'ok', error: null, read_at: now, phones: [
      { phone_id: 'p1', client: client({ mac: '24:9a:d8:00:00:01', ap_mac: 'f4:92:bf:00:00:02', essid: 'Voice', radio: 'na', channel: 36, rssi: -71, tx_rate_kbps: 144444, rx_rate_kbps: 130000 }), ap_name: 'Verkehrsausscheidungsziffer-Flur-Access-Point', switch_name: null },
      { phone_id: 'p2', client: client({ mac: '80:5e:c0:00:00:02', wired: true, sw_mac: 'd0:21:f9:00:00:09', sw_port: 7, wired_rate_mbps: 1000 }), ap_name: null, switch_name: 'Büro-Switch' }
    ] }
  : { state: 'disabled', error: null, read_at: null, phones: [] };
const problems = [];
try {
  for (const width of [390, 1280]) {
    enabled = false;
    const context = await browser.newContext({ locale: 'de-DE', timezoneId: 'Europe/Berlin', reducedMotion: 'reduce', viewport: { width, height: 900 } });
    await context.addInitScript(() => { localStorage.setItem('talkops.locale', 'de'); localStorage.setItem('talkops.gettingStarted.hidden', '1'); });
    await context.route('**/api/**', async (route) => {
      const req = route.request();
      const path = new URL(req.url()).pathname;
      let value = [];
      if (path === '/api/v1/auth/me') value = { user: { id: 'u1', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture' };
      else if (path === '/api/v1/setup') value = { needs_setup: false };
      else if (path === '/api/v1/phones') value = [phone('p1', '24:9a:d8:00:00:01', 'Flur-Handset', 'ax83h'), phone('p2', '80:5e:c0:00:00:02', 'Empfang', 't54w'), phone('p3', '80:5e:c0:00:00:03', 'Lager', 't54w')];
      else if (path === '/api/v1/phones/network') value = network();
      else if (path === '/api/v1/phone-settings') value = { catalog: [], values: {} };
      else if (path === '/api/v1/provisioning/options') value = { endpoint: 'https://pbx.example/provisioning', sip_host: '192.168.1.10' };
      else if (path === '/api/v1/unifi/connection') value = { connection: connection(), has_api_key: true };
      else if (path === '/api/v1/unifi/phone-network' && req.method() === 'PUT') {
        enabled = JSON.parse(req.postData()).enabled;
        puts += 1;
        value = { connection: connection(), has_api_key: true };
      }
      await route.fulfill({ status: 200, json: value });
    });
    const page = await context.newPage();
    page.on('pageerror', (e) => problems.push(`${width}: ${e.message}`));

    await page.goto(`${BASE}/phones`);
    await page.getByRole('link', { name: 'Flur-Handset' }).waitFor();
    assert.equal(await page.locator('th', { hasText: 'Netzwerk' }).count(), 0, 'no column while off');

    await page.goto(`${BASE}/wifi-surveys`);
    await page.getByRole('button', { name: 'Ubiquiti Networks API' }).or(page.getByRole('tab', { name: 'Ubiquiti Networks API' })).first().click();
    const box = page.getByRole('checkbox', { name: /Netzwerkdaten bei Telefonen anzeigen/ });
    await box.check();
    await page.waitForFunction(() => document.body.textContent.includes('Gespeichert'));
    assert.ok(enabled, 'switch sends enabled');

    await page.goto(`${BASE}/phones`);
    await page.locator('th', { hasText: 'Netzwerk' }).waitFor({ state: 'attached' });
    const cells = page.getByTestId('phone-network-cell');
    assert.equal(await cells.count(), 3);
    assert.match(await cells.nth(0).textContent(), /-71 dBm/);
    assert.match(await cells.nth(0).textContent(), /5 GHz · Kanal 36/);
    assert.match(await cells.nth(1).textContent(), /Büro-Switch/);
    assert.match(await cells.nth(1).textContent(), /Port 7 · 1 Gbit\/s/);
    assert.match(await cells.nth(2).textContent(), /nicht verbunden/);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    assert.ok(overflow <= 0, `${width}: page scrolls sideways by ${overflow}px`);
    await mkdir('test-results', { recursive: true });
    await page.screenshot({ path: `test-results/phone-network-${width}.png`, fullPage: true });
    await context.close();
  }
  assert.equal(puts, 2);
  assert.deepEqual(problems, []);
  console.log('phone network browser checks passed');
} finally {
  await browser.close();
}
