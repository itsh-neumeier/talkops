import { ApiError } from './api.ts';

/** Readable message for an API error. */
export function errorMessage(err: unknown): string {
	if (err instanceof ApiError) return err.message;
	if (err instanceof Error) return err.message;
	return String(err);
}

/** Copy text to the clipboard; false if the browser refused. */
export async function copy(text: string): Promise<boolean> {
	try {
		await navigator.clipboard.writeText(text);
		return true;
	} catch {
		// Clipboard API needs HTTPS; fall back to the legacy copy command,
		// which also works on plain-HTTP LAN addresses.
		const area = document.createElement('textarea');
		area.value = text;
		area.setAttribute('readonly', '');
		area.style.position = 'fixed';
		area.style.opacity = '0';
		document.body.appendChild(area);
		area.select();
		try {
			return document.execCommand('copy');
		} catch {
			return false;
		} finally {
			area.remove();
		}
	}
}
