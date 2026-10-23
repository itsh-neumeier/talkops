import assert from 'node:assert/strict';
import test from 'node:test';
import {
	CATEGORIES,
	bubbleGradient,
	bubbleSize,
	callCategory,
	defaultMapState,
	listHref,
	mapApiQuery,
	mapHref,
	mapPageQuery,
	parseMapState,
	shownTotal
} from '../src/lib/calls/map.ts';
import { apiQuery, emptyFilters, pageQuery, parseFilters } from '../src/lib/calls/filters.ts';

test('the map view, range and categories survive the page address', () => {
	assert.deepEqual(parseMapState(new URLSearchParams('')), defaultMapState());
	const s = {
		view: 'map' as const,
		range: '1w' as const,
		categories: ['outbound' as const, 'missed' as const]
	};
	const query = mapPageQuery(s);
	assert.equal(query, '?view=map&range=1w&categories=missed%2Coutbound');
	const back = parseMapState(new URLSearchParams(query));
	assert.deepEqual(back, { ...s, categories: ['missed', 'outbound'] });
	// Defaults are left out; all categories are the default.
	assert.equal(
		mapPageQuery({ view: 'map', range: '1d', categories: [...CATEGORIES] }),
		'?view=map'
	);
	assert.equal(mapHref('1h'), '/calls?view=map&range=1h');
});

test('invalid map address values give the defaults', () => {
	const s = parseMapState(new URLSearchParams('view=globe&range=2d&categories=spam,answered'));
	assert.deepEqual(s, { view: 'list', range: '1d', categories: ['answered'] });
	assert.deepEqual(
		parseMapState(new URLSearchParams('view=map&categories=x')).categories,
		CATEGORIES
	);
});

test('the map request has the range and only a category subset', () => {
	assert.equal(mapApiQuery({ range: '1m', categories: [...CATEGORIES] }), '/calls/map?range=1m');
	assert.equal(
		mapApiQuery({ range: '1h', categories: ['blocked', 'answered'] }),
		'/calls/map?range=1h&categories=answered%2Cblocked'
	);
});

test('a bubble link opens the list with the rolling range and the area', () => {
	const href = listHref('1d', 'DE-951', 'Bamberg · +49 951');
	assert.equal(href, '/calls?range=1d&area=DE-951&place=Bamberg+%C2%B7+%2B49+951');
	const f = parseFilters(new URL(href, 'http://x').searchParams);
	assert.equal(f.range, '1d');
	assert.equal(f.area, 'DE-951');
	assert.equal(f.place, 'Bamberg · +49 951');
	assert.equal(pageQuery(f), '?range=1d&area=DE-951&place=Bamberg+%C2%B7+%2B49+951');
	const now = Date.parse('2026-10-23T12:00:00Z');
	const api = new URLSearchParams(apiQuery(f, 0, 50, now).slice(1));
	assert.equal(api.get('since'), '2026-10-22T12:00:00.000Z');
	assert.equal(api.get('until'), null);
	assert.equal(api.get('area'), 'DE-951');
});

test('a rolling range replaces dates and an invalid area is ignored', () => {
	const f = parseFilters(new URLSearchParams('range=1m&since=2026-10-01&area=DE 951;drop'));
	assert.deepEqual(f, { ...emptyFilters(), range: '1m' });
	const month = new URLSearchParams(
		apiQuery(f, 0, 50, Date.parse('2026-10-31T00:00:00Z')).slice(1)
	);
	assert.equal(month.get('since'), '2026-10-01T00:00:00.000Z');
	assert.equal(parseFilters(new URLSearchParams('range=5y')).range, '');
	// A place label without an area is not kept.
	assert.equal(parseFilters(new URLSearchParams('place=x')).place, '');
});

test('bubble areas grow with the number of calls', () => {
	assert.equal(bubbleSize(0, 10), 0);
	assert.equal(bubbleSize(10, 10), 56);
	assert.equal(bubbleSize(1, 1_000_000), 14);
	const quarter = bubbleSize(25, 100, 0, 40);
	assert.equal(quarter, 20, 'a quarter of the calls gives half the diameter');
});

test('bubble segments follow the shown categories', () => {
	const counts = { answered: 1, missed: 1, outbound: 2, blocked: 0 };
	assert.equal(shownTotal(counts, [...CATEGORIES]), 4);
	assert.equal(shownTotal(counts, ['outbound']), 2);
	assert.equal(
		bubbleGradient(counts, [...CATEGORIES]),
		'conic-gradient(#059669 0.0deg 90.0deg, #dc2626 90.0deg 180.0deg, #2563eb 180.0deg 360.0deg)'
	);
	assert.equal(bubbleGradient(counts, ['blocked']), 'transparent');
	assert.match(bubbleGradient(counts, ['answered'], true), /^conic-gradient\(#34d399 /);
});

test('calls fall into the server categories', () => {
	const base = { answered_at: null, blocked_reason: null };
	assert.equal(callCategory({ ...base, direction: 'outbound' }), 'outbound');
	assert.equal(callCategory({ ...base, direction: 'inbound' }), 'missed');
	assert.equal(
		callCategory({ ...base, direction: 'inbound', answered_at: '2026-10-23T10:00:00Z' }),
		'answered'
	);
	assert.equal(
		callCategory({ ...base, direction: 'inbound', blocked_reason: 'phoneblock:12' }),
		'blocked'
	);
});

test('tile URLs are checked like on the server', async () => {
	const { tileUrlProblem } = await import('../src/lib/calls/map.ts');
	assert.equal(tileUrlProblem(''), '');
	assert.equal(tileUrlProblem('https://tile.openstreetmap.org/{z}/{x}/{y}.png'), '');
	assert.equal(tileUrlProblem('https://{s}.tile.example.com/{z}/{x}/{y}{r}.png'), '');
	assert.equal(tileUrlProblem('http://tile.example.com/{z}/{x}/{y}.png'), 'https');
	assert.equal(tileUrlProblem('https://tile.example.com/{z}/{x}.png'), 'placeholders');
	assert.equal(tileUrlProblem('https://tile.example.com/{z}/{x}/{y}/{key}.png'), 'invalid');
	assert.equal(tileUrlProblem("https://t.example.com/{z}/{x}/{y}.png';"), 'invalid');
});

test('the dashboard lists the areas with the most shown calls', async () => {
	const { topAreas } = await import('../src/lib/calls/map.ts');
	const area = (id: string, kind: string, answered: number, outbound: number) => ({
		id,
		kind,
		counts: { answered, missed: 0, outbound, blocked: 0 }
	});
	const areas = [
		area('a', 'area', 1, 0),
		area('b', 'area', 0, 5),
		area('n', 'none', 9, 9),
		area('c', 'area', 3, 0)
	];
	assert.deepEqual(
		topAreas(areas, [...CATEGORIES]).map((a) => a.id),
		['b', 'c', 'a']
	);
	assert.deepEqual(
		topAreas(areas, ['answered'], 2).map((a) => a.id),
		['c', 'a']
	);
});
