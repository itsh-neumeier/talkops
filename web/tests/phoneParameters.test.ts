import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
	artifactFileName,
	parameterCategory,
	parameterLabel,
	visibleParameters
} from '../src/lib/phoneParameters.ts';
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
	assert.equal(parameterCategory('cisco_enterprise_tftp_ip'), 'provisioning');
});
test('XML path parameters get readable labels and real file names', () => {
	assert.equal(
		parameterLabel('device[0]/devicePool[0]/dateTimeSetting[0]/dateTemplate[0]'),
		'dateTimeSetting › dateTemplate'
	);
	assert.equal(parameterLabel('device[0]/sipProfile[0]/sipLines[0]/line[1]'), 'sipLines › line 1');
	assert.equal(parameterLabel('P1362'), 'P1362');
	assert.equal(
		artifactFileName('SEP{$mac|upper}.cnf.xml', '00451d6be327'),
		'SEP00451D6BE327.cnf.xml'
	);
	assert.equal(artifactFileName('dialplan.xml', '00451d6be327'), 'dialplan.xml');
	assert.equal(artifactFileName('SEP{$mac|upper}.cnf.xml'), 'SEP{$mac|upper}.cnf.xml');
});
