// The side panel of a list page keeps the open entry in `?panel=<id>`, so a
// reload or a shared link opens it again.
import { replaceState } from '$app/navigation';
import { page } from '$app/state';

export function panelParam(): string | null {
	return page.url.searchParams.get('panel');
}

export function setPanelParam(id: string | null): void {
	const url = new URL(page.url.href);
	if (id) url.searchParams.set('panel', id);
	else url.searchParams.delete('panel');
	replaceState(url, page.state);
}

/**
 * Click on a list row: opens the panel, except for a click on a control in
 * the row or a click that opens a link in a new tab.
 */
export function rowClick(e: MouseEvent, open: () => void): void {
	if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
	const el = e.target as HTMLElement;
	const control = el.closest('a, button, input, select, textarea, label');
	if (control && !control.hasAttribute('data-panel-link')) return;
	e.preventDefault();
	open();
}
