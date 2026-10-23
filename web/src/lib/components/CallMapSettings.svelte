<script lang="ts">
	// Optional tile server of the call map (settings managers).
	import { onMount } from 'svelte';
	import { api, type MapTiles } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { tileUrlProblem } from '#lib/calls/map.ts';
	import { resetTiles } from '#lib/calls/tiles.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let tiles = $state<MapTiles | null>(null);
	let error = $state('');
	let saved = $state(false);

	const problem = $derived(tiles ? tileUrlProblem(tiles.tile_url) : '');

	onMount(async () => {
		try {
			const v = await api.get<Partial<MapTiles>>('/settings/call-map');
			tiles = { tile_url: v?.tile_url ?? '', attribution: v?.attribution ?? '' };
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!tiles || problem) return;
		error = '';
		saved = false;
		try {
			tiles = await api.put<MapTiles>('/settings/call-map', tiles);
			resetTiles();
			saved = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<form class="card space-y-3" onsubmit={save}>
	<h2>{t('callMap.settingsTitle')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('callMap.settingsHint')}</p>
	<ErrorBox {error} />
	{#if saved}<p class="text-sm text-emerald-700 dark:text-emerald-400">{t('common.saved')}</p>{/if}
	{#if tiles}
		<div>
			<label for="map-tile-url">{t('callMap.tileUrl')}</label>
			<input
				id="map-tile-url"
				class="input font-mono"
				type="url"
				inputmode="url"
				maxlength="500"
				placeholder="https://tile.openstreetmap.org/&#123;z&#125;/&#123;x&#125;/&#123;y&#125;.png"
				bind:value={tiles.tile_url}
				aria-invalid={!!problem}
				aria-describedby="map-tile-url-hint"
			/>
			<p id="map-tile-url-hint" class="hint">{t('callMap.tileUrlHint')}</p>
			{#if problem}
				<p class="text-sm text-red-600 dark:text-red-400">{t(`callMap.tileError.${problem}`)}</p>
			{/if}
		</div>
		<div>
			<label for="map-tile-attribution">{t('callMap.tileAttribution')}</label>
			<input
				id="map-tile-attribution"
				class="input"
				maxlength="300"
				required={!!tiles.tile_url.trim()}
				bind:value={tiles.attribution}
			/>
			<p class="hint">{t('callMap.tileAttributionHint')}</p>
		</div>
		<p class="badge badge-warn whitespace-normal">{t('callMap.tilePrivacy')}</p>
		<div class="flex justify-end">
			<button class="btn btn-primary" disabled={!!problem}>{t('common.save')}</button>
		</div>
	{:else if !error}
		<Skeleton lines={3} />
	{/if}
</form>
