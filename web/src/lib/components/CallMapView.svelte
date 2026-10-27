<script lang="ts">
	// Map view of the call log: range switch, category chips (legend), the map,
	// the details of a bubble and the list of calls without a location.
	import { onMount } from 'svelte';
	import { api, type CallMap as CallMapData, type MapArea, type MapTiles } from '#lib/api.ts';
	import { loadTiles } from '#lib/calls/tiles.ts';
	import CallMap from '#lib/components/CallMap.svelte';
	import CallMapPopup from '#lib/components/CallMapPopup.svelte';
	import CountryFlag from '#lib/components/CountryFlag.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import {
		CATEGORIES,
		CATEGORY_COLORS,
		MAP_RANGES,
		mapApiQuery,
		shownTotal,
		type Category,
		type MapRange
	} from '#lib/calls/map.ts';
	import { toggle } from '#lib/list/sort.ts';
	import { theme } from '#lib/theme.svelte.ts';
	import { formatDuration, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let {
		range = $bindable(),
		categories = $bindable(),
		phonebook = true,
		onchange
	}: {
		range: MapRange;
		categories: Category[];
		phonebook?: boolean;
		/** Range or categories changed (to update the page address). */
		onchange?: () => void;
	} = $props();

	/** Refresh interval of the 1-hour range. */
	const REFRESH_MS = 30_000;

	let data = $state<CallMapData | null>(null);
	let loading = $state(true);
	let error = $state('');
	let selectedId = $state<string | null>(null);
	/** Tile server setting; the map waits for it. */
	let tiles = $state<MapTiles | null>(null);
	let generation = 0;

	const selected = $derived(data?.areas.find((a) => a.id === selectedId) ?? null);
	const located = $derived(
		(data?.areas ?? []).filter((a) => a.lat !== null && shownTotal(a.counts, categories) > 0)
	);
	const unlocated = $derived(
		(data?.areas ?? []).filter((a) => a.kind === 'none' && shownTotal(a.counts, categories) > 0)
	);
	const anonymous = $derived(data ? shownTotal(data.anonymous, categories) : 0);
	const shownCalls = $derived(data ? shownTotal(data.counts, categories) : 0);

	async function load() {
		const run = ++generation;
		loading = true;
		try {
			const res = await api.get<CallMapData>(mapApiQuery({ range, categories }));
			if (run !== generation) return;
			data = res;
			error = '';
			if (selectedId && !res.areas.some((a) => a.id === selectedId)) selectedId = null;
		} catch (err) {
			if (run === generation) error = errorMessage(err);
		} finally {
			if (run === generation) loading = false;
		}
	}

	onMount(() => {
		load();
		loadTiles().then((v) => (tiles = v));
		const timer = setInterval(() => {
			if (range === '1h' && !document.hidden) load();
		}, REFRESH_MS);
		const visible = () => {
			if (range === '1h' && !document.hidden) load();
		};
		document.addEventListener('visibilitychange', visible);
		return () => {
			clearInterval(timer);
			document.removeEventListener('visibilitychange', visible);
			generation++;
		};
	});

	function setRange(r: MapRange) {
		range = r;
		onchange?.();
		load();
	}

	function toggleCategory(c: Category) {
		const next = toggle(categories, c);
		// At least one category stays on.
		if (!next.length) return;
		categories = CATEGORIES.filter((x) => next.includes(x));
		onchange?.();
		load();
	}

	function select(a: MapArea) {
		selectedId = selectedId === a.id ? null : a.id;
	}

	const color = (c: Category) => (theme.dark ? CATEGORY_COLORS[c].dark : CATEGORY_COLORS[c].light);
	const rangeLabel = (r: MapRange) => t(`callMap.range.${r}`);
</script>

<div class="space-y-4">
	<div class="card space-y-3">
		<div class="flex flex-wrap items-center justify-between gap-3">
			<Tabs
				variant="pills"
				label={t('callMap.range')}
				tabs={MAP_RANGES.map((r) => ({ id: r, label: rangeLabel(r) }))}
				value={range}
				onchange={setRange}
			/>
			<div class="flex flex-wrap gap-1" role="group" aria-label={t('callMap.legend')}>
				{#each CATEGORIES as c (c)}
					<button
						type="button"
						class="btn btn-sm gap-1.5 {categories.includes(c) ? '' : 'opacity-60'}"
						aria-pressed={categories.includes(c)}
						onclick={() => toggleCategory(c)}
					>
						<span
							class="inline-block h-2.5 w-2.5 rounded-full {categories.includes(c)
								? ''
								: 'ring-1 ring-slate-400'}"
							style:background={categories.includes(c) ? color(c) : 'transparent'}
						></span>
						{t(`callMap.category.${c}`)}
						{#if data}<span class="font-mono text-xs">{data.counts[c]}</span>{/if}
					</button>
				{/each}
			</div>
		</div>
		<ErrorBox {error} />
		{#if loading && data}
			<p role="status" class="text-sm text-slate-500 dark:text-slate-400">{t('common.loading')}</p>
		{/if}
		{#if data?.truncated}
			<p class="badge badge-warn">{t('callMap.truncated', { limit: data.limit })}</p>
		{/if}
		<div class="relative">
			{#if tiles}
				<CallMap
					{range}
					tileUrl={tiles.tile_url}
					tileAttribution={tiles.attribution}
					areas={located}
					shown={categories}
					selected={selectedId}
					onselect={select}
					label={t('callMap.mapLabel', { range: rangeLabel(range) })}
				/>
			{:else}
				<div class="skeleton h-72 rounded-lg" aria-hidden="true"></div>
			{/if}
			{#if data && !loading && shownCalls === 0}
				<p
					class="pointer-events-none absolute inset-x-0 top-4 mx-auto w-fit rounded-md bg-white/90 px-3 py-1.5 text-sm text-slate-600 shadow dark:bg-slate-900/90 dark:text-slate-300"
				>
					{t('callMap.empty')}
				</p>
			{/if}
		</div>
		<p
			class="flex flex-wrap gap-3 text-xs text-slate-600 dark:text-slate-300"
			aria-label={t('callMap.markerLegend')}
		>
			<span>☎ {t('callMap.marker.area')}</span><span>▯ {t('callMap.marker.mobile')}</span><span
				>▧ {t('callMap.marker.unlocated')}</span
			>
		</p>
	</div>

	<div class="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
		{#if selected}
			<div class="card popover-panel">
				<CallMapPopup
					area={selected}
					{range}
					shown={categories}
					{phonebook}
					onclose={() => (selectedId = null)}
				/>
			</div>
		{/if}
		<section class="card space-y-2" aria-labelledby="call-map-unlocated">
			<h2 id="call-map-unlocated" class="font-semibold">{t('callMap.unlocatedTitle')}</h2>
			<p class="hint">{t('callMap.unlocatedHint')}</p>
			{#if unlocated.length === 0 && anonymous === 0}
				<p class="text-sm text-slate-500 dark:text-slate-400">{t('callMap.unlocatedEmpty')}</p>
			{/if}
			<ul class="divide-y divide-slate-200 text-sm dark:divide-slate-800">
				{#each unlocated as a (a.id)}
					<li>
						<button
							type="button"
							class="flex w-full items-center justify-between gap-2 py-1.5 text-left hover:text-teal-700 dark:hover:text-teal-300"
							aria-pressed={selectedId === a.id}
							onclick={() => select(a)}
						>
							<span class="min-w-0">
								{#if a.country}<CountryFlag country={a.country} />{:else}{t(
										'callMap.unknownCountry'
									)}{/if}
								<span class="block text-xs text-slate-500 dark:text-slate-400"
									>{t('callMap.noLocationReasons', {
										mobile: a.mobile,
										service: a.service,
										other: a.unresolved
									})}</span
								>
							</span>
							<span class="font-mono">{shownTotal(a.counts, categories)}</span>
						</button>
					</li>
				{/each}
				{#if anonymous > 0}
					<li class="flex items-center justify-between gap-2 py-1.5">
						<span>{t('callMap.anonymous')}</span>
						<span class="font-mono">{anonymous}</span>
					</li>
				{/if}
			</ul>
			{#if data}
				<p class="pt-2 text-xs text-slate-500 dark:text-slate-400">
					{t('callMap.summary', {
						calls: shownCalls,
						areas: located.filter((a) => a.kind === 'area').length,
						duration: formatDuration((data.areas ?? []).reduce((sum, a) => sum + a.billsec, 0))
					})}
				</p>
			{/if}
		</section>
	</div>
	<p class="hint">{t('callMap.accuracy')}</p>
</div>
