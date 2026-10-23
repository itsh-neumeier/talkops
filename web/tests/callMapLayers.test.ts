import assert from 'node:assert/strict';
import test from 'node:test';
import {
	OFFLINE_MAX_ZOOM,
	labelBox,
	layerColors,
	onbArea,
	placeFiles,
	placeLabels,
	placeName,
	visibleLayers,
	waterVisible,
	type Place
} from '../src/lib/calls/mapLayers.ts';

test('detail layers appear at their zoom levels', () => {
	assert.deepEqual(visibleLayers(4, false), []);
	assert.deepEqual(visibleLayers(5, false), ['admin1', 'lakes', 'rivers', 'places']);
	assert.deepEqual(visibleLayers(8, false), ['admin1', 'lakes', 'rivers', 'places', 'onb']);
	assert.equal(OFFLINE_MAX_ZOOM, 11);
});

test('with a tile server only the local network areas stay', () => {
	assert.deepEqual(visibleLayers(6, true), []);
	assert.deepEqual(visibleLayers(9, true), ['onb']);
});

test('place files load by zoom level', () => {
	assert.deepEqual(placeFiles(4), []);
	assert.deepEqual(placeFiles(5), ['/map/places-5.json']);
	assert.deepEqual(placeFiles(7.5), ['/map/places-5.json', '/map/places-7.json']);
	assert.equal(placeFiles(10).at(-1), '/map/places-10.json');
});

test('major water shows from zoom 5, other water from zoom 7', () => {
	assert.equal(waterVisible(5, 5), true);
	assert.equal(waterVisible(7, 6), false);
	assert.equal(waterVisible(7, 7), true);
	assert.equal(waterVisible(undefined, 6), false);
});

test('labels of larger places win and never overlap', () => {
	const places: Place[] = [
		['Hirschaid', 49.82, 10.99, 12_000],
		['Bamberg', 49.89, 10.89, 77_000],
		['Nuremberg', 49.45, 11.08, 515_000, 'Nürnberg'],
		['Far away', 10, 10, 2_000_000]
	];
	// Bamberg and Hirschaid share a position; Far away is outside the view.
	const at: Record<string, { x: number; y: number } | null> = {
		'49.82': { x: 100, y: 100 },
		'49.89': { x: 104, y: 102 },
		'49.45': { x: 100, y: 300 },
		'10': null
	};
	const labels = placeLabels(places, (lat) => at[String(lat)] ?? null, 250, 'de');
	assert.deepEqual(
		labels.map((l) => l.name),
		['Nürnberg', 'Bamberg']
	);
	assert.equal(labels[0].major, true);
	assert.equal(placeLabels(places, (lat) => at[String(lat)] ?? null, 1).length, 1, 'limit');
	assert.equal(placeLabels(places, (lat) => at[String(lat)] ?? null)[0].name, 'Nuremberg');
	assert.ok(labelBox('Nürnberg', true).w > labelBox('Nürnberg', false).w);
});

test('place names follow the UI language', () => {
	assert.equal(placeName(['Munich', 0, 0, 1, 'München'], 'de'), 'München');
	assert.equal(placeName(['Munich', 0, 0, 1, 'München'], 'en'), 'Munich');
	assert.equal(placeName(['Bamberg', 0, 0, 1], 'de'), 'Bamberg');
});

test('only German local network areas have an outline', () => {
	assert.equal(onbArea('DE-951'), 'DE-951');
	assert.equal(onbArea('DE-none'), null);
	assert.equal(onbArea('AT-1'), null);
	assert.equal(onbArea(null), null);
	assert.notEqual(layerColors(true).onb, layerColors(false).onb);
});
