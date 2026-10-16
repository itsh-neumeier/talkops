import assert from 'node:assert/strict';
import test from 'node:test';
import { numberCountry } from '../src/lib/calls/country.ts';

test('international formats identify the country without changing the display value', () => {
	for (const number of ['+4995436999124', '0049 9543 6999124', '+49 (9543) 699-9124'])
		assert.equal(numberCountry(number), 'DE');
	assert.equal(numberCountry('+33142345678'), 'FR');
	assert.equal(numberCountry('+442079460018'), 'GB');
	assert.equal(numberCountry('+436641234567'), 'AT');
	assert.equal(numberCountry('+41446681800'), 'CH');
});

test('shared calling codes resolve the actual country', () => {
	assert.equal(numberCountry('+14155552671'), 'US');
	assert.equal(numberCountry('+14165551234'), 'CA');
	assert.equal(numberCountry('+12424612345'), 'BS');
	assert.equal(numberCountry('+18005551234'), null, 'NANP toll-free is shared');
	assert.equal(numberCountry('+447700900123'), null, 'do not guess an unresolved country');
});

test('extensions, hidden callers, services and malformed numbers have no country flag', () => {
	assert.equal(numberCountry('+4995436999124', true), null, 'extension metadata wins');
	for (const number of [
		'',
		'anonymous',
		'restricted',
		'101',
		'**101',
		'*97',
		'112',
		'+49112',
		'095436999124',
		'+80012345678',
		'+498001234567',
		'+99912345678',
		'+49abc95436999124',
		'sip:+4995436999124@example.org',
		'Call +4995436999124',
		'+49'.repeat(100),
		'+4995436999124 ext. 1'
	])
		assert.equal(numberCountry(number), null, number);
});
