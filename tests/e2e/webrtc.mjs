// Browser softphone end-to-end: logs in, registers over WebSocket (WebRTC)
// and calls an extension answered by SIPp. Needs Playwright with Chromium.
//
//   node tests/e2e/webrtc.mjs http://127.0.0.1:8080 <user> <password> <number>
// PLAYWRIGHT_MODULE / CHROMIUM_PATH point to a non-standard installation.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE ?? 'playwright');

const [base, user, password, number] = process.argv.slice(2);
const browser = await chromium.launch({
	executablePath: process.env.CHROMIUM_PATH || undefined,
	args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream']
});
const context = await browser.newContext({
	locale: 'en-US',
	permissions: ['microphone', 'camera']
});
const page = await context.newPage();
const fail = async (message) => {
	console.error(`FAIL: ${message}`);
	await browser.close();
	process.exit(1);
};
await page.goto(`${base}/login`);
await page.fill('#username', user);
await page.fill('#password', password);
await page.click('button:has-text("Sign in")');
await page.waitForURL(`${base}/`).catch(() => fail('login'));
await page.goto(`${base}/phone`);
await page
	.waitForSelector('.badge:has-text("ready")', { timeout: 20000 })
	.catch(() => fail('softphone did not register'));
await page.fill('input[aria-label="Number"]', number);
await page.click('button:has-text("Call")');
await page
	.waitForSelector('.badge:has-text("in call")', { timeout: 20000 })
	.catch(() => fail('call was not answered'));
// Media flows: the remote stream carries a live audio track.
await page.waitForTimeout(2000);
const tracks = await page.evaluate(() =>
	[...document.querySelectorAll('audio, video')]
		.map((el) => el.srcObject)
		.filter(Boolean)
		.flatMap((s) => s.getTracks().map((t) => `${t.kind}:${t.readyState}`))
);
if (!tracks.includes('audio:live')) await fail(`no remote audio (${tracks})`);
await page.click('button:has-text("Hang up")');
await page
	.waitForSelector('.badge:has-text("ready")', { timeout: 10000 })
	.catch(() => fail('hang up'));
console.log(`softphone call ok (${tracks.join(', ')})`);
await browser.close();
