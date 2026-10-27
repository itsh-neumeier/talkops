<script lang="ts">
	// Leaflet map with one bubble for each area (ADR 0044). Leaflet loads only
	// when this component mounts. Without a tile URL the base map is the
	// shipped country outline file, so the page sends no external request.
	import { onMount } from 'svelte';
	import type { Map as LeafletMap, LayerGroup } from 'leaflet';
	import type { MapArea } from '#lib/api.ts';
	import { bubbleSize, shownTotal, type Category, type MapRange } from '#lib/calls/map.ts';
	import {
		decorationsFit,
		escapeHtml,
		markerHtml,
		markerKind,
		markerName
	} from '#lib/calls/mapMarkers.ts';
	import CallMapPreview from '#lib/components/CallMapPreview.svelte';
	import { OFFLINE_MAX_ZOOM } from '#lib/calls/mapLayers.ts';
	import type { MapDetail } from '#lib/calls/mapDetail.ts';
	import { theme } from '#lib/theme.svelte.ts';
	import { motion } from '#lib/motion.ts';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';

	let {
		areas,
		shown,
		selected = null,
		onselect,
		label,
		tileUrl = '',
		tileAttribution = '',
		compact = false,
		range = '1d'
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
		/** Small map without mouse wheel zoom (dashboard). */
		compact?: boolean;
		range?: MapRange;
	} = $props();

	let el: HTMLDivElement | undefined = $state();
	let map: LeafletMap | null = $state(null);
	let bubbles: LayerGroup | null = null;
	let leaflet: typeof import('leaflet') | null = null;
	let fitted = false;
	/** State borders, water, places and ONB areas. */
	let detail: MapDetail | null = $state(null);
	let failed = $state(false);
	let previewId = $state<string | null>(null);
	const previewArea = $derived(areas.find((a) => a.id === previewId) ?? null);
	let closeTimer: ReturnType<typeof setTimeout> | undefined;
	let zoom = $state(4);
	let width = $state(0);
	let previewNode: HTMLElement | undefined = $state();
	let focusMarker: HTMLElement | null = null;
	let restoringFocus = false;
	function dismissPreview() {
		restoringFocus = true;
		closePreview();
		focusMarker?.focus();
		restoringFocus = false;
	}
	function showPreview(id: string, node: HTMLElement) {
		clearTimeout(closeTimer);
		focusMarker?.removeAttribute('aria-describedby');
		focusMarker = node;
		previewId = id;
		node.setAttribute('aria-describedby', 'call-map-preview-' + (compact ? 'widget' : 'full'));
	}
	function closePreview() {
		clearTimeout(closeTimer);
		focusMarker?.removeAttribute('aria-describedby');
		previewId = null;
	}
	function deferClose() {
		clearTimeout(closeTimer);
		closeTimer = setTimeout(() => {
			if (
				focusMarker !== document.activeElement &&
				!previewNode?.matches(':hover') &&
				!previewNode?.contains(document.activeElement)
			)
				closePreview();
		}, 250);
	}

	const ATTRIBUTION = [
		'© <a href="https://www.bundesnetzagentur.de/">Bundesnetzagentur</a>',
		'Source: <a href="https://www.rtr.at/">RTR-GmbH</a>',
		'<a href="https://www.geonames.org/">GeoNames</a> (CC BY 4.0)',
		'<a href="https://github.com/google/libphonenumber">libphonenumber</a>',
		'<a href="https://www.naturalearthdata.com/">Natural Earth</a>'
	].join(' · ');

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
					maxZoom: tileUrl ? 18 : OFFLINE_MAX_ZOOM,
					zoomSnap: 0.5,
					worldCopyJump: true,
					zoomControl: true,
					// The dashboard card does not catch the page scroll.
					scrollWheelZoom: !compact,
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
					m.createPane('cm-land').style.zIndex = '380';
					L.geoJSON(data, {
						pane: 'cm-land',
						interactive: false,
						style: () => ({
							className: 'call-map-country',
							fillOpacity: 1
						})
					}).addTo(m);
				}
				bubbles = L.layerGroup().addTo(m);
				const { MapDetail } = await import('#lib/calls/mapDetail.ts');
				if (disposed) return;
				detail = new MapDetail(L, m, {
					tiles: !!tileUrl,
					dark: theme.dark,
					locale: i18n.locale
				});
				// The zoom level as data attribute (layout checks and browser tests).
				const showZoom = () => {
					zoom = m.getZoom();
					if (el) el.dataset.zoom = String(zoom);
				};
				m.on('zoomend', showZoom);
				m.on('movestart', closePreview);
				showZoom();
				map = m;
			} catch {
				if (!disposed) failed = true;
			}
		})();
		return () => {
			disposed = true;
			clearTimeout(closeTimer);
			detail?.dispose();
			map?.remove();
			map = null;
		};
	});

	function areaLabel(a: MapArea, count: number): string {
		return t('callMap.bubbleLabel', {
			name: markerName(a, t('callMap.noLocation')),
			prefix: a.prefix,
			count
		});
	}

	// Detail layers follow the theme and the selected bubble.
	$effect(() => {
		const dark = theme.dark;
		detail?.setTheme(dark);
	});
	$effect(() => {
		const locale = i18n.locale;
		detail?.setLocale(locale);
	});
	$effect(() => {
		const sel = selected;
		void detail?.select(sel);
	});

	// Draws the bubbles again when data, categories, selection or theme change.
	$effect(() => {
		const m = map;
		const L = leaflet;
		void i18n.locale;
		void zoom;
		const mapWidth = width;
		const dark = theme.dark;
		const sel = selected;
		const located = areas.filter((a) => a.lat !== null && a.lon !== null);
		const counts = located.map((a) => shownTotal(a.counts, shown));
		if (!m || !L || !bubbles) return;
		closePreview();
		bubbles.clearLayers();
		const max = Math.max(0, ...counts);
		// Large bubbles first, so small bubbles stay on top and clickable.
		const order = located
			.map((a, i) => ({ a, n: counts[i] }))
			.filter((x) => x.n > 0)
			.sort((x, y) => y.n - x.n);
		const offsets = order.map(({ a }) =>
			a.kind === 'none' &&
			order.some(({ a: b }) => b.id !== a.id && a.country === b.country && b.kind === 'none')
				? markerKind(a) === 'mobile'
					? -40
					: 40
				: 0
		);
		const points = order.map(({ a }, i) => {
			const p = m.latLngToContainerPoint([a.lat!, a.lon!]);
			return { x: p.x + offsets[i], y: p.y };
		});
		for (const [i, { a, n }] of order.entries()) {
			const size = bubbleSize(n, max, compact ? 12 : 16, compact ? 26 : 38);
			const decorated = decorationsFit(points, i, mapWidth, compact);
			const w = Math.max(44, size + 22) + (decorated ? 48 : 0);
			const h = Math.max(44, size + 42);
			const icon = L.divIcon({
				className: `call-bubble${a.id === sel ? ' call-bubble-selected' : ''}${a.kind === 'none' ? ' call-bubble-none' : ''}`,
				iconSize: [w, h],
				iconAnchor: [w / 2 - offsets[i], h / 2],
				html: markerHtml(a, shown, size, dark, markerName(a, t('callMap.noLocation')), decorated)
			});
			const marker = L.marker([a.lat!, a.lon!], {
				icon,
				keyboard: true,
				interactive: true,
				riseOnHover: true
			});
			let touch = false;
			marker.on('add', () => {
				const node = marker.getElement();
				if (!node) return;
				node.setAttribute('aria-label', areaLabel(a, n));
				node.dataset.markerKind = markerKind(a);
				node.addEventListener('pointerdown', (e) => {
					touch = e.pointerType === 'touch';
				});
				node.addEventListener('pointermove', () => {
					if (!restoringFocus && a.id !== sel) showPreview(a.id, node);
				});
				node.addEventListener('pointerleave', deferClose);
				node.addEventListener('focus', () => {
					if (!restoringFocus && a.id !== sel) showPreview(a.id, node);
				});
				node.addEventListener('blur', deferClose);
				if (onselect) {
					node.setAttribute('role', 'button');
					node.setAttribute('aria-pressed', String(a.id === sel));
					node.addEventListener('keydown', (e) => {
						if (e.key === 'Enter' || e.key === ' ') {
							e.preventDefault();
							closePreview();
							onselect(a);
						}
					});
				} else {
					node.removeAttribute('tabindex');
				}
			});
			marker.on('click', () => {
				const node = marker.getElement();
				if (touch && node) {
					showPreview(a.id, node);
					touch = false;
				} else {
					closePreview();
					onselect?.(a);
				}
			});
			marker.addTo(bubbles);
		}
		if (!fitted && order.length) {
			fitted = true;
			const bounds = L.latLngBounds(order.map(({ a }) => [a.lat!, a.lon!] as [number, number]));
			m.fitBounds(bounds.pad(0.3), { maxZoom: 7, animate: false });
		}
	});
</script>

<svelte:window
	onpointerdown={(e) => {
		if (!(e.target instanceof Element) || !e.target.closest('.call-bubble, .call-map-preview'))
			closePreview();
	}}
	onkeydown={(e) => {
		if (e.key === 'Escape' && previewArea) {
			dismissPreview();
			e.stopImmediatePropagation();
		}
	}}
/>

<div class="relative">
	<div
		bind:this={el}
		bind:clientWidth={width}
		role="region"
		aria-label={label}
		class="call-map w-full overflow-hidden rounded-lg {compact
			? 'h-56'
			: 'h-[min(32rem,60dvh)] min-h-72'}"
	></div>
	{#if previewArea}
		<section
			bind:this={previewNode}
			id={'call-map-preview-' + (compact ? 'widget' : 'full')}
			class="call-map-preview absolute right-2 top-2 z-[1000] w-[min(19rem,calc(100%_-_1rem))] rounded-lg border border-slate-200 bg-white p-3 text-slate-900 shadow-lg dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
			aria-label={t('callMap.preview')}
			onpointerenter={() => clearTimeout(closeTimer)}
			onpointerleave={deferClose}
			onfocusout={deferClose}
		>
			<CallMapPreview
				area={previewArea}
				{shown}
				{range}
				onclose={dismissPreview}
				ondetails={() => {
					const a = previewArea;
					closePreview();
					if (a) onselect?.(a);
				}}
			/>
		</section>
	{/if}
	{#if !map && !failed}
		<div class="skeleton absolute inset-0 rounded-lg" aria-hidden="true"></div>
	{/if}
	{#if failed}
		<p class="absolute inset-0 grid place-items-center text-sm text-slate-500 dark:text-slate-400">
			{t('callMap.loadFailed')}
		</p>
	{/if}
</div>
