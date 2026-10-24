<script lang="ts">
	// A dashboard figure: value, trend against the previous period and a
	// small chart (sparkline or ring gauge).
	import {
		ringOffset,
		smoothPath,
		sparkPoints,
		trendClass,
		areaPath,
		type Trend
	} from '#lib/charts.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		label,
		value,
		trend = null,
		upIsGood = true,
		spark = null,
		ring = null,
		accent = 'text-teal-600 dark:text-teal-400',
		alert = false
	}: {
		label: string;
		value: string;
		trend?: Trend | null;
		/** False when a rising value is bad (missed calls). */
		upIsGood?: boolean;
		/** Values per time slot for a sparkline. */
		spark?: number[] | null;
		/** Percent for a ring gauge instead of a sparkline. */
		ring?: number | null;
		accent?: string;
		/** Shows the value in red. */
		alert?: boolean;
	} = $props();

	const SW = 96;
	const SH = 32;
	const points = $derived(spark && spark.length > 1 ? sparkPoints(spark, SW, SH) : []);
	const R = 15;
	const C = 2 * Math.PI * R;
	const id = $props.id();
	const arrow = $derived(trend?.direction === 'up' ? '▲' : trend?.direction === 'down' ? '▼' : '–');
</script>

<div class="card flex items-end justify-between gap-2">
	<div class="min-w-0">
		<p class="truncate text-xs text-slate-500 dark:text-slate-400">{label}</p>
		<p
			class="mt-1 text-2xl font-semibold tabular-nums {alert
				? 'text-red-600 dark:text-red-400'
				: ''}"
		>
			{value}
		</p>
		{#if trend}
			<p
				class="mt-0.5 text-xs whitespace-nowrap {trendClass(trend, upIsGood)}"
				title={t('dash.kpi.vsPrevious')}
				data-testid="kpi-trend"
			>
				<span aria-hidden="true">{arrow}</span>
				{trend.percent == null ? t('dash.kpi.new') : `${Math.abs(trend.percent)} %`}
				<span class="sr-only">{t('dash.kpi.vsPrevious')}</span>
			</p>
		{/if}
	</div>
	{#if ring !== undefined && ring !== null}
		<svg viewBox="0 0 40 40" class="h-12 w-12 shrink-0 -rotate-90 {accent}" aria-hidden="true">
			<circle
				cx="20"
				cy="20"
				r={R}
				class="fill-none stroke-slate-200 dark:stroke-slate-700"
				stroke-width="5"
			/>
			<circle
				cx="20"
				cy="20"
				r={R}
				class="fill-none stroke-current"
				stroke-width="5"
				stroke-linecap="round"
				stroke-dasharray={C}
				stroke-dashoffset={ringOffset(ring, C)}
			/>
		</svg>
	{:else if points.length}
		<svg viewBox="0 0 {SW} {SH}" class="h-8 w-20 shrink-0 {accent} @3xl:w-24" aria-hidden="true">
			<defs>
				<linearGradient id="spark-{id}" x1="0" x2="0" y1="0" y2="1">
					<stop offset="0%" style="stop-color: currentColor" stop-opacity="0.35" />
					<stop offset="100%" style="stop-color: currentColor" stop-opacity="0" />
				</linearGradient>
			</defs>
			<path d={areaPath(points, SH)} fill="url(#spark-{id})" />
			<path
				d={smoothPath(points)}
				class="fill-none stroke-current"
				stroke-width="1.75"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
		</svg>
	{/if}
</div>
