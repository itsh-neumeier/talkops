import { ApiError } from './api.ts';

/** Readable message for an API error. */
export function errorMessage(err: unknown): string {
	if (err instanceof ApiError) return err.message;
	if (err instanceof Error) return err.message;
	return String(err);
}

export async function copy(text: string) {
	try {
		await navigator.clipboard.writeText(text);
	} catch {
		// clipboard unavailable (insecure context); the value is visible anyway
	}
}
