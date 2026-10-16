import assert from 'node:assert/strict';
import test from 'node:test';
import { ariaSort, nextSort, pickList, toggle } from '../src/lib/list/sort.ts';
import {
	emptyFilters,
	knownSections,
	matches,
	pageQuery,
	parseFilters,
	sortContacts,
	type ContactRow
} from '../src/lib/phonebook/filters.ts';

const contact = (name: string, extra: Partial<ContactRow> = {}): ContactRow & { id: string } => ({
	id: name,
	name,
	company: '',
	phone_work: '',
	phone_mobile: '',
	phone_other: '',
	section_id: null,
	...extra
});

test('shared sort helpers', () => {
	assert.deepEqual(nextSort({ sort: 'name', order: 'asc' }, 'name'), {
		sort: 'name',
		order: 'desc'
	});
	assert.deepEqual(nextSort({ sort: 'name', order: 'desc' }, 'company', 'asc'), {
		sort: 'company',
		order: 'asc'
	});
	assert.equal(ariaSort(false, 'asc'), 'none');
	assert.equal(ariaSort(true, 'asc'), 'ascending');
	assert.equal(ariaSort(true, 'desc'), 'descending');
	assert.deepEqual(pickList(['a', 'b'] as const, 'b,x,b,a'), ['b', 'a']);
	assert.deepEqual(toggle(['a'], 'b'), ['a', 'b']);
	assert.deepEqual(toggle(['a', 'b'], 'a'), ['b']);
});

test('the page address keeps the filters and the contact link', () => {
	assert.equal(pageQuery(emptyFilters()), '');
	const f = {
		...emptyFilters(),
		section: ['global', 's-1'],
		mobile: 'with' as const,
		other: 'without' as const,
		search: ' Maria ',
		sort: 'section' as const,
		order: 'desc' as const
	};
	const query = pageQuery(f, 'c-1');
	assert.equal(
		query,
		'?contact=c-1&section=global%2Cs-1&mobile=with&other=without&search=Maria&sort=section&order=desc'
	);
	assert.deepEqual(parseFilters(new URLSearchParams(query)), { ...f, search: 'Maria' });
	assert.deepEqual(
		parseFilters(new URLSearchParams('work=maybe&sort=phone&order=up&section=,,')),
		emptyFilters()
	);
});

test('unknown sections are removed after the load', () => {
	const f = { ...emptyFilters(), section: ['global', 's-1', 'gone'] };
	assert.deepEqual(knownSections(f, ['s-1']), ['global', 's-1']);
	assert.deepEqual(knownSections(f, []), [], 'without sections all contacts are global');
});

test('filters by section, number presence and free text', () => {
	const maria = contact('Maria', { phone_mobile: '+49 951 1234567', section_id: 's-1' });
	const firma = contact('Büro', { company: 'Zweig GmbH', phone_work: '0951 99' });
	const f = emptyFilters();
	assert.ok(matches(maria, f) && matches(firma, f));
	assert.ok(matches(maria, { ...f, section: ['s-1'] }));
	assert.ok(!matches(firma, { ...f, section: ['s-1'] }));
	assert.ok(matches(firma, { ...f, section: ['global'] }));
	assert.ok(matches(maria, { ...f, mobile: 'with' }));
	assert.ok(!matches(firma, { ...f, mobile: 'with' }));
	assert.ok(matches(firma, { ...f, mobile: 'without', work: 'with' }));
	assert.ok(matches(firma, { ...f, search: 'zweig' }), 'company');
	assert.ok(matches(maria, { ...f, search: '9511234' }), 'digits only');
	assert.ok(matches(maria, { ...f, search: 'familie' }, 'Familie'), 'section name');
	assert.ok(!matches(maria, { ...f, search: 'zweig' }));
});

test('sorts by name, company and section in both directions', () => {
	const list = [
		contact('bert', { company: 'Zeta', section_id: 's-b' }),
		contact('Anna'),
		contact('Carl', { company: 'Alpha', section_id: 's-a' }),
		contact('anton', { section_id: 's-a' })
	];
	const names = { 's-a': 'Arbeit', 's-b': 'Familie' } as Record<string, string>;
	const sectionName = (id: string | null) => (id ? names[id] : 'Global');
	const order = (sort: 'name' | 'company' | 'section', dir: 'asc' | 'desc') =>
		sortContacts(list, { sort, order: dir }, sectionName, 'de').map((c) => c.name);
	assert.deepEqual(order('name', 'asc'), ['Anna', 'anton', 'bert', 'Carl']);
	assert.deepEqual(order('name', 'desc'), ['Carl', 'bert', 'anton', 'Anna']);
	// Contacts without a company come last in both directions.
	assert.deepEqual(order('company', 'asc'), ['Carl', 'bert', 'Anna', 'anton']);
	assert.deepEqual(order('company', 'desc'), ['bert', 'Carl', 'Anna', 'anton']);
	assert.deepEqual(order('section', 'asc'), ['Anna', 'anton', 'Carl', 'bert']);
	assert.deepEqual(order('section', 'desc'), ['bert', 'anton', 'Carl', 'Anna']);
});
