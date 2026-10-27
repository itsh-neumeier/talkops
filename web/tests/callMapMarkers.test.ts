import assert from 'node:assert/strict';
import test from 'node:test';
import type { MapArea } from '../src/lib/api.ts';
import { decorationsFit, markerHtml, markerKind, markerName } from '../src/lib/calls/mapMarkers.ts';

const base: MapArea = {
	id: 'DE-mobile',
	kind: 'none',
	country: 'DE',
	name: 'Germany',
	prefix: '+49',
	lat: 51,
	lon: 10,
	counts: { answered: 2, missed: 1, outbound: 0, blocked: 0 },
	total: 3,
	billsec: 30,
	mobile: 3,
	service: 0,
	unresolved: 0,
	calls: []
};
test('device shapes distinguish mobile, local areas and mixed legacy groups', () => {
	assert.equal(markerKind(base), 'mobile');
	assert.equal(markerKind({ ...base, service: 1 }), 'unlocated');
	assert.equal(markerKind({ ...base, kind: 'area' }), 'area');
	assert.equal(markerKind({ ...base, marker_kind: 'unlocated' }), 'unlocated');
	assert.equal(markerName(base, 'ohne Ortsbezug'), 'DE ohne Ortsbezug');
	assert.equal(markerName({ ...base, kind: 'area', name: 'Bamberg' }, 'ohne Ortsbezug'), 'Bamberg');
});
test('marker markup escapes labels and includes only shown categories', () => {
	const html = markerHtml(base, ['answered'], 24, false, '<img onerror="bad()">', true);
	assert.ok(!html.includes('<img'));
	assert.match(html, /&lt;img/);
	assert.match(html, /call-device-mobile/);
	assert.ok(!html.includes('#dc2626'));
	assert.equal(
		(
			markerHtml({ ...base, kind: 'area' }, ['answered'], 20, false, 'Bamberg', false).match(
				/<b>/g
			) || []
		).length,
		9
	);
	assert.ok(!markerHtml(base, ['answered'], 20, false, 'DE', false).includes('call-marker-stats'));
});
test('counts and labels disappear on dense maps and narrow/compact surfaces', () => {
	const points = [
		{ x: 150, y: 100 },
		{ x: 400, y: 250 }
	];
	assert.equal(decorationsFit(points, 0, 736, false), true);
	assert.equal(decorationsFit(points, 0, 320, false), false);
	assert.equal(decorationsFit(points, 0, 736, true), false);
	assert.equal(
		decorationsFit(
			[
				{ x: 150, y: 100 },
				{ x: 200, y: 150 }
			],
			0,
			736,
			false
		),
		false
	);
	assert.equal(decorationsFit([{ x: 20, y: 100 }], 0, 736, false), false);
});
