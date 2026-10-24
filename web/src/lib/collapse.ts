// Short lists on the dashboard: show the first entries and expand the rest
// on request. The open state is a per-browser convenience (localStorage).

/** Entries of a collapsed dashboard list. */
export const DASH_LIMIT = 3;

/**
 * The visible entries: all when open, else the first `limit` entries and
 * each later entry for which `keep` is true (for example a ringing call).
 */
export function visibleItems<T>(
	items: T[],
	open: boolean,
	limit = DASH_LIMIT,
	keep: (item: T) => boolean = () => false
): T[] {
	if (open) return items;
	return items.filter((item, i) => i < limit || keep(item));
}

/** Reads a stored open state; false without storage. */
export function readOpen(key: string): boolean {
	try {
		return localStorage.getItem(key) === '1';
	} catch {
		return false;
	}
}

/** Stores an open state; does nothing without storage. */
export function writeOpen(key: string, open: boolean): void {
	try {
		if (open) localStorage.setItem(key, '1');
		else localStorage.removeItem(key);
	} catch {
		// Private window or blocked storage: the state is not kept.
	}
}
