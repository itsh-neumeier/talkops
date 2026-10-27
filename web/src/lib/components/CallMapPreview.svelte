<script lang="ts">
	import type { MapArea } from '#lib/api.ts';
	import { markerKind, markerName } from '#lib/calls/mapMarkers.ts';
	import {
		CATEGORIES,
		CATEGORY_COLORS,
		shownTotal,
		type Category,
		type MapRange
	} from '#lib/calls/map.ts';
	import { formatDuration, t } from '#lib/i18n/index.svelte.ts';
	import { theme } from '#lib/theme.svelte.ts';
	let {
		area,
		shown,
		range,
		ondetails,
		onclose
	}: {
		area: MapArea;
		shown: Category[];
		range: MapRange;
		ondetails: () => void;
		onclose: () => void;
	} = $props();
</script>

<div class="flex items-start justify-between gap-2">
	<div>
		<h3 class="font-semibold">{markerName(area, t('callMap.noLocation'))}</h3>
		<p class="text-xs text-slate-500 dark:text-slate-400">
			{area.prefix} · {t(`callMap.marker.${markerKind(area)}`)}
		</p>
	</div>
	<button type="button" class="btn btn-sm" aria-label={t('common.close')} onclick={onclose}
		>✕</button
	>
</div>
<dl class="mt-2 grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-sm">
	{#each CATEGORIES.filter((c) => shown.includes(c)) as c (c)}
		<dt>
			<span
				class="mr-1 inline-block h-2 w-2 rounded-full"
				style:background={CATEGORY_COLORS[c][theme.dark ? 'dark' : 'light']}
			></span>{t(`callMap.category.${c}`)}
		</dt>
		<dd class="text-right font-mono">{area.counts[c]}</dd>
	{/each}
	<dt class="font-medium">{t('callMap.previewTotal', { range: t(`callMap.range.${range}`) })}</dt>
	<dd class="text-right font-mono">{shownTotal(area.counts, shown)}</dd>
	<dt>{t('callMap.duration')}</dt>
	<dd class="text-right font-mono">{formatDuration(area.billsec)}</dd>
</dl>
<p class="my-2 text-xs text-slate-600 dark:text-slate-300">
	{area.kind === 'area' ? t('callMap.areaHint') : t('callMap.countryHint')}
</p>
<button type="button" class="btn btn-sm" onclick={ondetails}>{t('callMap.details')}</button>
