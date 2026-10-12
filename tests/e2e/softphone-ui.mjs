// Browser behavior with a deterministic HTTP and SIP fixture. Real media is
// checked separately by webrtc.mjs against FreeSWITCH and SIPp.
import assert from 'node:assert/strict';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE ?? 'playwright');
const base = process.argv[2] ?? 'http://127.0.0.1:5178';
const browser = await chromium.launch({
	executablePath: process.env.CHROMIUM_PATH || undefined,
	args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream']
});
const identities = [
	{ extension_id: 'ext-a', extension_number: '200', display_name: 'Office', outbound_number: '+4989123', uses_default_number: false, hide_caller_id: false },
	{ extension_id: 'ext-b', extension_number: '201', display_name: 'Lab', outbound_number: '+4989456', uses_default_number: true, hide_caller_id: false }
];

async function scenario(role, choices) {
	const context = await browser.newContext({ locale: 'en-US', permissions: ['microphone', 'camera'] });
	const page = await context.newPage();
	const errors = [];
	page.on('pageerror', error => errors.push(error.message));
	const accounts = [];
	let registrations = 0;
	let unregistrations = 0;
	let sockets = 0;
	let activeSocket;
	let reconnectComplete;
	await page.addInitScript(() => {
		localStorage.setItem('talkops.locale', 'en');
		window.testNotifications = [];
		window.testSounds = [];
		const originalPlay = HTMLMediaElement.prototype.play;
		HTMLMediaElement.prototype.play = function () {
			const source = this.src;
			return originalPlay.call(this).then(() => {
				if (source.startsWith('blob:')) window.testSounds.push(source);
			});
		};
		window.Notification = class {
			static permission = 'granted';
			static async requestPermission() { return 'granted'; }
			constructor(title, options) { window.testNotifications.push({ title, ...options }); }
			close() {}
		};
	});
	await page.route('**/api/v1/**', async route => {
		const path = new URL(route.request().url()).pathname.replace('/api/v1', '');
		let data = [];
		if (path === '/auth/me') data = { user: { id: `${role}-${choices.length}`, display_name: 'Tester', role }, csrf_token: 'fixture' };
		if (path === '/me/webrtc/extensions') data = choices;
		if (path === '/me/webrtc') {
			const { extension_id: id } = route.request().postDataJSON();
			accounts.push(id);
			const choice = choices.find(c => c.extension_id === id);
			assert.ok(choice, 'only offered identities can be selected');
			data = { ...choice, sip_username: `browser-${id}`, sip_password: 'fixture', sip_domain: 'talkops.local', ws_path: '/api/v1/webrtc/ws', ice_servers: [], relay_only: false, video_enabled: false };
		}
		if (path === '/contacts') data = [{ id: 'contact', name: 'Alice', company: 'Office', phone_work: '210', phone_mobile: '+49123', phone_other: '', section_id: null }];
		if (path === '/me/webrtc/calls') data = [
			{ id: 'incoming', extension_id: null, caller_number: '+49999', destination: '200', direction: 'inbound', answered_at: null, started_at: '2026-10-12T12:00:00Z' },
			{ id: 'outgoing', extension_id: new URL(route.request().url()).searchParams.get('extension_id'), caller_number: '200', destination: '+49888', direction: 'outbound', answered_at: null, started_at: '2026-10-12T11:00:00Z' }
		];
		await route.fulfill({ json: data });
	});
	await page.routeWebSocket('**/api/v1/webrtc/ws', ws => {
		sockets++;
		activeSocket = ws;
		ws.onMessage(message => {
			const text = message.toString();
			if (!text.startsWith('REGISTER ')) return;
			const header = name => text.split('\r\n').find(line => line.toLowerCase().startsWith(`${name.toLowerCase()}:`));
			if (header('Expires')?.includes(' 0') || header('Contact')?.includes('expires=0')) unregistrations++;
			else registrations++;
			ws.send(['SIP/2.0 200 OK', header('Via'), header('From'), `${header('To')};tag=fixture`, header('Call-ID'), header('CSeq'), header('Contact'), 'Content-Length: 0', '', ''].filter(v => v !== undefined).join('\r\n'));
			reconnectComplete?.();
		});
	});
	await page.goto(`${base}/phone`);
	await page.getByRole('heading', { name: 'Softphone', exact: true }).waitFor();
	if (role === 'admin') {
		await page.getByLabel('Call identity').waitFor();
		assert.equal(accounts.length, 0, 'admin must choose explicitly');
		await page.getByLabel('Call identity').selectOption('ext-a');
	} else if (choices.length === 0) {
		await page.getByText('No active extension is available for this account.', { exact: true }).waitFor();
		assert.equal(accounts.length, 0);
		await context.close();
		return;
	}
	await page.locator('.badge').filter({ hasText: /^ready$/ }).waitFor();
	assert.equal(accounts[0], 'ext-a');
	await page.getByRole('button', { name: 'Favorite: Alice' }).click();
	await page.getByRole('button', { name: '210', exact: true }).click();
	assert.equal(await page.getByLabel('Number', { exact: true }).inputValue(), '210');
	await page.getByLabel('Filter calls').selectOption('missed');
	assert.equal(await page.getByRole('region', { name: 'Recent calls' }).getByRole('button', { name: '+49999', exact: true }).count(), 1);
	await page.getByRole('region', { name: 'Recent calls' }).getByRole('button', { name: '+49999', exact: true }).click();
	assert.equal(await page.getByLabel('Number', { exact: true }).inputValue(), '+49999');
	await page.getByText('Audio devices and tests', { exact: true }).click();
	await page.getByRole('button', { name: 'Test microphone', exact: true }).click();
	await page.getByLabel('Microphone level').waitFor();
	await page.getByRole('button', { name: 'Stop test' }).click();
	const soundCount = await page.evaluate(() => window.testSounds.length);
	await page.getByRole('button', { name: 'Test sound', exact: true }).click();
	await page.waitForFunction(count => window.testSounds.length > count, soundCount);
	assert.equal(await page.getByRole('alert').count(), 0, 'audio tests must succeed without an error');
	await page.getByRole('button', { name: 'Enable call notifications', exact: true }).click();
	await page.getByRole('button', { name: 'Disable call notifications', exact: true }).waitFor();
	if (choices.length > 1) {
		await page.getByLabel('Call identity').selectOption('ext-b');
		await page.locator('.badge').filter({ hasText: /^ready$/ }).waitFor();
		assert.equal(accounts.at(-1), 'ext-b');
		assert.ok(unregistrations >= 1, 'old identity must unregister');
	} else assert.equal(await page.locator('#phone-identity').count(), 0, 'one user identity is fixed');
	if (process.env.SOFTPHONE_SCREENSHOT && role === 'admin') { await page.evaluate(() => window.scrollTo(0, 0)); await page.screenshot({ path: process.env.SOFTPHONE_SCREENSHOT, fullPage: true }); }
	const beforeNavigation = sockets;
	await page.getByRole('link', { name: 'Phonebook', exact: true }).click();
	await page.waitForURL(`${base}/phonebook`);
	await page.getByRole('complementary', { name: 'Softphone' }).waitFor();
	assert.equal(sockets, beforeNavigation, 'navigation must preserve the SIP connection');
	const selected = accounts.at(-1);
	const soundBeforeIncoming = await page.evaluate(() => window.testSounds.length);
	activeSocket.send([
		`INVITE sip:browser-${selected}@talkops.local SIP/2.0`,
		'Via: SIP/2.0/WS fixture.invalid;branch=z9hG4bKfixture',
		'From: "Caller" <sip:555@talkops.local>;tag=incoming',
		`To: <sip:browser-${selected}@talkops.local>`,
		'Call-ID: incoming-fixture', 'CSeq: 1 INVITE',
		'Contact: <sip:555@fixture.invalid;transport=ws>',
		'Content-Length: 0', '', ''
	].join('\r\n'));
	const compact = page.getByRole('complementary', { name: 'Softphone' });
	await compact.getByRole('button', { name: 'Decline', exact: true }).waitFor();
	await page.waitForFunction(() => window.testNotifications.length === 1);
	await page.waitForFunction(count => window.testSounds.length > count, soundBeforeIncoming);
	if (choices.length > 1 || role === 'admin') assert.ok(await page.locator('#phone-identity').isDisabled());
	await compact.getByRole('button', { name: 'Decline', exact: true }).click();
	await compact.locator('.badge').filter({ hasText: /^ready$/ }).waitFor();
	await compact.getByRole('link').click();
	await page.waitForURL(`${base}/phone`);
	assert.equal(await page.getByRole('button', { name: 'Favorite: Alice' }).getAttribute('aria-pressed'), 'true');
	assert.equal(sockets, beforeNavigation);
	assert.ok(registrations >= 1);
	const previousRegistrations = registrations;
	const reconnected = new Promise(resolve => { reconnectComplete = resolve; });
	activeSocket.close({ code: 1001, reason: 'fixture reconnect' });
	let timeout;
	try {
		await Promise.race([reconnected, new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error('SIP reconnection timed out')), 20000); })]);
	} finally { clearTimeout(timeout); reconnectComplete = undefined; }
	await page.locator('.badge').filter({ hasText: /^ready$/ }).waitFor();
	assert.ok(registrations > previousRegistrations, 'lost transport must register again');
	assert.deepEqual(errors, []);
	await page.getByRole('button', { name: 'Log out', exact: true }).click();
	await page.waitForURL(`${base}/login`);
	await context.close();
}

try {
	await scenario('admin', identities);
	await scenario('user', identities.slice(0, 1));
	await scenario('user', identities);
	await scenario('operator', []);
	console.log('softphone UI ok: identity selection, audio tests, favorites, missed calls, navigation, incoming notification and logout');
} finally {
	await browser.close();
}
