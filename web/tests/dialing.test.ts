import assert from 'node:assert/strict';
import test from 'node:test';
import { blockSource, isBlocked } from '../src/lib/calls/blocking.ts';
import {
	countryOptions,
	missingPremium,
	normalizeValue,
	profileInput,
	reasonParts,
	sortRules,
	upsertRule,
	type DialingRule
} from '../src/lib/dialing.ts';

const rule = (kind: DialingRule['kind'], value: string, action: DialingRule['action'] = 'deny') =>
	({ kind, value, action, label: '' }) as DialingRule;

test('rule values are normalized like the API does', () => {
	assert.equal(normalizeValue('country', ' de '), 'DE');
	assert.equal(normalizeValue('country', 'DEU'), null);
	assert.equal(normalizeValue('prefix', '+49 900*'), '+49900');
	assert.equal(normalizeValue('prefix', '0900'), null);
	assert.equal(normalizeValue('prefix', '+'), null);
	assert.equal(normalizeValue('number', '+49 (30) 123-456'), '+4930123456');
	assert.equal(normalizeValue('number', '+0301234'), null);
	assert.equal(normalizeValue('number', '+49900*'), null);
});

test('rules are listed in the order of their effect', () => {
	const sorted = sortRules([
		rule('country', 'DE', 'allow'),
		rule('prefix', '+49'),
		rule('number', '+499001234567', 'allow'),
		rule('prefix', '+49900')
	]);
	assert.deepEqual(
		sorted.map((r) => r.value),
		['+499001234567', '+49900', '+49', 'DE']
	);
	// The same kind and value replaces the older rule.
	const replaced = upsertRule(sorted, rule('prefix', '+49900', 'allow'));
	assert.equal(replaced.length, 4);
	assert.equal(replaced.find((r) => r.value === '+49900')?.action, 'allow');
});

test('premium blocks are added once', () => {
	const premium = [
		{ prefix: '+49900', label: 'DE 0900', source: 'BNetzA' },
		{ prefix: '+49137', label: 'DE 0137', source: 'BNetzA' }
	];
	const missing = missingPremium([rule('prefix', '+49900', 'allow')], premium);
	assert.deepEqual(missing, [
		{ kind: 'prefix', value: '+49137', action: 'deny', label: 'DE 0137' }
	]);
	assert.deepEqual(missingPremium([...missing, rule('prefix', '+49900')], premium), []);
});

test('block reasons of dialing profiles are explained', () => {
	assert.deepEqual(reasonParts('dialing:base'), { kind: 'base' });
	assert.deepEqual(reasonParts('dialing:sip'), { kind: 'sip' });
	assert.deepEqual(reasonParts('dialing:prefix:+49900'), { kind: 'prefix', value: '+49900' });
	assert.deepEqual(reasonParts('dialing:country:AT'), { kind: 'country', value: 'AT' });
	assert.equal(reasonParts('dialing:country:at'), null);
	assert.equal(reasonParts('list:+49900*'), null);

	assert.equal(blockSource('dialing:base'), 'dialing');
	assert.equal(isBlocked({ direction: 'outbound', blocked_reason: 'dialing:prefix:+49900' }), true);
	assert.equal(isBlocked({ direction: 'outbound', blocked_reason: 'list:+4930*' }), false);
	assert.equal(isBlocked({ direction: 'internal', blocked_reason: 'dialing:base' }), false);
	assert.equal(isBlocked({ direction: 'outbound', blocked_reason: null }), false);
});

test('countries are offered by their local name', () => {
	const de = countryOptions('de');
	assert.ok(de.length > 200);
	assert.equal(de.find((c) => c.code === 'AT')?.name, 'Österreich');
	const names = de.map((c) => c.name);
	assert.deepEqual(
		names,
		[...names].sort((a, b) => a.localeCompare(b, 'de'))
	);
});

test('the request body has only the editable fields', () => {
	const body = profileInput({
		name: ' Night ',
		description: ' After hours ',
		base_mode: 'deny',
		sip_addresses: false,
		rules: [{ id: 'x', kind: 'country', value: 'DE', action: 'allow', label: 'Germany' }]
	});
	assert.deepEqual(body, {
		name: 'Night',
		description: 'After hours',
		base_mode: 'deny',
		sip_addresses: false,
		rules: [{ kind: 'country', value: 'DE', action: 'allow', label: 'Germany' }]
	});
});
