import assert from 'node:assert/strict';
import test from 'node:test';
import { allowNumber, blockSource, isBlocked } from '../src/lib/calls/blocking.ts';
import {
	apiQuery,
	callResult,
	emptyFilters,
	pageQuery,
	parseFilters
} from '../src/lib/calls/filters.ts';
import { missedIncoming } from '../src/lib/softphone.ts';
import type { Call } from '../src/lib/api.ts';

test('stored block decisions take priority and legacy declines keep their result', () => {
	for (const reason of ['list:+4930*', 'phoneblock:12', 'anonymous']) {
		const call = {
			direction: 'inbound',
			answered_at: null,
			hangup_cause: 'CALL_REJECTED',
			blocked_reason: reason
		} as Call;
		assert.equal(callResult(call), 'blocked');
		assert.equal(isBlocked(call), true);
		assert.equal(missedIncoming(call, 'extension'), false);
		assert.equal(callResult({ ...call, allowlisted: true }), 'blocked');
	}
	assert.equal(
		callResult({ direction: 'inbound', answered_at: null, hangup_cause: 'CALL_REJECTED' }),
		'busy'
	);
	assert.equal(isBlocked({ direction: 'outbound', blocked_reason: 'list:+4930*' }), false);
	assert.equal(
		callResult({
			direction: 'internal',
			answered_at: null,
			hangup_cause: 'NO_ANSWER',
			blocked_reason: 'anonymous'
		}),
		'missed'
	);
});

test('only exact international numbers are offered as whitelist candidates', () => {
	assert.equal(allowNumber('+4930123456'), true);
	for (const number of [
		'anonymous',
		'101',
		'004930123456',
		'+4930*',
		'+0123456',
		'+1234567890123456'
	])
		assert.equal(allowNumber(number), false, number);
	assert.equal(blockSource('list:+4930*'), 'list');
	assert.equal(blockSource('phoneblock:12'), 'phoneblock');
	assert.equal(blockSource('anonymous'), 'anonymous');
	assert.equal(blockSource('unexpected'), 'unknown');
});

test('blocked result survives address restoration and API pagination', () => {
	const filters = { ...emptyFilters(), result: ['blocked' as const, 'busy' as const] };
	assert.deepEqual(parseFilters(new URLSearchParams(pageQuery(filters))).result, [
		'blocked',
		'busy'
	]);
	const query = new URLSearchParams(apiQuery(filters, 50, 50).slice(1));
	assert.equal(query.get('result'), 'blocked,busy');
	assert.equal(query.get('offset'), '50');
});
