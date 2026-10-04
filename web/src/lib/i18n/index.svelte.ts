// Minimal i18n: typed message catalogs, locale persisted in localStorage.
// English is the reference catalog; other catalogs must provide every key.
import de from './de';
import en from './en';

export type MessageKey = keyof typeof en;
export const locales = { de, en } as const;
export type Locale = keyof typeof locales;

const STORAGE_KEY = 'talkops.locale';

function initialLocale(): Locale {
	try {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored && stored in locales) return stored as Locale;
	} catch {
		// storage unavailable
	}
	return navigator.language.toLowerCase().startsWith('de') ? 'de' : 'en';
}

export const i18n = $state({ locale: initialLocale() });

export function setLocale(locale: Locale) {
	i18n.locale = locale;
	document.documentElement.lang = locale;
	try {
		localStorage.setItem(STORAGE_KEY, locale);
	} catch {
		// storage unavailable
	}
}

/** Translates a key; `{name}` placeholders are replaced from `params`. */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
	let text: string = locales[i18n.locale][key] ?? en[key];
	for (const [name, value] of Object.entries(params ?? {})) {
		text = text.replaceAll(`{${name}}`, String(value));
	}
	return text;
}

export function formatDateTime(iso: string | null | undefined): string {
	if (!iso) return '—';
	return new Date(iso).toLocaleString(i18n.locale === 'de' ? 'de-DE' : 'en-GB', {
		dateStyle: 'short',
		timeStyle: 'short'
	});
}

export function formatDuration(secs: number): string {
	const m = Math.floor(secs / 60);
	const s = secs % 60;
	return `${m}:${String(s).padStart(2, '0')}`;
}
