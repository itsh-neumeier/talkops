/**
 * Whether the current page is still loading its data: skeletons are shown
 * until the first requests after a navigation have settled. Later polling
 * (dashboard, diagnostics) does not bring them back.
 */
import { untrack } from 'svelte';

export const net = $state({ pending: 0, settled: false });

let quiet: ReturnType<typeof setTimeout> | undefined;

// Requests also start inside effects (e.g. a debounced lookup): the counter
// must not become a dependency of the calling effect, or it re-runs forever.
export function requestStarted() {
	untrack(() => net.pending++);
}

export function requestFinished() {
	untrack(() => {
		net.pending = Math.max(0, net.pending - 1);
		// Settled once nothing new starts right after (the session check is
		// followed by the page's own requests).
		if (net.pending === 0) {
			setTimeout(() => {
				if (net.pending === 0) net.settled = true;
			}, 80);
		}
	});
}

/** A new page: skeletons until its data is there (or nothing is loaded). */
export function pageChanged() {
	net.settled = false;
	clearTimeout(quiet);
	quiet = setTimeout(() => {
		if (net.pending === 0) net.settled = true;
	}, 400);
}
