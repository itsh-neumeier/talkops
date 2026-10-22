import assert from 'node:assert/strict';
import test from 'node:test';
import {
	apLabel,
	band,
	clock,
	mbit,
	mergeTimeline,
	newestOffset,
	ratingBadge,
	rssiLevel,
	type Timeline
} from '../src/lib/wifiSurvey.ts';

const media = (offset: number) => ({
	offset_ms: offset,
	interval_ms: 1000,
	received: 50,
	expected: 50,
	flaws: 0,
	mos: 4.4,
	jitter_max_ms: null,
	rtcp_fraction_lost: null,
	rtcp_jitter_ms: null
});

test('formats survey offsets, bands, rates and labels', () => {
	assert.equal(clock(0), '0:00');
	assert.equal(clock(61_500), '1:01');
	assert.equal(clock(3_725_000), '1:02:05');
	assert.equal(band('ng'), '2.4 GHz');
	assert.equal(band('na'), '5 GHz');
	assert.equal(band(null), '—');
	assert.equal(mbit(144_444), '144');
	assert.equal(mbit(6_500), '6.5');
	assert.equal(apLabel(null, 'aa:bb'), 'aa:bb');
	assert.equal(apLabel('Hall', 'aa:bb'), 'Hall');
	assert.equal(ratingBadge('bad'), 'badge badge-bad');
	assert.equal(ratingBadge(null), 'badge badge-muted');
	assert.equal(rssiLevel(-60), 'clean');
	assert.equal(rssiLevel(-70), 'audible');
	assert.equal(rssiLevel(-80), 'bad');
});

test('live updates append only new values', () => {
	const base: Timeline = { media: [media(1000), media(2000)], radio: [], events: [] };
	assert.equal(newestOffset(base), 2000);
	assert.equal(newestOffset({ media: [], radio: [], events: [] }), -1);
	const merged = mergeTimeline(base, {
		media: [media(2000), media(3000)],
		radio: [],
		events: [
			{
				id: 'm1',
				offset_ms: 2500,
				kind: 'marker',
				marker: 'location',
				source: 'ui',
				note: '',
				data: {}
			}
		]
	});
	assert.deepEqual(
		merged.media.map((m) => m.offset_ms),
		[1000, 2000, 3000]
	);
	assert.equal(merged.events.length, 1);
	const again = mergeTimeline(merged, { media: [], radio: [], events: merged.events });
	assert.equal(again.events.length, 1);
});
