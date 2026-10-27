<script lang="ts">
	// Dashboard card: a small call map (1 hour or 1 day) and the five areas
	// with the most calls. A click opens the map view of the call log.
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api, type CallMap as CallMapData, type MapTiles } from '#lib/api.ts';
	import CallMap from '#lib/components/CallMap.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import {
		CATEGORIES,
		mapApiQuery,
		mapHref,
		shownTotal,
		topAreas,
		type MapRange
	} from '#lib/calls/map.ts';
	import { loadTiles } from '#lib/calls/tiles.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	const RANGES: MapRange[] = ['1h', '1d'];
	const shown = [...CATEGORIES];

	let range = $state<MapRange>('1d');
	let data = $state<CallMapData | null>(null);
	let tiles = $state<MapTiles | null>(null);
	let failed = $state(false);
	let generation = 0;

	const areas = $derived(Array.isArray(data?.areas) ? data.areas : []);
	const located = $derived(areas.filter((a) => a.lat !== null && a.lon !== null));
	const top = $derived(topAreas(areas, shown));

	async function load() {
		const run = ++generation;
		try {
			const res = await api.get<CallMapData>(mapApiQuery({ range, categories: shown }));
			if (run !== generation) return;
			data = res;
			failed = false;
		} catch {
			if (run === generation) failed = true;
		}
	}

	onMount(() => {
		load();
		loadTiles().then((v) => (tiles = v));
		const timer = setInterval(() => {
			if (range === '1h' && !document.hidden) load();
		}, 60_000);
		return () => {
			clearInterval(timer);
			generation++;
		};
	});

	function setRange(r: MapRange) {
		range = r;
		load();
	}
</script>

<section class="card space-y-3" aria-labelledby="dash-call-map">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h2 id="dash-call-map">{t('callMap.widgetTitle')}</h2>
		<div class="flex items-center gap-2">
			<Tabs
				variant="pills"
				label={t('callMap.range')}
				tabs={RANGES.map((r) => ({ id: r, label: t(`callMap.range.${r}`) }))}
				value={range}
				onchange={setRange}
			/>
			<a class="text-sm text-teal-700 hover:underline dark:text-teal-300" href={mapHref(range)}
				>{t('callMap.openMap')}</a
			>
		</div>
	</div>
	{#if failed}
		<p role="alert" class="text-sm text-red-600 dark:text-red-400">{t('status.error')}</p>
	{/if}
	<div class="grid grid-cols-1 gap-4 @3xl:grid-cols-3">
		<div class="@3xl:col-span-2">
			{#if tiles}
				<CallMap
					{range}
					compact
					areas={located}
					{shown}
					tileUrl={tiles.tile_url}
					tileAttribution={tiles.attribution}
					onselect={() => goto(mapHref(range))}
					label={t('callMap.mapLabel', { range: t(`callMap.range.${range}`) })}
				/>
			{:else}
				<div class="skeleton h-56 rounded-lg" aria-hidden="true"></div>
			{/if}
		</div>
		<div>
			<h3 class="mb-1 text-sm font-medium">{t('callMap.topAreas')}</h3>
			{#if data && top.length === 0}
				<p class="text-sm text-slate-500 dark:text-slate-400">{t('callMap.empty')}</p>
			{/if}
			<ol class="divide-y divide-slate-100 text-sm dark:divide-slate-800">
				{#each top as a (a.id)}
					<li class="flex items-center justify-between gap-2 py-1.5">
						<a class="min-w-0 truncate hover:underline" href={mapHref(range)}
							>{a.name} <span class="font-mono text-xs text-slate-500">{a.prefix}</span></a
						>
						<span class="font-mono tabular-nums">{shownTotal(a.counts, shown)}</span>
					</li>
				{/each}
			</ol>
		</div>
	</div>
</section>
