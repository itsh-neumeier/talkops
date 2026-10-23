<script lang="ts">
	// Leaflet map with one bubble for each area (ADR 0044). Leaflet loads only
	// when this component mounts. Without a tile URL the base map is the
	// shipped country outline file, so the page sends no external request.
	import { onMount } from 'svelte';
	import type { Map as LeafletMap, LayerGroup } from 'leaflet';
	import type { MapArea } from '#lib/api.ts';
	import { bubbleGradient, bubbleSize, shownTotal, type Category } from '#lib/calls/map.ts';
	import { theme } from '#lib/theme.svelte.ts';
	import { motion } from '#lib/motion.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		areas,
		shown,
		selected = null,
		onselect,
		label,
		tileUrl = '',
		tileAttribution = '',
		compact = false
	}: {
		/** Areas with a position; others are not drawn. */
		areas: MapArea[];
		shown: Category[];
		selected?: string | null;
		onselect?: (area: MapArea) => void;
		/** Accessible name of the map. */
		label: string;
		/** Optional tile server (`https://…/{z}/{x}/{y}.png`); empty: offline base map. */
		tileUrl?: string;
		tileAttribution?: string;
		/** Small map without zoom buttons (dashboard). */
		compact?: boolean;
	} = $props();

	let el: HTMLDivElement | undefined = $state();
	let map: LeafletMap | null = $state(null);
	let bubbles: LayerGroup | null = null;
	let leaflet: typeof import('leaflet') | null = null;
	let fitted = false;
	let failed = $state(false);

	const ATTRIBUTION = [
		'© <a href="https://www.bundesnetzagentur.de/">Bundesnetzagentur</a>',
		'Source: <a href="https://www.rtr.at/">RTR-GmbH</a>',
		'<a href="https://www.geonames.org/">GeoNames</a> (CC BY 4.0)',
		'<a href="https://github.com/google/libphonenumber">libphonenumber</a>',
		'<a href="https://www.naturalearthdata.com/">Natural Earth</a>'
	].join(' · ');

	function escapeHtml(text: string): string {
		return text.replace(
			/[&<>"']/g,
			(c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!
		);
	}

	onMount(() => {
		let disposed = false;
		(async () => {
			try {
				const mod = await import('leaflet');
				await import('leaflet/dist/leaflet.css');
				if (disposed || !el) return;
				const L = (mod as unknown as { default?: typeof import('leaflet') }).default ?? mod;
				leaflet = L;
				const animate = motion(1) > 0;
				const m = L.map(el, {
					center: [50, 10],
					zoom: compact ? 3 : 4,
					minZoom: 1,
					maxZoom: tileUrl ? 18 : 10,
					zoomSnap: 0.5,
					worldCopyJump: true,
					zoomControl: !compact,
					zoomAnimation: animate,
					fadeAnimation: animate,
					markerZoomAnimation: animate
				});
				m.attributionControl.setPrefix('<a href="https://leafletjs.com/">Leaflet</a>');
				m.attributionControl.addAttribution(ATTRIBUTION);
				if (tileUrl) {
					L.tileLayer(tileUrl, {
						attribution: escapeHtml(tileAttribution),
						maxZoom: 18,
						crossOrigin: false
					}).addTo(m);
				} else {
					const res = await fetch('/map/countries.geojson');
					if (!res.ok) throw new Error(String(res.status));
					const data = await res.json();
					if (disposed) {
						m.remove();
						return;
					}
					L.geoJSON(data, {
						interactive: false,
						style: () => ({
							className: 'call-map-country',
							weight: 0.6,
							fillOpacity: 1
						})
					}).addTo(m);
				}
				bubbles = L.layerGroup().addTo(m);
				map = m;
			} catch {
				if (!disposed) failed = true;
			}
		})();
		return () => {
			disposed = true;
			map?.remove();
			map = null;
		};
	});

	function areaLabel(a: MapArea, count: number): string {
		return t('callMap.bubbleLabel', { name: a.name, prefix: a.prefix, count });
	}

	// Draws the bubbles again when data, categories, selection or theme change.
	$effect(() => {
		const m = map;
		const L = leaflet;
		const dark = theme.dark;
		const sel = selected;
		const located = areas.filter((a) => a.lat !== null && a.lon !== null);
		const counts = located.map((a) => shownTotal(a.counts, shown));
		if (!m || !L || !bubbles) return;
		bubbles.clearLayers();
		const max = Math.max(0, ...counts);
		// Large bubbles first, so small bubbles stay on top and clickable.
		const order = located
			.map((a, i) => ({ a, n: counts[i] }))
			.filter((x) => x.n > 0)
			.sort((x, y) => y.n - x.n);
		for (const { a, n } of order) {
			const size = bubbleSize(n, max, compact ? 10 : 14, compact ? 36 : 56);
			const icon = L.divIcon({
				className: `call-bubble${a.id === sel ? ' call-bubble-selected' : ''}${a.kind === 'none' ? ' call-bubble-none' : ''}`,
				iconSize: [size, size],
				html: ''
			});
			const marker = L.marker([a.lat!, a.lon!], {
				icon,
				keyboard: !compact,
				interactive: !!onselect,
				riseOnHover: true
			});
			marker.on('add', () => {
				const node = marker.getElement();
				if (!node) return;
				node.style.background = bubbleGradient(a.counts, shown, dark);
				node.setAttribute('aria-label', areaLabel(a, n));
				node.title = areaLabel(a, n);
				if (onselect && !compact) {
					node.setAttribute('role', 'button');
					node.setAttribute('aria-pressed', String(a.id === sel));
					node.addEventListener('keydown', (e) => {
						if (e.key === 'Enter' || e.key === ' ') {
							e.preventDefault();
							onselect(a);
						}
					});
				} else {
					node.removeAttribute('tabindex');
				}
			});
			if (onselect) marker.on('click', () => onselect(a));
			marker.addTo(bubbles);
		}
		if (!fitted && order.length && !compact) {
			fitted = true;
			const bounds = L.latLngBounds(order.map(({ a }) => [a.lat!, a.lon!] as [number, number]));
			m.fitBounds(bounds.pad(0.3), { maxZoom: 7, animate: false });
		}
	});
</script>

<div class="relative">
	<div
		bind:this={el}
		role="region"
		aria-label={label}
		class="call-map w-full overflow-hidden rounded-lg {compact
			? 'h-56'
			: 'h-[min(32rem,60dvh)] min-h-72'}"
	></div>
	{#if !map && !failed}
		<div class="skeleton absolute inset-0 rounded-lg" aria-hidden="true"></div>
	{/if}
	{#if failed}
		<p class="absolute inset-0 grid place-items-center text-sm text-slate-500 dark:text-slate-400">
			{t('callMap.loadFailed')}
		</p>
	{/if}
</div>
