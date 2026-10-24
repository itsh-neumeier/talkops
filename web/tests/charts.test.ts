import assert from 'node:assert/strict';
import test from 'node:test';
import {
	areaPath,
	niceScale,
	ringOffset,
	smoothPath,
	sparkPoints,
	stack,
	trend,
	trendClass
} from '../src/lib/charts.ts';

test('round axes with whole-number steps', () => {
	assert.deepEqual(niceScale([]), { max: 4, step: 1 });
	assert.deepEqual(niceScale([3]), { max: 4, step: 1 });
	assert.deepEqual(niceScale([7]), { max: 8, step: 2 });
	assert.deepEqual(niceScale([13]), { max: 15, step: 5 });
	assert.deepEqual(niceScale([40]), { max: 40, step: 10 });
	assert.deepEqual(niceScale([42]), { max: 60, step: 20 });
	assert.deepEqual(niceScale([101]), { max: 150, step: 50 });
});

test('smooth lines stay inside the values', () => {
	const pts = [
		{ x: 0, y: 100 },
		{ x: 10, y: 0 },
		{ x: 20, y: 0 },
		{ x: 30, y: 100 }
	];
	const d = smoothPath(pts);
	assert.match(d, /^M0,100C/);
	// No control point above the peak or below the base of the flat part.
	const ys = [...d.matchAll(/,(-?[\d.]+)/g)].map((m) => Number(m[1]));
	assert.ok(
		ys.every((y) => y >= 0 && y <= 100),
		d
	);
	assert.equal(smoothPath([]), '');
	assert.equal(smoothPath([{ x: 1, y: 2 }]), 'M1,2');
	assert.match(areaPath(pts, 120), /L30,120L0,120Z$/);
});

test('stacks series', () => {
	assert.deepEqual(
		stack([
			[1, 2],
			[3, 4],
			[0, 1]
		]),
		[
			[1, 2],
			[4, 6],
			[4, 7]
		]
	);
});

test('trend against the previous period', () => {
	assert.deepEqual(trend(12, 10), { percent: 20, direction: 'up' });
	assert.deepEqual(trend(5, 10), { percent: -50, direction: 'down' });
	assert.deepEqual(trend(3, 3), { percent: 0, direction: 'flat' });
	assert.deepEqual(trend(3, 0), { percent: null, direction: 'up' });
	assert.equal(trend(null, 3), null);
	assert.match(trendClass(trend(12, 10)), /emerald/);
	assert.match(trendClass(trend(12, 10), false), /red/);
	assert.match(trendClass(trend(3, 3)), /slate/);
});

test('sparklines and ring gauges', () => {
	const p = sparkPoints([0, 5, 10], 100, 20);
	assert.deepEqual(p[0], { x: 2, y: 18 });
	assert.deepEqual(p[2], { x: 98, y: 2 });
	assert.deepEqual(sparkPoints([], 10, 10), []);
	assert.equal(ringOffset(100, 100), 0);
	assert.equal(ringOffset(25, 100), 75);
	assert.equal(ringOffset(null, 100), 100);
	assert.equal(ringOffset(140, 100), 0);
});
