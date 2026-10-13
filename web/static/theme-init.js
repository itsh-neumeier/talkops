// Applies the stored theme before first paint to avoid a flash. Loaded as a
// file (not inline) so the Content Security Policy allows it via 'self'.
try {
	const stored = localStorage.getItem('talkops.theme');
	const dark = stored ? stored === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
	document.documentElement.classList.toggle('dark', dark);
} catch {
	// storage unavailable: keep the light default
}
