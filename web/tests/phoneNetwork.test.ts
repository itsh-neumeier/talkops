import assert from 'node:assert/strict';
import test from 'node:test';
import {
	byPhone,
	formatBytes,
	formatUptime,
	linkSpeed,
	signalBars,
	signalClass,
	type PhoneNetworkView
} from '../src/lib/phoneNetwork.ts';

test('signal bars and colors follow the voice thresholds', () => {
	assert.equal(signalBars(null), 0);
	assert.equal(signalBars(-55), 4);
	assert.equal(signalBars(-65), 3);
	assert.equal(signalBars(-70), 2);
	assert.equal(signalBars(-82), 1);
	assert.match(signalClass(-60), /emerald/);
	assert.match(signalClass(-72), /amber/);
	assert.match(signalClass(-80), /red/);
	assert.match(signalClass(undefined), /slate/);
});

test('formats link speed, bytes and uptime', () => {
	assert.equal(linkSpeed(1000), '1 Gbit/s');
	assert.equal(linkSpeed(2500), '2500 Mbit/s');
	assert.equal(linkSpeed(100), '100 Mbit/s');
	assert.equal(linkSpeed(null), '—');
	assert.equal(formatBytes(512), '512 B');
	assert.equal(formatBytes(1536), '1.5 KB');
	assert.equal(formatBytes(3 * 1024 ** 3), '3.0 GB');
	assert.equal(formatUptime(45 * 60), '45 min');
	assert.equal(formatUptime(2 * 3600 + 300), '2 h 5 min');
	assert.equal(formatUptime(3 * 86400 + 4 * 3600), '3 d 4 h');
	assert.equal(formatUptime(null), '—');
});

test('maps the network data by phone', () => {
	const view = {
		state: 'ok',
		error: null,
		read_at: null,
		phones: [{ phone_id: 'a', client: { mac: 'x' }, ap_name: null, switch_name: null }]
	} as unknown as PhoneNetworkView;
	assert.equal(byPhone(view).get('a')?.client.mac, 'x');
	assert.equal(byPhone(null).size, 0);
});
