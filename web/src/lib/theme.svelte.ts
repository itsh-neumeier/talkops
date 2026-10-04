const STORAGE_KEY = 'talkops.theme';

export const theme = $state({ dark: document.documentElement.classList.contains('dark') });

export function toggleTheme() {
	theme.dark = !theme.dark;
	document.documentElement.classList.toggle('dark', theme.dark);
	try {
		localStorage.setItem(STORAGE_KEY, theme.dark ? 'dark' : 'light');
	} catch {
		// storage unavailable
	}
}
