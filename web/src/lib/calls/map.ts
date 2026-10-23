// State and helpers of the call origin map (ADR 0044). No browser
// dependencies, so the unit tests can import this module.
import { pickList, queryString } from '../list/sort.ts';

export type MapRange = '1h' | '1d' | '1w' | '1m';
export type Category = 'answered' | 'missed' | 'outbound' | 'blocked';
export type CallsView = 'list' | 'map';

export const MAP_RANGES: MapRange[] = ['1h', '1d', '1w', '1m'];
export const CATEGORIES: Category[] = ['answered', 'missed', 'outbound', 'blocked'];

/** Length of a range in milliseconds (rolling, 1 month = 30 days). */
export const RANGE_MS: Record<MapRange, number> = {
	'1h': 3_600_000,
	'1d': 86_400_000,
	'1w': 7 * 86_400_000,
	'1m': 30 * 86_400_000
};

/** Category colors: fill (light theme) and fill (dark theme). */
export const CATEGORY_COLORS: Record<Category, { light: string; dark: string }> = {
	answered: { light: '#059669', dark: '#34d399' },
	missed: { light: '#dc2626', dark: '#f87171' },
	outbound: { light: '#2563eb', dark: '#60a5fa' },
	blocked: { light: '#7c3aed', dark: '#a78bfa' }
};

export type CategoryCounts = Record<Category, number>;

export interface MapState {
	view: CallsView;
	range: MapRange;
	/** Shown categories; all when the address has none. */
	categories: Category[];
}

export function defaultMapState(): MapState {
	return { view: 'list', range: '1d', categories: [...CATEGORIES] };
}

/** Reads view, range and categories from the page address. */
export function parseMapState(params: { get(name: string): string | null }): MapState {
	const s = defaultMapState();
	s.view = params.get('view') === 'map' ? 'map' : 'list';
	const range = params.get('range') ?? '';
	if ((MAP_RANGES as string[]).includes(range)) s.range = range as MapRange;
	const categories = pickList(CATEGORIES, params.get('categories'));
	if (categories.length) s.categories = categories;
	return s;
}

/** The query string of the map view; defaults are left out. */
export function mapPageQuery(s: MapState): string {
	const p = new URLSearchParams();
	p.set('view', 'map');
	if (s.range !== '1d') p.set('range', s.range);
	if (s.categories.length && s.categories.length < CATEGORIES.length) {
		p.set('categories', CATEGORIES.filter((c) => s.categories.includes(c)).join(','));
	}
	return queryString(p);
}

/** The API request of the map. */
export function mapApiQuery(s: Pick<MapState, 'range' | 'categories'>): string {
	const p = new URLSearchParams();
	p.set('range', s.range);
	if (s.categories.length && s.categories.length < CATEGORIES.length) {
		p.set('categories', CATEGORIES.filter((c) => s.categories.includes(c)).join(','));
	}
	return `/calls/map?${p}`;
}

/**
 * Link from a map bubble to the call list: the same rolling range and only
 * the calls of the area. `place` is the label of the filter.
 */
export function listHref(range: MapRange, area: string, place: string): string {
	const p = new URLSearchParams({ range, area });
	if (place) p.set('place', place);
	return `/calls?${p}`;
}

/** Link from the dashboard widget to the map view. */
export function mapHref(range: MapRange): string {
	return `/calls${mapPageQuery({ view: 'map', range, categories: [...CATEGORIES] })}`;
}

/** Total of the shown categories. */
export function shownTotal(counts: CategoryCounts, shown: Category[]): number {
	return shown.reduce((sum, c) => sum + (counts[c] ?? 0), 0);
}

/**
 * Bubble diameter in pixels: the area grows with the number of calls.
 * `max` is the largest count on the map.
 */
export function bubbleSize(count: number, max: number, min = 14, largest = 56): number {
	if (count <= 0 || max <= 0) return 0;
	const ratio = Math.sqrt(count / max);
	return Math.round(min + (largest - min) * ratio);
}

/**
 * CSS `conic-gradient` of the category segments of a bubble, in the order of
 * `CATEGORIES`. A bubble with one category has one color.
 */
export function bubbleGradient(counts: CategoryCounts, shown: Category[], dark = false): string {
	const total = shownTotal(counts, shown);
	if (total <= 0) return 'transparent';
	const parts: string[] = [];
	let start = 0;
	for (const c of CATEGORIES) {
		if (!shown.includes(c) || !counts[c]) continue;
		const end = start + (counts[c] / total) * 360;
		const color = dark ? CATEGORY_COLORS[c].dark : CATEGORY_COLORS[c].light;
		parts.push(`${color} ${start.toFixed(1)}deg ${end.toFixed(1)}deg`);
		start = end;
	}
	return `conic-gradient(${parts.join(', ')})`;
}

/** Category of a call record, as the server counts it. */
export function callCategory(c: {
	direction: string;
	answered_at: string | null;
	blocked_reason?: string | null;
}): Category {
	if (c.direction === 'outbound') return 'outbound';
	if (c.blocked_reason) return 'blocked';
	return c.answered_at ? 'answered' : 'missed';
}

/**
 * Checks a tile URL like the server: `https`, the placeholders `{z}`, `{x}`
 * and `{y}`, only known placeholders. Returns an error key or `''`.
 */
export function tileUrlProblem(
	url: string | null | undefined
): '' | 'https' | 'placeholders' | 'invalid' {
	const value = (url ?? '').trim();
	if (!value) return '';
	if (!value.startsWith('https://')) return 'https';
	if (!['{z}', '{x}', '{y}'].every((p) => value.includes(p))) return 'placeholders';
	const rest = ['{z}', '{x}', '{y}', '{s}', '{r}'].reduce((v, p) => v.replaceAll(p, ''), value);
	if (/[{}\s'"<>\;,`^|]/.test(rest) || rest.length > 500) return 'invalid';
	return '';
}

/** Top areas with a location for a compact list (dashboard). */
export function topAreas<T extends { kind: string; counts: CategoryCounts }>(
	areas: T[],
	shown: Category[],
	count = 5
): T[] {
	return areas
		.filter((a) => a.kind === 'area' && shownTotal(a.counts, shown) > 0)
		.sort((a, b) => shownTotal(b.counts, shown) - shownTotal(a.counts, shown))
		.slice(0, count);
}
