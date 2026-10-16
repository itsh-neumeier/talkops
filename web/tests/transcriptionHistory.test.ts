import assert from 'node:assert/strict';
import test from 'node:test';
import { digitsMatch } from '../src/lib/list/sort.ts';
import {
	activeFilters,
	emptyFilters,
	matches,
	pageQuery,
	parseFilters,
	runSecs,
	sortJobs,
	type HistoryRow
} from '../src/lib/transcriptions/history.ts';

const row = (id: string, extra: Partial<HistoryRow> = {}): HistoryRow & { id: string } => ({
	id,
	status: 'done',
	engine_key: 'local:fast',
	direction: 'inbound',
	caller_number: null,
	caller_name: null,
	destination: null,
	requested_by: null,
	created_at: '2026-10-15T09:00:00Z',
	started_at: null,
	finished_at: null,
	...extra
});

test('national numbers find international numbers', () => {
	assert.ok(digitsMatch('0911 2193', ['+499112193553']));
	assert.ok(digitsMatch('+49 911', ['0049911']));
	assert.ok(!digitsMatch('09', ['+499112193553']), 'too short');
	assert.ok(!digitsMatch('Maria 0911', ['+499112193553']), 'not only digits');
});

test('the page address keeps the history filters', () => {
	assert.equal(pageQuery(emptyFilters()), '?tab=history');
	const f = {
		...emptyFilters(),
		status: ['failed' as const],
		engine: ['local:fast', 'cloud:p1:whisper-1'],
		direction: ['internal' as const],
		since: '2026-10-14',
		until: '2026-10-15',
		search: ' Maria ',
		sort: 'status' as const,
		order: 'asc' as const
	};
	const query = pageQuery(f);
	assert.deepEqual(parseFilters(new URLSearchParams(query)), { ...f, search: 'Maria' });
	assert.deepEqual(
		parseFilters(new URLSearchParams('status=running,done&direction=up&since=x&sort=name')),
		{ ...emptyFilters(), status: ['done'] },
		'only final states and known values'
	);
	assert.deepEqual(activeFilters(f), { time: true, direction: true, engine: true, status: true });
});

test('filters by status, model, direction, day and parties', () => {
	const call = row('call', {
		finished_at: '2026-10-15T10:00:00Z',
		caller: { name: 'Familie Vollmeier', number: '+4995436999124' },
		called: { name: 'Familie Zweig', number: '101' },
		via_number: '+4995431241'
	});
	const f = emptyFilters();
	assert.ok(matches(call, f));
	assert.ok(!matches(call, { ...f, status: ['failed'] }));
	assert.ok(matches(call, { ...f, engine: ['local:fast'] }));
	assert.ok(!matches(call, { ...f, engine: ['local:accurate'] }));
	assert.ok(!matches(call, { ...f, direction: ['outbound'] }));
	assert.ok(!matches(row('none', { direction: null }), { ...f, direction: ['inbound'] }));
	assert.ok(matches(call, { ...f, since: '2026-10-15', until: '2026-10-15' }));
	assert.ok(!matches(call, { ...f, since: '2026-10-16' }));
	assert.ok(!matches(call, { ...f, until: '2026-10-14' }));
	assert.ok(matches(call, { ...f, search: 'zweig' }));
	assert.ok(matches(call, { ...f, search: '095431241' }), 'the dialed own number');
	assert.ok(!matches(call, { ...f, search: 'bob' }));
});

test('sorts by time, duration and status in both directions', () => {
	const list = [
		row('a', {
			status: 'failed',
			started_at: '2026-10-14T10:00:00Z',
			finished_at: '2026-10-14T10:00:30Z'
		}),
		row('b', { started_at: '2026-10-15T09:00:00Z', finished_at: '2026-10-15T09:03:10Z' }),
		row('c', { status: 'cancelled', created_at: '2026-10-15T12:00:00Z' })
	];
	const ids = (sort: 'time' | 'duration' | 'status', order: 'asc' | 'desc') =>
		sortJobs(list, { sort, order }).map((j) => j.id);
	assert.deepEqual(ids('time', 'desc'), ['c', 'b', 'a']);
	assert.deepEqual(ids('time', 'asc'), ['a', 'b', 'c']);
	assert.deepEqual(ids('duration', 'desc'), ['b', 'a', 'c'], 'not run: last');
	assert.deepEqual(ids('duration', 'asc'), ['a', 'b', 'c']);
	assert.deepEqual(ids('status', 'asc'), ['b', 'a', 'c']);
	assert.deepEqual(ids('status', 'desc'), ['c', 'a', 'b']);
	assert.equal(runSecs(list[1]), 190);
	assert.equal(runSecs(list[2]), null);
});
