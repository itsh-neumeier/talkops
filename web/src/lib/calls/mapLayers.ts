// Detail layers of the offline call map (ADR 0044): which layer shows at
// which zoom level, and the placement of place labels without overlap.
// No browser dependencies, so the unit tests can import this module.

export type LayerId = 'admin1' | 'rivers' | 'lakes' | 'places' | 'onb';

export interface LayerSpec {
	id: LayerId;
	/** First zoom level at which the layer shows. */
	minZoom: number;
	/** Shown only with the offline base map (a tile server has its own). */
	offlineOnly: boolean;
}

/** Highest zoom level of the offline base map. */
export const OFFLINE_MAX_ZOOM = 11;

export const LAYERS: LayerSpec[] = [
	{ id: 'admin1', minZoom: 5, offlineOnly: true },
	{ id: 'lakes', minZoom: 5, offlineOnly: true },
	{ id: 'rivers', minZoom: 5, offlineOnly: true },
	{ id: 'places', minZoom: 5, offlineOnly: true },
	{ id: 'onb', minZoom: 8, offlineOnly: false }
];

/** Place files (`/map/places-<z>.json`) by their first zoom level. */
export const PLACE_ZOOMS = [5, 7, 8, 10];

/** The layers to show at a zoom level. */
export function visibleLayers(zoom: number, tiles: boolean): LayerId[] {
	return LAYERS.filter((l) => zoom >= l.minZoom && !(tiles && l.offlineOnly)).map((l) => l.id);
}

/** The place files that a zoom level needs. */
export function placeFiles(zoom: number): string[] {
	return PLACE_ZOOMS.filter((z) => zoom >= z).map((z) => `/map/places-${z}.json`);
}

/** Water features (rivers, lakes) carry their first zoom level in `z`. */
export function waterVisible(featureZoom: number | undefined, zoom: number): boolean {
	return zoom >= (featureZoom ?? 7);
}

/** A place: name, latitude, longitude, population, German name if different. */
export type Place = [string, number, number, number, string?];

/** The label text of a place in the UI language. */
export function placeName(place: Place, locale: string): string {
	return locale === 'de' && place[4] ? place[4] : place[0];
}

export interface Label {
	place: Place;
	name: string;
	x: number;
	y: number;
	/** Large cities get a stronger label. */
	major: boolean;
}

/** Approximate size of a label in pixels. */
export function labelBox(name: string, major: boolean): { w: number; h: number } {
	return { w: Math.ceil(name.length * (major ? 7.2 : 6.2)) + 8, h: major ? 16 : 14 };
}

/**
 * Chooses the labels to draw: larger places first, no label overlaps
 * another, at most `limit` labels. `project` gives the screen position of a
 * place or `null` when it is outside the view.
 */
export function placeLabels(
	places: Place[],
	project: (lat: number, lon: number) => { x: number; y: number } | null,
	limit = 250,
	locale = 'en'
): Label[] {
	const sorted = [...places].sort((a, b) => b[3] - a[3]);
	const taken: { x0: number; y0: number; x1: number; y1: number }[] = [];
	const out: Label[] = [];
	for (const place of sorted) {
		if (out.length >= limit) break;
		const p = project(place[1], place[2]);
		if (!p) continue;
		const major = place[3] >= 500_000;
		const { w, h } = labelBox(placeName(place, locale), major);
		const box = { x0: p.x - w / 2, y0: p.y - h, x1: p.x + w / 2, y1: p.y + 4 };
		if (taken.some((b) => box.x0 < b.x1 && box.x1 > b.x0 && box.y0 < b.y1 && box.y1 > b.y0)) {
			continue;
		}
		taken.push(box);
		out.push({ place, name: placeName(place, locale), x: p.x, y: p.y, major });
	}
	return out;
}

/** The local network area of a bubble with an outline (Germany only). */
export function onbArea(id: string | null | undefined): string | null {
	return id && /^DE-\d+$/.test(id) ? id : null;
}

/** Line and fill colors of the detail layers for each theme. */
export function layerColors(dark: boolean) {
	return dark
		? {
				admin1: '#64748b',
				water: '#1e3a5f',
				waterLine: '#335f8f',
				onb: '#334155',
				onbSelected: '#2dd4bf'
			}
		: {
				admin1: '#94a3b8',
				water: '#bfdbfe',
				waterLine: '#7fb2e5',
				onb: '#e2e8f0',
				onbSelected: '#0d9488'
			};
}
