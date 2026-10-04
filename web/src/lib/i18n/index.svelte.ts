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

export function t(key: MessageKey): string {
	return locales[i18n.locale][key] ?? en[key];
}
