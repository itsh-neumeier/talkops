// Keeps the filter and sort state of a list in the page address, so that a
// reload or a shared link shows the same list.
import { goto } from '$app/navigation';
import { page } from '$app/state';

/** Replaces the query string of the current page without a new history entry. */
export function syncAddress(path: string, query: string): void {
	if (query === page.url.search) return;
	goto(`${path}${query}`, { replace: true, shallow: true });
}
