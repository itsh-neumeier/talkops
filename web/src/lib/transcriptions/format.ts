// Formats for the transcription page and the model dialog.
import type { EngineChoice } from '../api.ts';

/** A rough duration: `45 s`, `3 min`, `1 h 5 min`. */
export function formatEta(secs: number | null | undefined): string {
	if (secs == null || !Number.isFinite(secs)) return '—';
	const s = Math.max(0, Math.round(secs));
	if (s < 60) return `${Math.max(1, s)} s`;
	const m = Math.round(s / 60);
	if (m < 60) return `${m} min`;
	const h = Math.floor(m / 60);
	const rest = m % 60;
	return rest ? `${h} h ${rest} min` : `${h} h`;
}

/** Bytes as GB or MB with one decimal. */
export function formatBytes(bytes: number): string {
	if (!bytes || bytes < 0) return '0 MB';
	const gb = bytes / 1024 ** 3;
	return gb >= 1 ? `${gb.toFixed(1)} GB` : `${Math.round(bytes / 1024 ** 2)} MB`;
}

/** Price in USD with enough decimals for small amounts. */
export function formatUsd(usd: number): string {
	return `$${usd < 0.1 ? usd.toFixed(3) : usd.toFixed(2)}`;
}

/** Stable key of an engine choice (same as the server's speed key). */
export function choiceKey(c: EngineChoice): string {
	return c.kind === 'local' ? `local:${c.model}` : `cloud:${c.provider_id}:${c.model}`;
}

/** Share between 0 and 1 as a whole percent. */
export function percent(share: number | null | undefined): number {
	return Math.round(Math.min(1, Math.max(0, share ?? 0)) * 100);
}
