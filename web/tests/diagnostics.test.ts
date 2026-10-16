import assert from 'node:assert/strict';
import test from 'node:test';
import { freeswitchUptime, since } from '../src/lib/diagnostics.ts';

test('since formats days, hours and minutes', () => {
	const now = Date.parse('2026-10-16T12:00:00Z');
	assert.equal(since('2026-10-16T11:55:00Z', now), '5m');
	assert.equal(since('2026-10-16T09:30:00Z', now), '2h 30m');
	assert.equal(since('2026-10-13T11:00:00Z', now), '3d 1h 0m');
	assert.equal(since('2026-10-16T13:00:00Z', now), '0m');
});

test('freeswitchUptime reads the first status line', () => {
	assert.equal(
		freeswitchUptime('UP 0 years, 3 days, 4 hours, 12 minutes, 5 seconds\nmore'),
		'3d 4h 12m'
	);
	assert.equal(freeswitchUptime('UP 1 years, 0 days, 0 hours, 1 minutes'), '365d 0h 1m');
	assert.equal(freeswitchUptime(null), '0m');
});
