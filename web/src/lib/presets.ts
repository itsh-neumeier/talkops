import type { Preset } from './api.ts';
import { i18n } from './i18n/index.svelte.ts';
import type { MessageKey } from './i18n/index.svelte.ts';

export function presetNotes(p: Preset): string {
	return (i18n.locale === 'de' ? p.notes.de : p.notes.en) || p.notes.en;
}

export function presetHint(p: Preset): string {
	const h = p.credentials.username_hint;
	return (i18n.locale === 'de' ? h.de : h.en) || h.en;
}

export const statusKey: Record<Preset['status'], MessageKey> = {
	verified: 'trunks.statusVerified',
	community: 'trunks.statusCommunity',
	untested: 'trunks.statusUntested'
};

export const statusClass: Record<Preset['status'], string> = {
	verified: 'badge-ok',
	community: 'badge-ok',
	untested: 'badge-warn'
};

export function presetLabel(p: Preset): string {
	return p.product ? `${p.name} – ${p.product}` : p.name;
}
