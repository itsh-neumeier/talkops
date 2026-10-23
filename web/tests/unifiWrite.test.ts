import assert from 'node:assert/strict';
import test from 'node:test';
import {
	applyBody,
	canRollBack,
	FIELD_KEYS,
	initialSelection,
	slotText,
	valueText,
	warnings,
	writeErrorKey,
	type Proposal,
	type WifiChange
} from '../src/lib/unifiWrite.ts';
import en from '../src/lib/i18n/en.ts';

const l = { on: 'on', off: 'off', absent: 'not set' };

const proposal: Proposal = {
	wifi_id: 'w1',
	wifi_name: 'Voice',
	base_hash: 'h1',
	not_writable: null,
	write: { allowed: true, reason: null, version: '11.0.89', min_version: '10.6.106' },
	hints: [{ id: 'fast_roaming_80211k', recommendation: 'fast_roaming' }],
	changes: [
		{
			field: 'bssTransitionEnabled',
			current: { value: false },
			proposed: true,
			recommendation: 'fast_roaming',
			selected: true
		},
		{
			field: 'broadcastingFrequenciesGHz',
			current: { value: [2.4, 5] },
			proposed: [5],
			recommendation: 'band_5ghz',
			selected: false,
			warning: 'band_2g_removed'
		},
		{
			field: 'securityConfiguration.wpa3FastRoamingEnabled',
			current: { value: false },
			proposed: true,
			recommendation: 'fast_roaming',
			selected: true,
			warning: 'fast_roaming_compat'
		}
	]
};

test('formats SSID values for the diff dialog', () => {
	assert.equal(valueText('uapsdEnabled', true, l), 'on');
	assert.equal(valueText('securityConfiguration.pmfMode', null, l), 'off');
	assert.equal(valueText('securityConfiguration.pmfMode', 'OPTIONAL', l), 'OPTIONAL');
	assert.equal(valueText('broadcastingFrequenciesGHz', [2.4, 5], l), '2.4 GHz + 5 GHz');
	assert.equal(
		valueText(
			'handoffSuggestionsConfiguration',
			{ band5GHzRssiThreshold: -75, band6GHzRssiThreshold: null },
			l
		),
		'5 GHz: -75 dBm · 6 GHz: off'
	);
	assert.equal(
		valueText('dtimPeriodByFrequencyGHzOverride', { '2.4': 3, '5': 1, '6': 1 }, l),
		'2.4 GHz: 3 · 5 GHz: 1 · 6 GHz: 1'
	);
	assert.equal(slotText('handoffSuggestionsConfiguration', { absent: true }, l), 'not set');
	assert.equal(slotText('uapsdEnabled', null, l), 'not set');
	assert.equal(slotText('uapsdEnabled', { value: false }, l), 'off');
});

test('builds the apply request from the selected rows only', () => {
	const sel = initialSelection(proposal);
	assert.deepEqual(
		[...sel],
		['bssTransitionEnabled', 'securityConfiguration.wpa3FastRoamingEnabled']
	);
	const body = applyBody(proposal, sel, 's1');
	assert.deepEqual(body, {
		survey_id: 's1',
		wifi_id: 'w1',
		base_hash: 'h1',
		confirm: true,
		changes: [
			{ field: 'bssTransitionEnabled', value: true },
			{ field: 'securityConfiguration.wpa3FastRoamingEnabled', value: true }
		]
	});
	assert.deepEqual(warnings(proposal, sel), ['disconnect', 'fast_roaming_compat']);
	sel.add('broadcastingFrequenciesGHz');
	assert.deepEqual(warnings(proposal, sel), [
		'disconnect',
		'band_2g_removed',
		'fast_roaming_compat'
	]);
});

test('only applied or failed changes with snapshots can roll back', () => {
	const c: WifiChange = {
		id: 'c1',
		site_id: 's',
		wifi_id: 'w1',
		wifi_name: 'Voice',
		kind: 'apply',
		rollback_of: null,
		survey_id: null,
		state: 'applied',
		fields: ['uapsdEnabled'],
		before: { uapsdEnabled: { value: false } },
		after: { uapsdEnabled: { value: true } },
		network_version: '11.0.89',
		error: null,
		created_by: null,
		created_by_name: null,
		created_at: '2026-10-22T10:00:00Z',
		finished_at: null
	};
	assert.ok(canRollBack(c));
	assert.ok(!canRollBack({ ...c, state: 'rolled_back' }));
	assert.ok(!canRollBack({ ...c, kind: 'rollback' }));
	assert.ok(!canRollBack({ ...c, state: 'failed', after: null }));
});

test('every field, hint, warning and error code has a UI text', () => {
	const keys = new Set(Object.keys(en));
	for (const f of FIELD_KEYS) assert.ok(keys.has(`unifiWrite.field.${f}`), f);
	for (const h of [
		'fast_roaming_80211r',
		'fast_roaming_80211k',
		'min_data_rate',
		'tx_power',
		'coverage',
		'pingpong'
	])
		assert.ok(keys.has(`unifiWrite.hint.${h}`), h);
	for (const w of ['disconnect', 'fast_roaming_compat', 'band_2g_removed'])
		assert.ok(keys.has(`unifiWrite.warn.${w}`), w);
	for (const r of ['fast_roaming', 'min_rssi', 'band_5ghz', 'voice_ssid', 'coverage', 'pingpong'])
		assert.ok(keys.has(`unifiWrite.rec.${r}`), r);
	const key = writeErrorKey({ code: 'survey_active' });
	assert.equal(key, 'unifiWrite.err.survey_active');
	assert.ok(keys.has(key!));
	assert.equal(writeErrorKey({ code: 'conflict' }), null);
	assert.equal(writeErrorKey(new Error('x')), null);
});
