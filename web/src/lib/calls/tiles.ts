// Tile server setting of the call map, loaded once for each page view.
import { api, type MapTiles } from '../api.ts';

let pending: Promise<MapTiles> | null = null;

/** The tile server setting; the offline map when the request fails. */
export function loadTiles(): Promise<MapTiles> {
	pending ??= api
		.get<Partial<MapTiles>>('/settings/call-map')
		.then((v) => ({ tile_url: v?.tile_url ?? '', attribution: v?.attribution ?? '' }))
		.catch(() => {
			pending = null;
			return { tile_url: '', attribution: '' };
		});
	return pending;
}

/** Forgets the loaded setting after a change. */
export function resetTiles(): void {
	pending = null;
}
