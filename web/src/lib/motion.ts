// Durations for Svelte transitions; zero when the user prefers reduced motion.

/** The duration in ms, or 0 with `prefers-reduced-motion: reduce`. */
export function motion(ms: number): number {
	if (typeof matchMedia !== 'function') return ms;
	return matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : ms;
}
