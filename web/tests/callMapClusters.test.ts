import assert from 'node:assert/strict';
import test from 'node:test';
import type { MapArea } from '../src/lib/api.ts';
import {
	boxesOverlap,
	clusterCounts,
	clusterMarkers,
	type MarkerBox
} from '../src/lib/calls/mapClusters.ts';

const box = (id: string, x: number, y = 0): MarkerBox => ({
	id,
	x,
	y,
	width: 70,
	height: 80,
	labelHeight: 0
});
const separate = (groups: ReturnType<typeof clusterMarkers>) => {
	for (let i = 0; i < groups.length; i++) {
		for (let j = i + 1; j < groups.length; j++)
			assert.equal(boxesOverlap(groups[i], groups[j]), false);
	}
};

test('near phones bundle, separated phones stay individual and zoom splits the bundle', () => {
	const points = [box('bamberg', 0), box('hallstadt', 25), box('munich', 250)];
	const groups = clusterMarkers(points);
	assert.equal(groups.length, 2);
	assert.equal(groups.find((g) => g.members.length === 2)!.x, 12.5);
	separate(groups);
	const zoomed = clusterMarkers(points.map((p) => ({ ...p, x: p.x * 4 })));
	assert.equal(zoomed.length, 3);
	separate(zoomed);
	assert.deepEqual(clusterMarkers([...points].reverse()), groups, 'stable across API order');
	assert.equal(points[0].x, 0, 'input position unchanged');
});

test('large phones and persistent country labels are part of the footprint', () => {
	const points = [{ ...box('mobile', 0), width: 180, labelHeight: 20 }, box('area', 0, 95)];
	assert.equal(clusterMarkers(points).length, 1, 'label collides with phone below');
	assert.equal(clusterMarkers([{ ...box('large', 0), width: 120 }, box('small', 90)]).length, 1);
	assert.equal(clusterMarkers([box('a', 0), box('b', 80)]).length, 2, 'gap fits exactly');
});

test('new cluster centroids cannot hide a third phone; all areas remain reachable', () => {
	const points = [box('a', 0), box('b', 70), box('c', 105, 70), box('d', 200, 0)];
	const groups = clusterMarkers(points);
	separate(groups);
	assert.deepEqual(groups.flatMap((g) => g.members.map((p) => p.id)).sort(), ['a', 'b', 'c', 'd']);
	assert.equal(clusterMarkers([box('mobile', 10), box('service', 10)]).length, 1);
	assert.deepEqual(clusterMarkers([]), []);
});

test('dense layouts have disjoint final footprints and retain each member once', () => {
	const points = Array.from({ length: 240 }, (_, i) => ({
		...box(String(i), (i * 131) % 1500, (i * 73) % 900),
		width: i % 5 ? 70 : 180,
		labelHeight: i % 5 ? 0 : 20
	}));
	const groups = clusterMarkers(points);
	separate(groups);
	assert.equal(new Set(groups.flatMap((g) => g.members.map((p) => p.id))).size, points.length);
});

test('cluster category totals include all members without exposing call samples', () => {
	const areas = [
		{ counts: { answered: 2, missed: 1, outbound: 8, blocked: 0 } },
		{ counts: { answered: 3, missed: 0, outbound: 1, blocked: 4 } }
	] as MapArea[];
	assert.deepEqual(clusterCounts(areas), { answered: 5, missed: 1, outbound: 9, blocked: 4 });
	assert.deepEqual(clusterCounts([]), { answered: 0, missed: 0, outbound: 0, blocked: 0 });
});
