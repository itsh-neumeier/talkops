import assert from 'node:assert/strict';
import test from 'node:test';
import { DASH_LIMIT, readOpen, visibleItems, writeOpen } from '../src/lib/collapse.ts';

test('a collapsed list shows the first entries and kept ones', () => {
	const items = [1, 2, 3, 4, 5, 6];
	assert.equal(DASH_LIMIT, 3);
	assert.deepEqual(visibleItems(items, false), [1, 2, 3]);
	assert.deepEqual(visibleItems(items, true), items);
	assert.deepEqual(
		visibleItems(items, false, 3, (n) => n === 5),
		[1, 2, 3, 5]
	);
	assert.deepEqual(visibleItems([1], false), [1]);
});

test('the open state survives missing storage', () => {
	assert.equal(readOpen('x'), false);
	writeOpen('x', true);
});
