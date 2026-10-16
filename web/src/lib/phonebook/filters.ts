// Filter and sort state of the phone book page, kept in the address bar.
// The page loads all contacts, so the filters and the sort run in the browser.
import { digitsMatch, parseOrder, queryString, textCompare, type Order } from '../list/sort.ts';

export type SortKey = 'name' | 'company' | 'section';
/** Phone number fields of a contact. */
export type NumberKind = 'work' | 'mobile' | 'other';
/** '' = all contacts, 'with' = the number is set, 'without' = it is empty. */
export type Presence = '' | 'with' | 'without';

export const SORT_KEYS: SortKey[] = ['name', 'company', 'section'];
export const NUMBER_KINDS: NumberKind[] = ['work', 'mobile', 'other'];
export const PRESENCES: Presence[] = ['', 'with', 'without'];
/** Filter value of the global phone book (contacts without a section). */
export const GLOBAL = 'global';

export interface PhonebookFilters {
	/** `GLOBAL` and section ids; empty = all phone books. */
	section: string[];
	work: Presence;
	mobile: Presence;
	other: Presence;
	search: string;
	sort: SortKey;
	order: Order;
}

/** The contact fields that the filters and the sort use. */
export interface ContactRow {
	name: string;
	company: string;
	phone_work: string;
	phone_mobile: string;
	phone_other: string;
	section_id: string | null;
}

export function emptyFilters(): PhonebookFilters {
	return {
		section: [],
		work: '',
		mobile: '',
		other: '',
		search: '',
		sort: 'name',
		order: 'asc'
	};
}

const presence = (raw: string | null): Presence => (raw === 'with' || raw === 'without' ? raw : '');

/** Reads the filters from the query string of the page address. */
export function parseFilters(params: { get(name: string): string | null }): PhonebookFilters {
	const f = emptyFilters();
	f.section = (params.get('section') ?? '')
		.split(',')
		.map((v) => v.trim())
		.filter((v, i, all) => v !== '' && all.indexOf(v) === i);
	f.work = presence(params.get('work'));
	f.mobile = presence(params.get('mobile'));
	f.other = presence(params.get('other'));
	f.search = params.get('search') ?? '';
	const sort = params.get('sort') ?? '';
	f.sort = (SORT_KEYS as string[]).includes(sort) ? (sort as SortKey) : 'name';
	f.order = parseOrder(params.get('order'), 'asc');
	return f;
}

/**
 * The query string of the page address; defaults are left out. `contact`
 * keeps the link to a highlighted entry (`?contact=<id>`).
 */
export function pageQuery(f: PhonebookFilters, contact = ''): string {
	const p = new URLSearchParams();
	if (contact) p.set('contact', contact);
	if (f.section.length) p.set('section', f.section.join(','));
	for (const kind of NUMBER_KINDS) if (f[kind]) p.set(kind, f[kind]);
	if (f.search.trim()) p.set('search', f.search.trim());
	if (f.sort !== 'name') p.set('sort', f.sort);
	if (f.order !== 'asc') p.set('order', f.order);
	return queryString(p);
}

/** Removes sections that do not exist (deleted, or a wrong link). */
export function knownSections(f: PhonebookFilters, sectionIds: string[]): string[] {
	if (sectionIds.length === 0) return [];
	return f.section.filter((s) => s === GLOBAL || sectionIds.includes(s));
}

/** Active filters of each column (for the filter mark). */
export function activeFilters(f: PhonebookFilters): Record<string, boolean> {
	return {
		section: f.section.length > 0,
		work: !!f.work,
		mobile: !!f.mobile,
		other: !!f.other
	};
}

export function anyFilter(f: PhonebookFilters): boolean {
	return Object.values(activeFilters(f)).some(Boolean) || !!f.search.trim();
}

/** True if the contact matches the free text (name, company or a number). */
export function matchesSearch(c: ContactRow, search: string, sectionName = ''): boolean {
	const q = search.trim().toLowerCase();
	if (!q) return true;
	const numbers = [c.phone_work, c.phone_mobile, c.phone_other];
	if ([c.name, c.company, sectionName, ...numbers].some((v) => v.toLowerCase().includes(q))) {
		return true;
	}
	// "0951 123" also finds "+49 951 123…" when the text is a number.
	return digitsMatch(q, numbers);
}

function hasNumber(value: string, want: Presence): boolean {
	if (want === 'with') return value.trim() !== '';
	if (want === 'without') return value.trim() === '';
	return true;
}

/** True if the contact matches all filters. */
export function matches(c: ContactRow, f: PhonebookFilters, sectionName = ''): boolean {
	if (f.section.length > 0 && !f.section.includes(c.section_id ?? GLOBAL)) return false;
	return (
		hasNumber(c.phone_work, f.work) &&
		hasNumber(c.phone_mobile, f.mobile) &&
		hasNumber(c.phone_other, f.other) &&
		matchesSearch(c, f.search, sectionName)
	);
}

/**
 * Sorts a copy of the list. Contacts without a company come last in both
 * directions. The global phone book comes before the sections in ascending
 * order. Equal values are sorted by name.
 */
export function sortContacts<T extends ContactRow>(
	list: readonly T[],
	f: Pick<PhonebookFilters, 'sort' | 'order'>,
	sectionName: (id: string | null) => string,
	locale?: string
): T[] {
	const cmp = textCompare(locale);
	const dir = f.order === 'asc' ? 1 : -1;
	const byName = (a: T, b: T) => cmp(a.name, b.name);
	return [...list].sort((a, b) => {
		let r = 0;
		if (f.sort === 'company') {
			const ea = a.company.trim() === '';
			const eb = b.company.trim() === '';
			if (ea !== eb) return ea ? 1 : -1;
			r = dir * cmp(a.company, b.company);
		} else if (f.sort === 'section') {
			const ga = a.section_id === null;
			const gb = b.section_id === null;
			r =
				dir *
				(ga !== gb ? (ga ? -1 : 1) : cmp(sectionName(a.section_id), sectionName(b.section_id)));
		} else {
			r = dir * byName(a, b);
		}
		return r || byName(a, b);
	});
}
