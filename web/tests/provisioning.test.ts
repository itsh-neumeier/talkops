import { test } from 'node:test';
import assert from 'node:assert/strict';
import { configurationDiff, keyNeedsAccount, putProvisioningKey } from '../src/lib/provisioning.ts';
import type { ProvisioningKey } from '../src/lib/api.ts';
test('resetting one programmable key preserves other groups and transport overrides', () => {
	const first: ProvisioningKey = {
		group: 'programmable',
		key: 18,
		type: 'menu',
		value: '',
		label: 'Menu',
		account: 0,
		history_type: 0,
		phonebook: 0
	};
	const second = { ...first, group: 'expansion_1' };
	const state = { endpoint: 'http://pbx/provisioning', keys: [first, second] };
	const reset = putProvisioningKey(state, null, 'programmable', 18);
	assert.equal(reset.endpoint, state.endpoint);
	assert.deepEqual(reset.keys, [second]);
	assert.equal(state.keys.length, 2);
});
test('local actions do not require a SIP account', () => {
	assert.equal(keyNeedsAccount('history'), false);
	assert.equal(keyNeedsAccount('menu'), false);
	assert.equal(keyNeedsAccount('speed_dial'), true);
});
test('configuration preview diff shows added and removed parameter values', () => {
	assert.equal(configurationDiff('a = old\nb = same', 'a = new\nb = same'), '- a = old\n+ a = new');
});
