import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parameterCategory, visibleParameters } from '../src/lib/phoneParameters.ts';
import type { PhoneParameter } from '../src/lib/api.ts';
test('manufacturer search includes descriptions and preserves explicit empty overrides', () => {
	const field: PhoneParameter = {
		key: 'x',
		parameter: 'P1362',
		description: 'Display language',
		artifact: 'phone.xml',
		category: 'locale',
		kind: 'text',
		managed: false,
		copyable: true,
		default: 'en'
	};
	assert.deepEqual(visibleParameters([field], ' LANGUAGE ', false), [field]);
	assert.deepEqual(visibleParameters([field], '', true, { x: '' }), [field]);
	assert.deepEqual(visibleParameters([field], '', true, {}), []);
	assert.deepEqual(visibleParameters([field], 'other', false), []);
	assert.equal(parameterCategory('snom_admin_password'), 'security');
	assert.equal(parameterCategory('yealink_time_zone'), 'locale');
	assert.equal(parameterCategory('grandstream_vlan_id'), 'network');
});
