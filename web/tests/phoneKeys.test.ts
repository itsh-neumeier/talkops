import assert from 'node:assert/strict';
import test from 'node:test';
import type { Contact, LineKey, PhoneAccount, PhoneModel } from '../src/lib/api.ts';
import {
	contactNumbers,
	contactsForPhone,
	effectivePhoneKey,
	keyValidation,
	phoneKeyPages,
	unsupportedKey
} from '../src/lib/phoneKeys.ts';

const model: PhoneModel = {
	id: 't41p',
	name: 'SIP-T41P',
	vendor: 'yealink',
	family: 'desk',
	accounts: 6,
	line_keys: 15,
	line_key_limit: 6,
	speed_dial_keys: true,
	video: false,
	ringtone_max_kb: 0,
	wallpaper: false,
	layout: {
		width: 192,
		height: 64,
		color: false,
		touch: false,
		left: 3,
		right: 3,
		columns: 2,
		page_key: true,
		pages: [
			[1, 4, 2, 5, 3, 0],
			[6, 9, 7, 10, 8, 0],
			[11, 14, 12, 15, 13, 0]
		]
	}
};
const accounts: PhoneAccount[] = [
	{
		account_index: 1,
		device_id: 'a1',
		extension_id: 'e1',
		extension_number: '101',
		display_name: 'Office',
		phone_label: 'Desk',
		phone_display_name: ''
	}
];
const key: LineKey = { key: 2, type: 'speed_dial', value: '**103', label: 'Workshop', account: 1 };
const contact = (id: string, section_id: string | null, extra: Partial<Contact> = {}): Contact => ({
	id,
	name: id,
	company: '',
	phone_work: '',
	phone_mobile: '',
	phone_other: '',
	section_id,
	...extra
});

test('picker only shows global and draft-assigned phonebooks, including all number fields', () => {
	const rows = [
		contact('Global', null, { phone_work: '+4930123' }),
		contact('Supplier', 'suppliers', {
			phone_work: '+4930456',
			phone_mobile: '+49170123',
			phone_other: '**103'
		}),
		contact('Private', 'private', { phone_work: '+4930789' }),
		contact('No number', 'suppliers')
	];
	assert.deepEqual(
		contactsForPhone(rows, ['suppliers']).map((c) => c.id),
		['Global', 'No number', 'Supplier']
	);
	assert.deepEqual(
		contactsForPhone(rows, ['suppliers'], 'global').map((c) => c.id),
		['Global']
	);
	assert.deepEqual(contactsForPhone(rows, [], 'suppliers'), []);
	assert.deepEqual(
		contactsForPhone(rows, ['suppliers'], '', '170').map((c) => c.id),
		['Supplier']
	);
	assert.deepEqual(contactNumbers(rows[1]), [
		{ kind: 'work', value: '+4930456' },
		{ kind: 'mobile', value: '+49170123' },
		{ kind: 'other', value: '**103' }
	]);
	assert.deepEqual(contactNumbers(rows[3]), []);
});

test('default, unused and configured assignments have distinct results', () => {
	assert.equal(effectivePhoneKey(1, [], accounts).label, 'Desk');
	assert.equal(effectivePhoneKey(1, [], accounts).type, 'line');
	assert.equal(effectivePhoneKey(2, [], accounts).type, 'none');
	assert.deepEqual(effectivePhoneKey(2, [key], accounts), key);
	const unused: LineKey = { key: 1, type: 'none', value: '', label: '', account: 1 };
	assert.equal(effectivePhoneKey(1, [unused], accounts).type, 'none');
	assert.equal(unsupportedKey({ ...key, key: 16 }, model), true);
	assert.equal(unsupportedKey({ ...key, key: 7, type: 'line' }, model), true);
	assert.equal(unsupportedKey({ ...key, key: 15 }, model), false);
	assert.deepEqual(
		phoneKeyPages(model)
			.flat()
			.filter(Boolean)
			.sort((a, b) => a - b),
		Array.from({ length: 15 }, (_, i) => i + 1)
	);
});

test('key validation protects model limits, missing accounts and dial targets', () => {
	assert.equal(keyValidation(key, model, accounts), null);
	assert.equal(keyValidation({ ...key, account: 2 }, model, accounts), 'phonekeys.missingAccount');
	assert.equal(keyValidation({ ...key, type: 'none' }, model, []), null);
	for (const value of ['${x}', '123\n456', '', '1'.repeat(33)])
		assert.equal(keyValidation({ ...key, value }, model, accounts), 'phonekeys.invalidNumber');
	for (const value of ['**103', '+4930555', '*97'])
		assert.equal(keyValidation({ ...key, value }, model, accounts), null);
	assert.equal(keyValidation({ ...key, type: 'blf', value: 'park+*71' }, model, accounts), null);
	assert.equal(
		keyValidation({ ...key, key: 7, type: 'line' }, model, accounts),
		'phonekeys.invalidSlot'
	);
	assert.equal(
		keyValidation(key, { ...model, speed_dial_keys: false }, accounts),
		'phonekeys.noSpeedDial'
	);
});
