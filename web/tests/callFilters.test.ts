import assert from 'node:assert/strict';
import test from 'node:test';
import {
	activeCount,
	apiQuery,
	callResult,
	daysAgo,
	emptyFilters,
	pageQuery,
	parseFilters
} from '../src/lib/calls/filters.ts';

test('the page address keeps only changed filters and reads them back', () => {
	assert.equal(pageQuery(emptyFilters()), '');
	const f = {
		...emptyFilters(),
		since: '2026-10-01',
		direction: ['inbound' as const, 'internal' as const],
		result: ['missed' as const],
		to: ' Zweig ',
		recording: 'with' as const,
		sort: 'duration' as const,
		order: 'asc' as const
	};
	const query = pageQuery(f);
	assert.equal(
		query,
		'?since=2026-10-01&direction=inbound%2Cinternal&result=missed&to=Zweig&recording=with&sort=duration&order=asc'
	);
	const back = parseFilters(new URLSearchParams(query));
	assert.deepEqual(back, { ...f, to: 'Zweig' });
});

test('invalid address values are ignored', () => {
	const f = parseFilters(
		new URLSearchParams(
			'since=yesterday&direction=inbound,sideways,inbound&result=x&recording=all&sort=name&order=up'
		)
	);
	assert.deepEqual(f, { ...emptyFilters(), direction: ['inbound'] });
});

test('the API query has inclusive local days, sort order and the page', () => {
	const q = new URLSearchParams(
		apiQuery({ ...emptyFilters(), since: '2026-10-01', until: '2026-10-01' }, 50, 50).slice(1)
	);
	const since = new Date(q.get('since')!);
	const until = new Date(q.get('until')!);
	assert.equal(since.getDate(), 1);
	assert.equal(until.getDate(), 2, 'up to the start of the next day');
	assert.equal(until.getTime() - since.getTime(), 24 * 3600 * 1000);
	assert.equal(q.get('sort'), 'time');
	assert.equal(q.get('order'), 'desc');
	assert.equal(q.get('offset'), '50');
	assert.equal(q.get('limit'), '50');
});

test('filter marks, quick ranges and call results', () => {
	const marks = activeCount({ ...emptyFilters(), from: ' ', until: '2026-10-02' });
	assert.equal(marks.from, false);
	assert.equal(marks.time, true);
	assert.equal(daysAgo(0, new Date(2026, 9, 15, 23, 30)), '2026-10-15');
	assert.equal(daysAgo(6, new Date(2026, 9, 3, 1)), '2026-09-27');
	assert.equal(
		callResult({ answered_at: '2026-10-15T10:00:00Z', hangup_cause: 'USER_BUSY' }),
		'answered'
	);
	assert.equal(callResult({ answered_at: null, hangup_cause: 'USER_BUSY' }), 'busy');
	assert.equal(callResult({ answered_at: null, hangup_cause: 'NO_ANSWER' }), 'missed');
});
