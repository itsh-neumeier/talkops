import assert from 'node:assert/strict';
import test from 'node:test';
import {
	choiceKey,
	formatBytes,
	formatEta,
	formatUsd,
	percent
} from '../src/lib/transcriptions/format.ts';

test('durations, sizes, prices and shares', () => {
	assert.equal(formatEta(null), '—');
	assert.equal(formatEta(0.2), '1 s');
	assert.equal(formatEta(45), '45 s');
	assert.equal(formatEta(170), '3 min');
	assert.equal(formatEta(3600), '1 h');
	assert.equal(formatEta(3900), '1 h 5 min');
	assert.equal(formatBytes(2.5 * 1024 ** 3), '2.5 GB');
	assert.equal(formatBytes(300 * 1024 ** 2), '300 MB');
	assert.equal(formatUsd(0.012), '$0.012');
	assert.equal(formatUsd(1.5), '$1.50');
	assert.equal(percent(0.456), 46);
	assert.equal(percent(2), 100);
	assert.equal(percent(null), 0);
	assert.equal(choiceKey({ kind: 'local', model: 'fast' }), 'local:fast');
	assert.equal(
		choiceKey({ kind: 'cloud', provider_id: 'p', model: 'whisper-1' }),
		'cloud:p:whisper-1'
	);
});
