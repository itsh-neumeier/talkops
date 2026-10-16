// Shared helpers for sortable and filterable lists (call list, phone book,
// transcription history). The functions have no browser dependencies, so the
// unit tests can import them.

export type Order = 'asc' | 'desc';

export interface SortState<K extends string> {
	sort: K;
	order: Order;
}

/**
 * The sort state after a click on the header of column `key`: the same
 * column changes the direction, a new column starts with `first`.
 */
export function nextSort<K extends string>(
	current: SortState<K>,
	key: K,
	first: Order = 'desc'
): SortState<K> {
	if (current.sort === key) {
		return { sort: key, order: current.order === 'desc' ? 'asc' : 'desc' };
	}
	return { sort: key, order: first };
}

/** Value of the `aria-sort` attribute of a column header. */
export function ariaSort(active: boolean, order: Order): 'ascending' | 'descending' | 'none' {
	if (!active) return 'none';
	return order === 'asc' ? 'ascending' : 'descending';
}

/** Reads the direction from the address; other values give `fallback`. */
export function parseOrder(raw: string | null, fallback: Order): Order {
	return raw === 'asc' || raw === 'desc' ? raw : fallback;
}

/**
 * Reads a comma-separated list from the address. Only known values are kept,
 * each value once.
 */
export function pickList<T extends string>(values: readonly T[], raw: string | null): T[] {
	return (raw ?? '')
		.split(',')
		.filter((v): v is T => (values as readonly string[]).includes(v))
		.filter((v, i, all) => all.indexOf(v) === i);
}

/** Adds the value to the list or removes it. */
export function toggle<T>(list: T[], value: T): T[] {
	return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
}

/** A query string with a leading `?`, or an empty string without parameters. */
export function queryString(params: URLSearchParams): string {
	const s = params.toString();
	return s ? `?${s}` : '';
}

/** Compares texts in the order of the user's language, numbers by value. */
export function textCompare(locale?: string): (a: string, b: string) => number {
	const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
	return (a, b) => collator.compare(a, b);
}
