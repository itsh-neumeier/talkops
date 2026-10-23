// Detail layers of the call map in the browser (ADR 0044): state borders,
// rivers, lakes, place labels and German local network areas. Each file
// loads the first time its zoom level is reached.
import type * as Leaflet from 'leaflet';
import {
	layerColors,
	onbArea,
	placeFiles,
	placeLabels,
	visibleLayers,
	waterVisible,
	type LayerId,
	type Place
} from './mapLayers.ts';

type L = typeof Leaflet;

const FILES: Record<Exclude<LayerId, 'places'>, string> = {
	admin1: '/map/admin1.geojson',
	rivers: '/map/rivers.geojson',
	lakes: '/map/lakes.geojson',
	onb: '/map/onb.geojson'
};

const cache = new Map<string, Promise<unknown>>();

/** Loads a static map file once for each page view. */
function load<T>(url: string): Promise<T> {
	let p = cache.get(url);
	if (!p) {
		p = fetch(url).then((r) => {
			if (!r.ok) throw new Error(`${url}: ${r.status}`);
			return r.json();
		});
		p.catch(() => cache.delete(url));
		cache.set(url, p);
	}
	return p as Promise<T>;
}

export interface DetailOptions {
	/** A tile server draws the background; only the ONB layer stays. */
	tiles: boolean;
	dark: boolean;
	/** UI language for place names. */
	locale: string;
}

export class MapDetail {
	private layers = new Map<LayerId, Leaflet.GeoJSON>();
	private labels: Leaflet.LayerGroup;
	private places = new Map<string, Place[]>();
	private highlight: Leaflet.GeoJSON | null = null;
	private selected: string | null = null;
	private disposed = false;

	constructor(
		private L: L,
		private map: Leaflet.Map,
		private opts: DetailOptions
	) {
		// Panes order the layers: land < water < borders < ONB < labels < bubbles.
		for (const [name, z] of [
			['cm-water', 390],
			['cm-admin', 395],
			['cm-onb', 405],
			['cm-labels', 450]
		] as const) {
			const pane = map.createPane(name);
			pane.style.zIndex = String(z);
			if (name === 'cm-labels' || name === 'cm-onb') pane.style.pointerEvents = 'none';
		}
		this.labels = L.layerGroup([], { pane: 'cm-labels' }).addTo(map);
		map.on('zoomend', this.update);
		map.on('moveend', this.drawLabels);
		this.update();
	}

	dispose() {
		this.disposed = true;
		this.map.off('zoomend', this.update);
		this.map.off('moveend', this.drawLabels);
	}

	setLocale(locale: string) {
		if (locale === this.opts.locale) return;
		this.opts.locale = locale;
		this.drawLabels();
	}

	setTheme(dark: boolean) {
		this.opts.dark = dark;
		for (const [id, layer] of this.layers) layer.setStyle(this.style(id));
		this.highlight?.setStyle(this.highlightStyle());
	}

	/** Colors the local network area of the selected bubble. */
	async select(id: string | null) {
		this.selected = onbArea(id);
		this.highlight?.remove();
		this.highlight = null;
		if (!this.selected) return;
		const wanted = this.selected;
		try {
			const data = await load<GeoJSON.FeatureCollection>(FILES.onb);
			if (this.disposed || this.selected !== wanted) return;
			const feature = data.features.find((f) => f.properties?.id === wanted);
			if (!feature) return;
			this.highlight = this.L.geoJSON(feature, {
				pane: 'cm-onb',
				interactive: false,
				style: () => this.highlightStyle()
			}).addTo(this.map);
		} catch {
			// The area outline is optional.
		}
	}

	private highlightStyle(): Leaflet.PathOptions {
		const c = layerColors(this.opts.dark);
		return { color: c.onbSelected, weight: 2, fillColor: c.onbSelected, fillOpacity: 0.18 };
	}

	private style(id: LayerId): Leaflet.StyleFunction {
		const c = layerColors(this.opts.dark);
		const zoom = this.map.getZoom();
		switch (id) {
			case 'admin1':
				return () => ({ color: c.admin1, weight: 0.8, dashArray: '4 3', fill: false });
			case 'rivers':
				return (f) => ({
					color: c.waterLine,
					weight: zoom >= 8 ? 1.4 : 1,
					opacity: waterVisible(f?.properties?.z, zoom) ? 1 : 0,
					fill: false
				});
			case 'lakes':
				return (f) => {
					const on = waterVisible(f?.properties?.z, zoom);
					return {
						color: c.waterLine,
						weight: 0.5,
						fillColor: c.water,
						fillOpacity: on ? 1 : 0,
						opacity: on ? 1 : 0
					};
				};
			default:
				return () => ({ color: c.onb, weight: 0.7, fill: false });
		}
	}

	private update = () => {
		const zoom = this.map.getZoom();
		const wanted = new Set(visibleLayers(zoom, this.opts.tiles));
		for (const id of ['admin1', 'lakes', 'rivers', 'onb'] as const) {
			const layer = this.layers.get(id);
			if (wanted.has(id)) {
				if (layer) {
					if (!this.map.hasLayer(layer)) layer.addTo(this.map);
					layer.setStyle(this.style(id));
				} else {
					void this.add(id);
				}
			} else if (layer && this.map.hasLayer(layer)) {
				layer.remove();
			}
		}
		if (wanted.has('places')) void this.loadPlaces(zoom);
		this.drawLabels();
	};

	private async add(id: Exclude<LayerId, 'places'>) {
		if (this.layers.has(id)) return;
		try {
			const data = await load<GeoJSON.FeatureCollection>(FILES[id]);
			if (this.disposed || this.layers.has(id)) return;
			const pane = id === 'onb' ? 'cm-onb' : id === 'admin1' ? 'cm-admin' : 'cm-water';
			const layer = this.L.geoJSON(data, {
				pane,
				interactive: false,
				renderer: this.L.canvas({ pane }),
				style: this.style(id)
			} as Leaflet.GeoJSONOptions);
			this.layers.set(id, layer);
			if (visibleLayers(this.map.getZoom(), this.opts.tiles).includes(id)) layer.addTo(this.map);
		} catch {
			// A missing detail file leaves the countries map usable.
		}
	}

	private async loadPlaces(zoom: number) {
		const missing = placeFiles(zoom).filter((f) => !this.places.has(f));
		if (!missing.length) return;
		await Promise.all(
			missing.map(async (file) => {
				try {
					const data = await load<{ places: Place[] }>(file);
					this.places.set(file, data.places);
				} catch {
					// Labels are optional.
				}
			})
		);
		if (!this.disposed) this.drawLabels();
	}

	private drawLabels = () => {
		if (this.disposed) return;
		this.labels.clearLayers();
		const zoom = this.map.getZoom();
		if (!visibleLayers(zoom, this.opts.tiles).includes('places')) return;
		const bounds = this.map.getBounds().pad(0.05);
		const candidates: Place[] = [];
		for (const file of placeFiles(zoom)) {
			for (const p of this.places.get(file) ?? []) {
				if (bounds.contains([p[1], p[2]])) candidates.push(p);
			}
		}
		const size = this.map.getSize();
		const labels = placeLabels(
			candidates,
			(lat, lon) => {
				const pt = this.map.latLngToContainerPoint([lat, lon]);
				return pt.x < 0 || pt.y < 0 || pt.x > size.x || pt.y > size.y ? null : pt;
			},
			250,
			this.opts.locale
		);
		for (const label of labels) {
			const el = document.createElement('span');
			el.textContent = label.name;
			this.L.marker([label.place[1], label.place[2]], {
				pane: 'cm-labels',
				interactive: false,
				keyboard: false,
				icon: this.L.divIcon({
					className: `call-map-place${label.major ? ' call-map-place-major' : ''}`,
					html: el,
					iconSize: undefined
				})
			}).addTo(this.labels);
		}
	};
}
